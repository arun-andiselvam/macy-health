//! Update checks against GitHub Releases (`latest.json`, signed with the
//! key in tauri.conf.json). Checks run in the background; installing is
//! always the user's choice, from the tray or Settings.

use std::{sync::Mutex, thread, time::Duration};

use serde::Serialize;
use tauri::{async_runtime, AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const CHANGED_EVENT: &str = "update:changed";
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Serialize, Default)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
    },
    Installing,
    Failed {
        message: String,
    },
}

#[derive(Default)]
pub struct UpdaterState {
    status: UpdateStatus,
    pending: Option<Update>,
}

pub type SharedUpdater = Mutex<UpdaterState>;

pub fn status(app: &AppHandle) -> UpdateStatus {
    app.state::<SharedUpdater>().lock().unwrap().status.clone()
}

pub fn available_version(app: &AppHandle) -> Option<String> {
    match status(app) {
        UpdateStatus::Available { version } => Some(version),
        _ => None,
    }
}

fn set_status(app: &AppHandle, status: UpdateStatus) {
    app.state::<SharedUpdater>().lock().unwrap().status = status.clone();
    crate::tray::rebuild_menu(app);
    let _ = app.emit(CHANGED_EVENT, status);
}

/// Background checks, release builds only (dev builds have no release to compare to).
pub fn start_background_checks(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(FIRST_CHECK_DELAY);
        loop {
            async_runtime::block_on(check(&app, false));
            thread::sleep(CHECK_EVERY);
        }
    });
}

/// Background checks stay quiet on failure (e.g. offline); user checks report it.
pub async fn check(app: &AppHandle, user_initiated: bool) {
    if matches!(
        status(app),
        UpdateStatus::Checking | UpdateStatus::Installing
    ) {
        return;
    }
    let previous = status(app);
    if user_initiated {
        set_status(app, UpdateStatus::Checking);
    }

    let result = async { app.updater()?.check().await }.await;
    match result {
        Ok(Some(update)) => {
            let version = update.version.clone();
            app.state::<SharedUpdater>().lock().unwrap().pending = Some(update);
            set_status(app, UpdateStatus::Available { version });
        }
        Ok(None) => set_status(app, UpdateStatus::UpToDate),
        Err(err) if user_initiated => set_status(
            app,
            UpdateStatus::Failed {
                message: format!("Couldn't check for updates: {err}"),
            },
        ),
        Err(err) => {
            eprintln!("updater: {err}");
            set_status(app, previous);
        }
    }
}

/// Downloads, installs and restarts into the new version.
pub async fn install(app: &AppHandle) {
    let Some(update) = app.state::<SharedUpdater>().lock().unwrap().pending.clone() else {
        return;
    };
    set_status(app, UpdateStatus::Installing);
    match update.download_and_install(|_, _| {}, || {}).await {
        Ok(()) => app.restart(),
        Err(err) => set_status(
            app,
            UpdateStatus::Failed {
                message: format!("Couldn't install the update: {err}"),
            },
        ),
    }
}

// ---------- commands ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current_version: String,
    status: UpdateStatus,
}

#[tauri::command]
pub fn get_update_info(app: AppHandle) -> UpdateInfo {
    UpdateInfo {
        current_version: app.package_info().version.to_string(),
        status: status(&app),
    }
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) {
    check(&app, true).await;
}

#[tauri::command]
pub async fn install_update(app: AppHandle) {
    install(&app).await;
}
