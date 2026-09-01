mod commands;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

static APP_EXITING: AtomicBool = AtomicBool::new(false);
const TRAY_ICON_ID: &str = "main-tray";
const TRAY_MENU_SHOW_ID: &str = "tray-show";
const TRAY_MENU_HIDE_ID: &str = "tray-hide";
const TRAY_MENU_SETTINGS_ID: &str = "tray-settings";
const TRAY_MENU_EXIT_ID: &str = "tray-exit";

fn restore_main_window(app: &AppHandle) -> tauri::Result<()> {
    let _ = commands::launcher_restore_from_tray(app.clone());
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    Ok(())
}

fn open_settings_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show()?;
        let _ = window.center();
        let _ = window.set_focus();
        return Ok(());
    }

    let settings_window =
        WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings".into()))
            .title("設定")
            .inner_size(420.0, 220.0)
            .resizable(false)
            .center()
            .build()?;
    let _ = settings_window.set_focus();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let hide_item_visible =
                MenuItemBuilder::with_id(TRAY_MENU_HIDE_ID, "非表示").build(app)?;
            let settings_item_visible =
                MenuItemBuilder::with_id(TRAY_MENU_SETTINGS_ID, "設定").build(app)?;
            let exit_item_visible =
                MenuItemBuilder::with_id(TRAY_MENU_EXIT_ID, "終了").build(app)?;
            let visible_menu = MenuBuilder::new(app)
                .items(&[
                    &hide_item_visible,
                    &settings_item_visible,
                    &exit_item_visible,
                ])
                .build()?;

            let show_item_hidden =
                MenuItemBuilder::with_id(TRAY_MENU_SHOW_ID, "表示").build(app)?;
            let settings_item_hidden =
                MenuItemBuilder::with_id(TRAY_MENU_SETTINGS_ID, "設定").build(app)?;
            let exit_item_hidden =
                MenuItemBuilder::with_id(TRAY_MENU_EXIT_ID, "終了").build(app)?;
            let hidden_menu = MenuBuilder::new(app)
                .items(&[&show_item_hidden, &settings_item_hidden, &exit_item_hidden])
                .build()?;

            let mut tray_builder = TrayIconBuilder::with_id(TRAY_ICON_ID)
                .menu(&visible_menu)
                .show_menu_on_left_click(true)
                .on_tray_icon_event({
                    let visible_menu = visible_menu.clone();
                    let hidden_menu = hidden_menu.clone();
                    move |tray, event| {
                        if matches!(
                            event,
                            TrayIconEvent::Click {
                                button: MouseButton::Left,
                                ..
                            }
                        ) {
                            let should_show_visible_menu = tray
                                .app_handle()
                                .get_webview_window("main")
                                .and_then(|window| window.is_visible().ok())
                                .unwrap_or(false);
                            if should_show_visible_menu {
                                let _ = tray.set_menu(Some(visible_menu.clone()));
                            } else {
                                let _ = tray.set_menu(Some(hidden_menu.clone()));
                            }
                        }
                        if matches!(
                            event,
                            TrayIconEvent::DoubleClick {
                                button: MouseButton::Left,
                                ..
                            }
                        ) {
                            let app = tray.app_handle();
                            let _ = restore_main_window(&app);
                        }
                    }
                });

            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder.build(app)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_SHOW_ID => {
                let _ = restore_main_window(app);
            }
            TRAY_MENU_HIDE_ID => {
                let _ = commands::launcher_hide_to_tray(app.clone());
            }
            TRAY_MENU_SETTINGS_ID => {
                let _ = open_settings_window(app);
            }
            TRAY_MENU_EXIT_ID => {
                APP_EXITING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if APP_EXITING.load(Ordering::SeqCst) {
                    return;
                }
                api.prevent_close();
                let _ = commands::launcher_hide_to_tray(window.app_handle().clone());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::launcher_init,
            commands::launcher_start_tracking,
            commands::launcher_stop_tracking,
            commands::launcher_hide_to_tray,
            commands::launcher_restore_from_tray,
            commands::launcher_open_slot_editor,
            commands::settings_load,
            commands::settings_save,
            commands::slot_swap,
            commands::slot_update,
            commands::slot_clear,
            commands::slot_execute,
            commands::path_detect_data_type,
            commands::path_exists,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
