use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

const QUIT_ID: &str = "quit";
const START_ID: &str = "start";
const CHECK_ID: &str = "check";

pub fn build(app: &mut App, tooltip: &str) -> tauri::Result<()> {
    // The tray context menu is intentionally minimal: everything else (Start,
    // settings, opening the bar UI) lives in the popover opened by a left click.
    let quit = MenuItem::with_id(app, QUIT_ID, "Quitter", true, None::<&str>)?;
    let start = MenuItem::with_id(app, START_ID, "Démarrer la réunion", true, None::<&str>)?;
    let check = MenuItem::with_id(app, CHECK_ID, "Vérifier les mises à jour", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&start, &check, &quit])?;

    let icon = normal_icon()?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id().as_ref() == QUIT_ID {
                app.exit(0);
            } else if event.id().as_ref() == CHECK_ID {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::updater::check(&handle).await;
                    // The answer is in the panel: bring it up, it reads it when it gets focus.
                    let shown = handle
                        .get_webview_window("main")
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(false);
                    if !shown {
                        toggle(&handle);
                    }
                });
            } else if event.id().as_ref() == START_ID {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    // Armed meeting, or the next one ahead of time.
                    let _ = crate::commands::start_meeting_now(handle).await;
                });
            }
        })
        .on_tray_icon_event(|app, event| {
            tauri_plugin_positioner::on_tray_event(app.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle(app.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("Main window is unavailable");
        return;
    };
    match window.is_visible() {
        Ok(true) => {
            if let Err(error) = window.hide() {
                eprintln!("Could not hide main window: {error}");
            }
        }
        Ok(false) => {
            if let Err(error) = window.move_window(Position::TrayCenter) {
                eprintln!("Could not position main window: {error}");
                return;
            }
            if let Err(error) = window.show() {
                eprintln!("Could not show main window: {error}");
                return;
            }
            if let Err(error) = window.set_focus() {
                eprintln!("Could not focus main window: {error}");
            }
        }
        Err(error) => eprintln!("Could not inspect main window visibility: {error}"),
    }
}

pub fn refresh(app: &AppHandle, connected: bool, name: Option<&str>) {
    let Some(tray) = app.tray_by_id("main") else {
        eprintln!("Main tray icon is unavailable");
        return;
    };
    let tooltip = if connected {
        format!("Meeting Display — {} connectée", name.unwrap_or("Busy Bar"))
    } else {
        "Meeting Display — barre hors ligne".to_string()
    };
    if let Err(error) = tray.set_tooltip(Some(tooltip)) {
        eprintln!("Could not update tray tooltip: {error}");
    }
    let icon = if connected {
        normal_icon()
    } else {
        offline_icon()
    };
    match icon {
        Ok(icon) => {
            if let Err(error) = tray.set_icon(Some(icon)) {
                eprintln!("Could not update tray icon: {error}");
            }
        }
        Err(error) => eprintln!("Could not load tray icon: {error}"),
    }
}

fn normal_icon() -> tauri::Result<tauri::image::Image<'static>> {
    tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
}

fn offline_icon() -> tauri::Result<tauri::image::Image<'static>> {
    tauri::image::Image::from_bytes(include_bytes!("../icons/icon-offline.png"))
}
