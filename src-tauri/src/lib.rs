use std::sync::Mutex;
use tauri::{
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

mod api;
mod claude;
mod tray;
pub mod types;
pub mod utils;

use types::AppState;
use tray::show_popup;

#[cfg(target_os = "macos")]
use tray::set_tray_highlight;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .manage(AppState {
            client: reqwest::Client::new(),
            refresh_handle: Mutex::new(None),
            tray_data: Mutex::new(types::TrayData::default()),
            minimize_to_tray: Mutex::new(false),
        })
        .setup(|app| {
            // 开机自启时隐藏主窗口，直接后台运行
            let is_autostart = std::env::args().any(|a| a == "--autostart");
            if is_autostart {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                let state = app.state::<AppState>();
                *state.minimize_to_tray.lock().unwrap() = true;
                #[cfg(target_os = "macos")]
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .tooltip("ZhipuKit")
                .on_tray_icon_event(|tray, event| {
                    match event {
                        TrayIconEvent::Click { button_state, .. } => {
                            match button_state {
                                MouseButtonState::Down => {
                                    let app = tray.app_handle();
                                    let _ = show_popup(&app);
                                }
                                MouseButtonState::Up => {
                                    #[cfg(target_os = "macos")]
                                    set_tray_highlight(&tray.app_handle(), false);
                                }
                            }
                        }
                        TrayIconEvent::DoubleClick { .. } => {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            if let Some(popup) = app.get_webview_window("tray-popup") {
                                let _ = popup.hide();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // 监听主窗口关闭事件
            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let state = app_handle.state::<AppState>();
                        let minimize = state.minimize_to_tray.lock().unwrap();

                        if !*minimize {
                            // 首次关闭：阻止关闭，让前端弹窗确认
                            api.prevent_close();
                            let _ = app_handle.emit("confirm-minimize-to-tray", ());
                        } else {
                            // 已确认过，直接隐藏到托盘
                            api.prevent_close();
                            if let Some(w) = app_handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                            #[cfg(target_os = "macos")]
                            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                        }
                    }
                });
            }

            // popup 失焦或主窗口移动时自动隐藏
            if let Some(popup) = app.get_webview_window("tray-popup") {
                let app_handle = app.handle().clone();
                popup.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Focused(false) => {
                            if let Some(p) = app_handle.get_webview_window("tray-popup") {
                                let _ = p.hide();
                            }
                        }
                        _ => {}
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::query_balance,
            api::query_coding_plan,
            api::count_tokens,
            api::start_auto_refresh,
            api::stop_auto_refresh,
            tray::open_devtools,
            tray::get_app_info,
            tray::open_url,
            tray::update_tray_data,
            tray::confirm_minimize_to_tray,
            tray::exit_app,
            tray::start_window_drag,
            tray::get_tray_popup_data,
            tray::tray_show_main,
            tray::resize_popup,
            claude::detect_claude_code,
            claude::read_claude_config,
            claude::save_claude_config,
            claude::setup_claude_hook,
            claude::check_claude_hook_status,
            claude::test_zhipukit_status,
            claude::save_zhipu_endpoint,
            claude::save_zhipu_cache_duration,
            claude::read_segment_config,
            claude::save_segment_config,
            claude::read_bar_colors,
            claude::save_bar_colors
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
