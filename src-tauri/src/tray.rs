use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

use tauri::{
    image::Image,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, Wry,
};

use crate::{
    popup, scheduler,
    settings::{self, PauseFor},
};

pub const TRAY_ID: &str = "main";

const STATUS_ID: &str = "status";
const PAUSE_30_ID: &str = "pause:30";
const PAUSE_60_ID: &str = "pause:60";
const PAUSE_TOMORROW_ID: &str = "pause:tomorrow";
const RESUME_ID: &str = "resume";
const TOGGLE_PREFIX: &str = "toggle:";
const TEST_POPUP_ID: &str = "test_popup";
const SETTINGS_ID: &str = "settings";
const QUIT_ID: &str = "quit";

/// Status line at the top of the tray menu ("Next: 💧 Drink water in 12m").
/// Replaced whenever the menu is rebuilt.
#[derive(Default)]
struct TrayStatus(Mutex<Option<MenuItem<Wry>>>);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    app.manage(TrayStatus::default());
    let menu = build_menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Macy Health")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            PAUSE_30_ID => settings::pause(app, PauseFor::Minutes(30)),
            PAUSE_60_ID => settings::pause(app, PauseFor::Minutes(60)),
            PAUSE_TOMORROW_ID => settings::pause(app, PauseFor::UntilTomorrow),
            RESUME_ID => settings::resume(app),
            TEST_POPUP_ID => show_test_popup(app),
            SETTINGS_ID => show_settings(app),
            QUIT_ID => app.exit(0),
            id => {
                if let Some(reminder_id) = id.strip_prefix(TOGGLE_PREFIX) {
                    toggle_reminder(app, reminder_id);
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let status = MenuItem::with_id(
        app,
        STATUS_ID,
        scheduler::status_text(app),
        false,
        None::<&str>,
    )?;
    *app.state::<TrayStatus>().0.lock().unwrap() = Some(status.clone());

    let (reminders, paused) = {
        let state = app.state::<scheduler::SchedulerState>();
        let scheduler = state.lock().unwrap();
        (scheduler.reminders(), scheduler.paused_until().is_some())
    };

    let pause: Box<dyn IsMenuItem<Wry>> = if paused {
        Box::new(MenuItem::with_id(
            app,
            RESUME_ID,
            "Resume Reminders",
            true,
            None::<&str>,
        )?)
    } else {
        Box::new(Submenu::with_items(
            app,
            "Pause Reminders",
            true,
            &[
                &MenuItem::with_id(app, PAUSE_30_ID, "For 30 Minutes", true, None::<&str>)?,
                &MenuItem::with_id(app, PAUSE_60_ID, "For 1 Hour", true, None::<&str>)?,
                &MenuItem::with_id(app, PAUSE_TOMORROW_ID, "Until Tomorrow", true, None::<&str>)?,
            ],
        )?)
    };
    let toggles = reminders
        .iter()
        .map(|r| {
            CheckMenuItem::with_id(
                app,
                format!("{TOGGLE_PREFIX}{}", r.id),
                format!("{} {}", r.emoji, r.title),
                true,
                r.enabled,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;

    let separator = || PredefinedMenuItem::separator(app);
    let (sep1, sep2, sep3) = (separator()?, separator()?, separator()?);
    let test_popup = MenuItem::with_id(app, TEST_POPUP_ID, "Test Popup", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, SETTINGS_ID, "Settings…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit Macy Health", true, Some("CmdOrCtrl+Q"))?;

    let mut items: Vec<&dyn IsMenuItem<Wry>> = vec![&status, pause.as_ref(), &sep1];
    items.extend(toggles.iter().map(|t| t as &dyn IsMenuItem<Wry>));
    items.extend([
        &sep2 as &dyn IsMenuItem<Wry>,
        &test_popup,
        &settings,
        &sep3,
        &quit,
    ]);
    Menu::with_items(app, &items)
}

/// Rebuilds the menu after reminders change (titles, toggles, additions).
pub fn rebuild_menu(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match build_menu(app) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(err) => eprintln!("tray: failed to rebuild menu: {err}"),
    }
}

pub fn set_status(app: &AppHandle, text: &str) {
    if let Some(state) = app.try_state::<TrayStatus>() {
        if let Some(status) = state.0.lock().unwrap().as_ref() {
            let _ = status.set_text(text);
        }
    }
}

fn toggle_reminder(app: &AppHandle, id: &str) {
    let enabled = app
        .state::<scheduler::SchedulerState>()
        .lock()
        .unwrap()
        .get(id)
        .map(|r| r.enabled);
    if let Some(enabled) = enabled {
        if let Err(err) = settings::set_enabled(app, id, !enabled) {
            eprintln!("tray: {err}");
        }
    }
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Cycles through the built-in reminder kinds so each popup style can be checked.
fn show_test_popup(app: &AppHandle) {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    let mut reminder = crate::reminder::presets()[n % 3].to_popup();
    reminder.id = format!("test-{n}");
    popup::enqueue(app, reminder);
}
