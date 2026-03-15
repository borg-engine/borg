#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, RunEvent, WindowEvent,
};
#[cfg(target_os = "macos")]
use tauri::menu::Submenu;
use tauri_plugin_global_shortcut::ShortcutState;

fn toggle_window_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn navigate_to(app: &tauri::AppHandle, path: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let js = format!("window.location.hash = '{}'", path);
        let _ = window.eval(&js);
    }
}

#[cfg(target_os = "macos")]
fn setup_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let app_menu = Submenu::with_items(
        app,
        "Borg",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Borg"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "preferences", "Preferences...", true, Some("CmdOrCtrl+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let menu = Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])?;
    Ok(menu)
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show_hide = MenuItem::with_id(app, "show_hide", "Show/Hide Window", true, None::<&str>)?;
    let tasks = MenuItem::with_id(app, "tasks", "Tasks", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Borg", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_hide, &tasks, &separator, &quit])?;

    let mut tray_builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu);
    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }
    let _tray = tray_builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show_hide" => toggle_window_visibility(app),
            "tasks" => navigate_to(app, "#/tasks"),
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                show_window(tray.app_handle());
            }
        })
        .tooltip("Borg")
        .build(app)?;

    Ok(())
}

fn setup_global_shortcut(app: &tauri::AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcut = if cfg!(target_os = "macos") {
        "CommandOrControl+Shift+B"
    } else {
        "Ctrl+Shift+B"
    };

    let app_handle = app.clone();
    let _ = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_window_visibility(&app_handle);
            }
        });
}

fn setup_deep_links(app: &tauri::AppHandle) {
    use tauri_plugin_deep_link::DeepLinkExt;

    let app_handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            let url_str = url.as_str();
            if let Some(path) = url_str.strip_prefix("borg://") {
                let hash_path = format!("#/{}", path);
                navigate_to(&app_handle, &hash_path);
            }
        }
    });
}

#[tauri::command]
fn navigate(app: tauri::AppHandle, path: String) {
    navigate_to(&app, &path);
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![navigate])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let menu = setup_menu(app.handle())?;
                app.set_menu(menu)?;
                app.on_menu_event(|app, event| {
                    if event.id.as_ref() == "preferences" {
                        navigate_to(app, "#/settings");
                    }
                });
            }

            setup_tray(app.handle())?;
            setup_global_shortcut(app.handle());
            setup_deep_links(app.handle());

            // Inject notification click handler into the webview
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(
                    r#"
                    if (window.__TAURI__) {
                        window.__TAURI__.event.listen('notification-clicked', (event) => {
                            if (event.payload && event.payload.path) {
                                window.location.hash = event.payload.path;
                            }
                        });
                    }
                    "#,
                );
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        match event {
            // Hide window on close instead of quitting (minimize to tray)
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                api.prevent_close();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            // Emit notification-clicked events to the webview for navigation
            RunEvent::Resumed => {
                let _ = app.emit("notification-clicked", serde_json::json!({}));
            }
            _ => {}
        }
    });
}
