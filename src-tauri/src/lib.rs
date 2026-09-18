mod popup;
mod scheduler;
mod sound;
mod tray;

use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(popup::PopupState::default())
        .invoke_handler(tauri::generate_handler![
            popup::popup_current,
            popup::popup_action,
            sound::play_sound
        ])
        .setup(|app| {
            // Menu-bar only: no Dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            popup::create(app.handle())?;
            tray::create(app.handle())?;
            scheduler::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing Settings hides it; the app keeps running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
