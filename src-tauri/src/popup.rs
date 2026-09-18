//! Reminder popup: a transparent, non-focusable, always-on-top window in the
//! top-right corner of the screen under the cursor. Reminders queue and show
//! one at a time.

use std::{collections::VecDeque, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const POPUP_LABEL: &str = "popup";
const SHOW_EVENT: &str = "reminder:show";

// Logical px. Includes transparent padding around the card for its shadow.
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 168.0;
const MARGIN: f64 = 4.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupReminder {
    pub id: String,
    pub kind: String,
    pub emoji: String,
    pub title: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PopupAction {
    Done,
    Snooze,
    Skip,
    Dismissed,
}

#[derive(Default)]
pub struct PopupQueue {
    current: Option<PopupReminder>,
    pending: VecDeque<PopupReminder>,
}

pub type PopupState = Mutex<PopupQueue>;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("popup.html".into()))
        .title("Macy Health Reminder")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .visible_on_all_workspaces(true);

    // macOS gets a higher window level in `macos::float_above_everything`.
    #[cfg(not(target_os = "macos"))]
    let builder = builder.always_on_top(true);

    let _window = builder.build()?;

    #[cfg(target_os = "macos")]
    macos::float_above_everything(&_window)?;

    Ok(())
}

pub fn enqueue(app: &AppHandle, reminder: PopupReminder) {
    let state = app.state::<PopupState>();
    let mut queue = state.lock().unwrap();
    if queue.current.is_some() {
        queue.pending.push_back(reminder);
        return;
    }
    queue.current = Some(reminder.clone());
    drop(queue);
    present(app, &reminder);
}

fn present(app: &AppHandle, reminder: &PopupReminder) {
    let Some(window) = app.get_webview_window(POPUP_LABEL) else {
        return;
    };
    if let Err(err) = place_top_right(app, &window) {
        eprintln!("popup: failed to position window: {err}");
    }
    let _ = window.show();
    let _ = app.emit_to(POPUP_LABEL, SHOW_EVENT, reminder);
}

/// Top-right of the work area (below the menu bar) on the monitor under the cursor.
fn place_top_right(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let origin = area.position.to_logical::<f64>(scale);
    let size = area.size.to_logical::<f64>(scale);

    window.set_size(LogicalSize::new(WIDTH, HEIGHT))?;
    window.set_position(LogicalPosition::new(
        origin.x + size.width - WIDTH - MARGIN,
        origin.y + MARGIN,
    ))
}

/// Popup webview calls this on load in case it missed the show event.
#[tauri::command]
pub fn popup_current(state: State<PopupState>) -> Option<PopupReminder> {
    state.lock().unwrap().current.clone()
}

#[tauri::command]
pub fn popup_action(app: AppHandle, state: State<PopupState>, id: String, action: PopupAction) {
    let next = {
        let mut queue = state.lock().unwrap();
        if queue.current.as_ref().map(|r| r.id.as_str()) != Some(id.as_str()) {
            return; // stale action from an already-replaced popup
        }
        queue.current = queue.pending.pop_front();
        queue.current.clone()
    };

    crate::scheduler::handle_action(&app, &id, action);

    match next {
        Some(reminder) => present(&app, &reminder),
        None => {
            if let Some(window) = app.get_webview_window(POPUP_LABEL) {
                let _ = window.hide();
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior};

    /// Float above normal and full-screen apps, on every Space, out of Cmd+`.
    pub fn float_above_everything(window: &tauri::WebviewWindow) -> tauri::Result<()> {
        let ptr = window.ns_window()? as *const NSWindow;
        // SAFETY: called on the main thread during setup; tao owns the NSWindow
        // for the lifetime of the Tauri window.
        let ns_window = unsafe { &*ptr };
        ns_window.setLevel(NSStatusWindowLevel);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns_window.setHidesOnDeactivate(false);
        Ok(())
    }
}
