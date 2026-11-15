pub mod busybar;
mod calendar;
mod commands;
mod logging;
mod meeting_state;
mod poller;
mod scheduler;
mod screens;
mod settings;
mod state;
mod tray;
mod updater;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let log_guard = logging::init(app.handle())?;
            app.manage(log_guard);
            let settings = settings::load(app.handle())?;
            app.manage(state::AppState::new(
                settings.base_url,
                settings.ics_url,
                settings.busy_session_policy,
                settings.trigger_smart_home,
                settings.quiet_hours_start,
                settings.quiet_hours_end,
                settings.discreet_mode,
                settings.auto_start,
            ));
            tray::build(app, "Meeting Display — connexion…")?;
            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| match event {
                    tauri::WindowEvent::Focused(false) => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            if let Err(error) = window.hide() {
                                eprintln!("Could not hide blurred main window: {error}");
                            }
                        }
                    }
                    // Keep the app alive if the popover ever receives a close
                    // request (e.g. Alt+F4): hide it instead of destroying it.
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            if let Err(error) = window.hide() {
                                eprintln!("Could not hide closed main window: {error}");
                            }
                        }
                    }
                    _ => {}
                });
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(poller::run(handle));
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(calendar::run_poller(handle));
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(scheduler::run(handle));
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(updater::run(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_start,
            commands::send_key,
            commands::get_connection,
            commands::get_settings,
            commands::save_settings,
            commands::test_connection,
            commands::open_bar_ui,
            commands::get_autostart,
            commands::set_autostart,
            commands::get_meetings,
            commands::open_join_url,
            commands::save_calendar_settings,
            commands::confirm_meeting_start,
            commands::save_meeting_settings,
            commands::get_meeting_status,
            commands::start_meeting_now,
            commands::skip_meeting,
            commands::end_meeting,
            commands::restore_meetings
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // On exit, best-effort removal of our drawn elements so no
            // Meeting Display screen is left on the bar's screen after quitting.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<state::AppState>();
                // Stop the scheduler from drawing, wait for any in-flight draw
                // (shared draw lock), then remove our elements last.
                state.set_shutting_down();
                let base = state.base_url();
                tauri::async_runtime::block_on(async {
                    // Wait unconditionally for any in-flight draw to release the
                    // lock (bounded by the client's request timeout), so the final
                    // clear always runs last. The runtime is multi-threaded, so
                    // this does not deadlock the scheduler task.
                    let _guard = state.draw_lock.lock().await;
                    if let Ok(client) = busybar::BusyBarClient::new(&base) {
                        let _ = client.clear(screens::APP).await;
                        let current = state
                            .meeting_state
                            .lock()
                            .expect("meeting state poisoned")
                            .clone();
                        if let meeting_state::MeetingState::InMeeting {
                            saved_snapshot: Some(snapshot),
                            started_at,
                            ..
                        } = current
                        {
                            let _ = client
                                .put_snapshot(&meeting_state::restore_snapshot(
                                    &snapshot,
                                    chrono::Utc::now(),
                                    started_at,
                                ))
                                .await;
                        }
                    }
                });
            }
        });
}
