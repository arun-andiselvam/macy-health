//! Persists reminders to `settings.json` in the app config dir and exposes
//! the commands the Settings window uses.

use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    popup,
    reminder::{self, Reminder, CUSTOM_KIND},
    scheduler::{self, SchedulerState},
    tray,
};

const FILE_NAME: &str = "settings.json";
const VERSION: u32 = 1;
const CHANGED_EVENT: &str = "reminders:changed";
const GENERAL_CHANGED_EVENT: &str = "general:changed";

const IDLE_CHOICES: [u64; 5] = [0, 2, 5, 10, 15];
const POPUP_SECONDS_RANGE: std::ops::RangeInclusive<u64> = 10..=120;

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    reminders: Vec<Reminder>,
    #[serde(default)]
    general: General,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    /// Pause after this many minutes without input; 0 turns it off.
    pub idle_pause_minutes: u64,
    /// How long a popup stays before closing itself.
    pub popup_seconds: u64,
    /// Persisted copy of the scheduler's pause (epoch ms).
    pub paused_until: Option<u64>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            idle_pause_minutes: 5,
            popup_seconds: 30,
            paused_until: None,
        }
    }
}

pub type GeneralState = Mutex<General>;

pub fn general(app: &AppHandle) -> General {
    app.state::<GeneralState>().lock().unwrap().clone()
}

/// What the Settings window shows under "General".
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralView {
    launch_at_login: bool,
    idle_pause_minutes: u64,
    popup_seconds: u64,
    paused_until: Option<u64>,
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

/// Saved settings, or presets on first launch. A corrupt file is kept as
/// `settings.json.bak` and replaced with presets.
pub fn load(app: &AppHandle) -> (Vec<Reminder>, General) {
    let defaults = || (reminder::presets(), General::default());
    let Some(path) = path(app) else {
        return defaults();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return defaults();
    };
    match serde_json::from_str::<SettingsFile>(&text) {
        Ok(file) => (file.reminders, file.general),
        Err(err) => {
            eprintln!("settings: unreadable {}: {err}", path.display());
            let _ = fs::rename(&path, path.with_extension("json.bak"));
            defaults()
        }
    }
}

pub fn init(app: &AppHandle, general: General) {
    app.manage::<GeneralState>(Mutex::new(general));
}

/// Writes reminders, general settings and the current pause to disk.
fn persist(app: &AppHandle) -> Result<(), String> {
    let (reminders, paused_until) = {
        let state = app.state::<SchedulerState>();
        let scheduler = state.lock().unwrap();
        (scheduler.reminders(), scheduler.paused_until())
    };
    let mut general = general(app);
    general.paused_until = paused_until;

    let path = path(app).ok_or("Couldn't find the settings folder.")?;
    let file = SettingsFile {
        version: VERSION,
        reminders,
        general,
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
    persist(app)?;
    tray::rebuild_menu(app);
    scheduler::refresh_status(app);
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

// ---------- general settings & pause ----------

fn general_view(app: &AppHandle) -> GeneralView {
    let general = general(app);
    GeneralView {
        launch_at_login: app.autolaunch().is_enabled().unwrap_or(false),
        idle_pause_minutes: general.idle_pause_minutes,
        popup_seconds: general.popup_seconds,
        paused_until: app.state::<SchedulerState>().lock().unwrap().paused_until(),
    }
}

/// Saves, then updates the tray and Settings after a pause starts or ends.
pub fn pause_changed(app: &AppHandle) {
    if let Err(err) = persist(app) {
        eprintln!("settings: {err}");
    }
    tray::rebuild_menu(app);
    scheduler::refresh_status(app);
    let _ = app.emit(GENERAL_CHANGED_EVENT, general_view(app));
}

pub enum PauseFor {
    Minutes(u64),
    UntilTomorrow,
}

pub fn pause(app: &AppHandle, pause_for: PauseFor) {
    let until = match pause_for {
        PauseFor::Minutes(m) => scheduler::now_ms() + m * 60_000,
        PauseFor::UntilTomorrow => scheduler::next_midnight_ms(),
    };
    app.state::<SchedulerState>().lock().unwrap().pause(until);
    popup::dismiss_all(app);
    pause_changed(app);
}

pub fn resume(app: &AppHandle) {
    app.state::<SchedulerState>()
        .lock()
        .unwrap()
        .resume(scheduler::now_ms());
    pause_changed(app);
}

#[tauri::command]
pub fn get_general(app: AppHandle) -> GeneralView {
    general_view(&app)
}

#[tauri::command]
pub fn save_general(
    app: AppHandle,
    launch_at_login: bool,
    idle_pause_minutes: u64,
    popup_seconds: u64,
) -> Result<GeneralView, String> {
    if !IDLE_CHOICES.contains(&idle_pause_minutes) {
        return Err("Choose one of the listed away times.".into());
    }
    if !POPUP_SECONDS_RANGE.contains(&popup_seconds) {
        return Err("Popups can stay between 10 seconds and 2 minutes.".into());
    }

    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().unwrap_or(false) != launch_at_login {
        let result = if launch_at_login {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        result.map_err(|e| format!("Couldn't change the login item: {e}"))?;
    }

    {
        let state = app.state::<GeneralState>();
        let mut general = state.lock().unwrap();
        general.idle_pause_minutes = idle_pause_minutes;
        general.popup_seconds = popup_seconds;
    }
    persist(&app)?;
    let view = general_view(&app);
    let _ = app.emit(GENERAL_CHANGED_EVENT, &view);
    Ok(view)
}

/// `minutes: None` pauses until tomorrow.
#[tauri::command]
pub fn pause_reminders(app: AppHandle, minutes: Option<u64>) -> GeneralView {
    pause(
        &app,
        minutes.map_or(PauseFor::UntilTomorrow, PauseFor::Minutes),
    );
    general_view(&app)
}

#[tauri::command]
pub fn resume_reminders(app: AppHandle) -> GeneralView {
    resume(&app);
    general_view(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_files_without_general_still_load() {
        let json = r#"{"version":1,"reminders":[]}"#;
        let file: SettingsFile = serde_json::from_str(json).unwrap();
        assert_eq!(file.general.idle_pause_minutes, 5);
        assert_eq!(file.general.popup_seconds, 30);
        assert!(file.general.paused_until.is_none());
    }
}
