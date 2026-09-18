//! Persists reminders to `settings.json` in the app config dir and exposes
//! the commands the Settings window uses.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    popup,
    reminder::{self, Reminder, CUSTOM_KIND},
    scheduler::{self, SchedulerState},
    tray,
};

const FILE_NAME: &str = "settings.json";
const VERSION: u32 = 1;
const CHANGED_EVENT: &str = "reminders:changed";

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    reminders: Vec<Reminder>,
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

/// Saved reminders, or the presets on first launch. A corrupt file is kept
/// as `settings.json.bak` and replaced with presets.
pub fn load(app: &AppHandle) -> Vec<Reminder> {
    let Some(path) = path(app) else {
        return reminder::presets();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return reminder::presets();
    };
    match serde_json::from_str::<SettingsFile>(&text) {
        Ok(file) => file.reminders,
        Err(err) => {
            eprintln!("settings: unreadable {}: {err}", path.display());
            let _ = fs::rename(&path, path.with_extension("json.bak"));
            reminder::presets()
        }
    }
}

fn save(app: &AppHandle, reminders: &[Reminder]) -> Result<(), String> {
    let path = path(app).ok_or("Couldn't find the settings folder.")?;
    let file = SettingsFile {
        version: VERSION,
        reminders: reminders.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Couldn't create settings folder: {e}"))?;
    }
    // Write-then-rename so a crash never leaves a half-written file.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("Couldn't save settings: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Couldn't save settings: {e}"))
}

/// Applies a change to the scheduler, then saves and notifies tray + Settings.
pub fn apply(
    app: &AppHandle,
    change: impl FnOnce(&mut scheduler::Scheduler, u64) -> Result<(), String>,
) -> Result<Vec<Reminder>, String> {
    let reminders = {
        let state = app.state::<SchedulerState>();
        let mut scheduler = state.lock().unwrap();
        change(&mut scheduler, scheduler::now_ms())?;
        scheduler.reminders()
    };
    save(app, &reminders)?;
    tray::rebuild_menu(app);
    let _ = app.emit(CHANGED_EVENT, &reminders);
    Ok(reminders)
}

fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "r{}-{}",
        scheduler::now_ms(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

// ---------- commands ----------

#[tauri::command]
pub fn get_reminders(app: AppHandle) -> Vec<Reminder> {
    app.state::<SchedulerState>().lock().unwrap().reminders()
}

/// Creates (empty id) or updates a reminder.
#[tauri::command]
pub fn save_reminder(app: AppHandle, mut reminder: Reminder) -> Result<Vec<Reminder>, String> {
    reminder.validate()?;
    apply(&app, |scheduler, now| {
        match scheduler.get(&reminder.id) {
            // Kind is fixed once created; don't trust the client with it.
            Some(existing) => reminder.kind = existing.kind.clone(),
            None if reminder.id.is_empty() => {
                reminder.id = new_id();
                reminder.kind = CUSTOM_KIND.into();
            }
            None => return Err("That reminder no longer exists.".into()),
        }
        scheduler.upsert(reminder, now);
        Ok(())
    })
}

#[tauri::command]
pub fn delete_reminder(app: AppHandle, id: String) -> Result<Vec<Reminder>, String> {
    apply(&app, |scheduler, _| {
        match scheduler.get(&id) {
            Some(r) if r.is_custom() => {}
            Some(_) => return Err("Built-in reminders can be turned off but not deleted.".into()),
            None => return Err("That reminder no longer exists.".into()),
        }
        scheduler.remove(&id);
        Ok(())
    })
}

pub fn set_enabled(app: &AppHandle, id: &str, enabled: bool) -> Result<Vec<Reminder>, String> {
    apply(app, |scheduler, now| {
        let mut reminder = scheduler
            .get(id)
            .cloned()
            .ok_or("That reminder no longer exists.")?;
        reminder.enabled = enabled;
        scheduler.upsert(reminder, now);
        Ok(())
    })
}

#[tauri::command]
pub fn set_reminder_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<Vec<Reminder>, String> {
    set_enabled(&app, &id, enabled)
}

/// Shows a draft reminder's popup without scheduling it.
#[tauri::command]
pub fn preview_reminder(app: AppHandle, mut reminder: Reminder) -> Result<(), String> {
    reminder.validate()?;
    let kind = app
        .state::<SchedulerState>()
        .lock()
        .unwrap()
        .get(&reminder.id)
        .map_or_else(|| CUSTOM_KIND.to_string(), |r| r.kind.clone());
    let mut popup = reminder.to_popup();
    popup.id = format!("preview-{}", new_id());
    popup.kind = kind;
    popup::enqueue(&app, popup);
    Ok(())
}
