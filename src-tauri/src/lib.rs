mod popup;
mod reminder;
mod scheduler;
mod settings;
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
            sound::play_sound,
            settings::get_reminders,
            settings::save_reminder,
            settings::delete_reminder,
            settings::set_reminder_enabled,
            settings::preview_reminder,
        ])
        .setup(|app| {
            // Menu-bar only: no Dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();
            scheduler::init(handle, settings::load(handle));
            popup::create(handle)?;
            tray::create(handle)?;
            scheduler::start_ticking(handle);
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
