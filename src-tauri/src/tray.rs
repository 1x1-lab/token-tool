use crate::types::{AppState, BalanceInfo, CodingPlanInfo};
use std::time::Instant;
use crate::utils::{format_amount, get_home_dir};
use tauri::{Emitter, Manager};

#[cfg(target_os = "macos")]
pub(crate) fn set_tray_highlight(app: &tauri::AppHandle, highlighted: bool) {
    use objc2::msg_send;
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.with_inner_tray_icon(move |inner| {
            if let Some(ns_status_item) = inner.ns_status_item() {
                let button: &objc2::runtime::AnyObject =
                    unsafe { msg_send![&*ns_status_item, button] };
                unsafe {
                    let _: () = msg_send![button, setHighlighted: highlighted];
                }
            }
        });
    }
}

pub(crate) fn show_popup(app: &tauri::AppHandle) -> Result<(), String> {
    // 窗口已在 tauri.conf.json 中预定义，直接获取
    let Some(window) = app.get_webview_window("tray-popup") else {
        return Err("tray-popup window not found".into());
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return Ok(());
    }

    // 定位失败不阻止窗口显示（Mac 重启后屏幕信息可能暂不可用）
    let _ = position_popup(app, &window);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit_to("tray-popup", "popup-shown", ());

    Ok(())
}

fn position_popup(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    // 获取弹出窗口实际尺寸
    let win_size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let popup_w = win_size.width as f64 / scale;
    let popup_h = win_size.height as f64 / scale;
    let gap = 4.0;

    // 获取屏幕工作区
    let monitor = if let Some(main_win) = app.get_webview_window("main") {
        main_win.primary_monitor().ok().flatten()
    } else {
        None
    };
    let (screen_w, screen_h) = if let Some(m) = &monitor {
        (
            m.size().width as f64 / m.scale_factor(),
            m.size().height as f64 / m.scale_factor(),
        )
    } else {
        (1920.0, 1080.0)
    };

    // 托盘图标位置
    let tray = app.tray_by_id("main-tray");
    let tray_rect = tray.as_ref().and_then(|t| t.rect().ok().flatten());

    let (tray_cx, tray_top, tray_bottom) = if let Some(rect) = &tray_rect {
        let (px, py, sw, sh) = match (rect.position, rect.size) {
            (tauri::Position::Physical(p), tauri::Size::Physical(s)) => {
                (p.x as f64, p.y as f64, s.width as f64, s.height as f64)
            }
            (tauri::Position::Logical(p), tauri::Size::Logical(s)) => {
                (p.x * scale, p.y * scale, s.width * scale, s.height * scale)
            }
            _ => return Ok(()),
        };
        // 图标水平中心、顶部、底部（逻辑坐标）
        (
            px / scale + sw / scale / 2.0,
            py / scale,
            (py + sh) / scale,
        )
    } else {
        // 无图标信息，默认屏幕右下角
        (screen_w - 16.0, screen_h - 64.0, screen_h - 16.0)
    };

    // 默认水平居中于图标
    let mut x = tray_cx - popup_w / 2.0;
    // 默认在图标上方
    let mut y = tray_top - popup_h - gap;

    // 边界修正：左侧溢出
    if x < 0.0 {
        x = gap;
    }
    // 右侧溢出
    if x + popup_w > screen_w {
        x = screen_w - popup_w - gap;
    }
    // 上方空间不足 → 改到图标下方
    if y < 0.0 {
        y = tray_bottom + gap;
    }
    // 下方也溢出（极端情况）
    if y + popup_h > screen_h {
        y = screen_h - popup_h - gap;
    }

    let _ = window.set_position(tauri::Position::Logical(
        tauri::LogicalPosition::new(x, y),
    ));

    Ok(())
}

#[tauri::command]
pub async fn update_tray_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    balance: Option<BalanceInfo>,
    coding_plan: Option<CodingPlanInfo>,
) -> Result<(), String> {
    {
        let mut tray_data = state.tray_data.lock().unwrap();
        if balance.is_some() {
            tray_data.balance = balance.clone();
        }
        if coding_plan.is_some() {
            tray_data.coding_plan = coding_plan.clone();
        }
        // 两个数据都有时写入 statusline 缓存（已移除，由 zhipukit-claude-code-plugin 自行管理）
    }
    // 更新 tooltip：余额 + 额度摘要
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tray_data = state.tray_data.lock().unwrap();
        let mut parts: Vec<String> = Vec::new();

        if let Some(ref b) = tray_data.balance {
            parts.push(format!("¥{}", format_amount(b.available_balance)));
        }
        if let Some(ref p) = tray_data.coding_plan {
            parts.push(format!("5h {}%", p.hour5_percentage));
            if p.weekly_percentage > 0 {
                parts.push(format!("周 {}%", p.weekly_percentage));
            }
            if p.mcp_total > 0 {
                parts.push(format!("MCP {}/{}", p.mcp_used, p.mcp_total));
            }
        }

        let tooltip = if parts.is_empty() {
            "ZhipuKit".to_string()
        } else {
            parts.join(" | ")
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
    Ok(())
}

/// 从配置文件读取 minimize_to_tray 设置
pub(crate) fn read_minimize_setting() -> Option<bool> {
    let home = get_home_dir().ok()?;
    let config_path = home.join(".claude").join("settings.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let raw: serde_json::Value = serde_json::from_str(&content).ok()?;
    raw.get("zhipuMinimizeToTray").and_then(|v| v.as_bool())
}

/// 保存 minimize_to_tray 设置到配置文件
fn save_minimize_setting(minimize: bool) -> Result<(), String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    let mut raw: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    raw["zhipuMinimizeToTray"] = serde_json::Value::Bool(minimize);

    let output =
        serde_json::to_string_pretty(&raw).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&config_path, output)
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn confirm_minimize_to_tray(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    minimize: bool,
) -> Result<(), String> {
    if minimize {
        *state.minimize_to_tray.lock().unwrap() = true;
        let _ = save_minimize_setting(true);
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        *state.main_hidden_at.lock().unwrap() = Some(Instant::now());
        #[cfg(target_os = "macos")]
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_minimize_to_tray(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    Ok(*state.minimize_to_tray.lock().unwrap())
}

#[tauri::command]
pub async fn set_minimize_to_tray(
    state: tauri::State<'_, AppState>,
    minimize: bool,
) -> Result<(), String> {
    *state.minimize_to_tray.lock().unwrap() = minimize;
    save_minimize_setting(minimize)?;
    Ok(())
}

#[tauri::command]
pub async fn exit_app(app: tauri::AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn start_window_drag(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.start_dragging().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_tray_popup_data(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let tray_data = state.tray_data.lock().unwrap();
    Ok(serde_json::json!({
        "balance": tray_data.balance,
        "coding_plan": tray_data.coding_plan,
    }))
}

#[tauri::command]
pub async fn resize_popup(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("tray-popup") {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
            width, height,
        )));
        // 尺寸变化后重新定位，避免超出屏幕
        position_popup(&app, &window)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn tray_show_main(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        // 隐藏超过 5 分钟则重新加载，防止 WebView 进程被 macOS 终止导致白屏
        let should_reload = {
            let hidden_at = state.main_hidden_at.lock().unwrap();
            hidden_at.map(|t: Instant| t.elapsed().as_secs() > 300).unwrap_or(false)
        };
        let _ = window.show();
        let _ = window.set_focus();
        if should_reload {
            if let Ok(url) = window.url() {
                let _ = window.navigate(url);
            }
        }
    }
    if let Some(popup) = app.get_webview_window("tray-popup") {
        let _ = popup.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn open_devtools(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
    }
}

#[tauri::command]
pub async fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "ZhipuKit",
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
        "family": std::env::consts::FAMILY,
    })
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("无法打开链接: {}", e))
}
