mod idle;
#[cfg(target_os = "macos")]
mod menu_style;
mod popup;
mod reminder;
mod scheduler;
mod settings;
mod sound;
mod tray;
mod updater;

use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just opens Settings in the running app.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_settings(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::SharedUpdater::default())
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
            settings::get_general,
            settings::save_general,
            settings::pause_reminders,
            settings::resume_reminders,
            updater::get_update_info,
            updater::check_for_updates,
            updater::install_update,
        ])
        .setup(|app| {
            // Menu-bar only: no Dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();
            let (reminders, general) = settings::load(handle);
            scheduler::init(handle, reminders, general.paused_until);
            settings::init(handle, general);
            popup::create(handle)?;
            tray::create(handle)?;
            scheduler::start_ticking(handle);
            updater::start_background_checks(handle);
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
