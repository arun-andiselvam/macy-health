use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, Wry,
};

pub const TRAY_ID: &str = "main";
pub const STATUS_ITEM_ID: &str = "status";

const SETTINGS_ID: &str = "settings";
const QUIT_ID: &str = "quit";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Macy Health")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SETTINGS_ID => show_settings(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    // Status line is updated by the scheduler (Phase 3).
    let status = MenuItem::with_id(
        app,
        STATUS_ITEM_ID,
        "No reminders scheduled",
        false,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, SETTINGS_ID, "Settings…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit Macy Health", true, Some("CmdOrCtrl+Q"))?;

    Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
