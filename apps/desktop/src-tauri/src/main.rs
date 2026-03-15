#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};
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

fn navigate_to(app: &tauri::AppHandle, path: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let js = format!("window.location.hash = '{}'", path);
        let _ = window.eval(&js);
    }
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show_hide = MenuItem::with_id(app, "show_hide", "Show/Hide Window", true, None::<&str>)?;
    let tasks = MenuItem::with_id(app, "tasks", "Tasks", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_hide, &tasks, &separator, &quit])?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
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
                toggle_window_visibility(tray.app_handle());
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
    let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
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
            // borg://tasks/123 -> #/tasks/123
            if let Some(path) = url_str.strip_prefix("borg://") {
                let hash_path = format!("#/{}", path);
                navigate_to(&app_handle, &hash_path);
            }
        }
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            setup_tray(app.handle())?;
            setup_global_shortcut(app.handle());
            setup_deep_links(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
