use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 将查询结果写入 zhipukit-cache.json，供 zhipukit-status.exe (statusline) 复用
fn write_status_cache(balance: &BalanceInfo, plan: &CodingPlanInfo) {
    if let Ok(home) = get_home_dir() {
        let cache_path = home.join(".claude").join("zhipukit-cache.json");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // 格式化输出文本，与 zhipukit-status.exe 的格式保持一致
        let mut line1 = format!("ZhipuKit {}", plan.level.to_uppercase());
        line1.push_str(&format!(" | ¥{}", format_amount(balance.available_balance)));

        // 获取 git 分支
        let git_branch = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        if !git_branch.is_empty() {
            line1.push_str(&format!(" | Git ({})", git_branch));
        }

        let mut quota_parts: Vec<String> = Vec::new();

        // 5h 额度
        if plan.hour5_percentage > 0 {
            let pct = plan.hour5_percentage;
            let bar = format_status_bar(pct, 8);
            let mut s = format!("5h {}{} {}", bar, "%", pct);
            if plan.hour5_next_reset > 0 {
                let remaining_ms = (plan.hour5_next_reset - now).max(0);
                let elapsed_ms = (5 * 3600 * 1000 - remaining_ms).max(0);
                s.push_str(&format!(" ({}/5h)", format_remaining(elapsed_ms)));
            }
            quota_parts.push(s);
        }

        // MCP
        if plan.mcp_total > 0 {
            let pct = (plan.mcp_used * 100 / plan.mcp_total).min(100);
            let bar = format_status_bar(pct, 8);
            let mut s = format!("MCP {}", bar);
            let mut time_info = format!("{}/{}", plan.mcp_used, plan.mcp_total);
            if plan.mcp_next_reset > 0 {
                let remaining_ms = (plan.mcp_next_reset - now).max(0);
                let elapsed_ms = (30 * 24 * 3600 * 1000 - remaining_ms).max(0);
                let d = elapsed_ms / (24 * 3600 * 1000);
                let h = (elapsed_ms % (24 * 3600 * 1000)) / (3600 * 1000);
                time_info.push_str(&format!(" | {}d {}h/30d", d, h));
            }
            s.push_str(&format!(" ({})", time_info));
            quota_parts.push(s);
        }

        let output = if quota_parts.is_empty() {
            line1
        } else {
            format!("{}\n{}", line1, quota_parts.join(" | "))
        };

        let _ = std::fs::write(&cache_path, serde_json::json!({
            "cached_at": now,
            "output": output
        }).to_string());
    }
}

/// ANSI 彩色进度条（与 zhipukit-status.exe 保持一致）
fn format_status_bar(percentage: i64, length: usize) -> String {
    let pct = (percentage.clamp(0, 100) as f64) / 100.0;
    let filled = (pct * length as f64).round() as usize;
    let empty = length - filled;
    let color = if percentage >= 70 {
        "\x1b[31m"
    } else if percentage >= 50 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    };
    let reset = "\x1b[0m";
    format!("{}{}{}{}", color, "█".repeat(filled), reset, "░".repeat(empty))
}

/// 格式化剩余时间
fn format_remaining(ms: i64) -> String {
    let secs = ms / 1000;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{}h {}m", h, m)
    } else {
        format!("{}m", m)
    }
}

/// 创建 shell 命令，Windows 上隐藏控制台窗口
fn build_shell_command(program: &str, args: &[&str]) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BalanceInfo {
    pub balance: f64,
    pub recharge_amount: f64,
    pub give_amount: f64,
    pub total_spend_amount: f64,
    pub frozen_balance: f64,
    pub available_balance: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TokenCountResult {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CodingQuotaLimit {
    pub limit_type: String,
    pub percentage: i64,
    pub usage: i64,
    pub current_value: i64,
    pub remaining: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CodingPlanInfo {
    pub level: String,
    pub hour5_percentage: i64,
    pub hour5_next_reset: i64,
    pub weekly_percentage: i64,
    pub weekly_next_reset: i64,
    pub mcp_total: i64,
    pub mcp_used: i64,
    pub mcp_remaining: i64,
    pub mcp_next_reset: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClaudeCodeStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClaudeCodeConfig {
    pub model: Option<String>,
    pub anthropic_auth_token: Option<String>,
    pub anthropic_base_url: Option<String>,
    pub anthropic_default_haiku_model: Option<String>,
    pub anthropic_default_sonnet_model: Option<String>,
    pub anthropic_default_opus_model: Option<String>,
    pub api_timeout_ms: Option<String>,
    pub broken_plugins: Vec<String>,
}

fn get_home_dir() -> Result<std::path::PathBuf, String> {
    if cfg!(windows) {
        std::env::var("USERPROFILE")
            .map(std::path::PathBuf::from)
            .map_err(|_| "Cannot determine home directory".to_string())
    } else {
        std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .map_err(|_| "Cannot determine home directory".to_string())
    }
}

#[tauri::command]
async fn detect_claude_code() -> Result<ClaudeCodeStatus, String> {
    let config_path = get_home_dir()
        .ok()
        .map(|h| h.join(".claude").join("settings.json").to_string_lossy().to_string());

    // macOS .app 不继承用户 shell PATH，需要用 login shell 执行
    // Windows: cmd /C 不需要 -c 参数，且需要隐藏控制台窗口
    let (shell, which_args, version_args) = if cfg!(windows) {
        ("cmd", vec!["/C", "where claude"], vec!["/C", "claude --version"])
    } else {
        ("/bin/zsh", vec!["-l", "-c", "which claude"], vec!["-l", "-c", "claude --version"])
    };

    let output = build_shell_command(shell, &which_args)
        .output()
        .await
        .map_err(|e| format!("检测失败: {}", e))?;

    if !output.status.success() {
        return Ok(ClaudeCodeStatus {
            installed: false,
            version: None,
            path: None,
            config_path,
        });
    }

    let path = String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("").trim().to_string();

    let version_output = build_shell_command(shell, &version_args)
        .output()
        .await
        .ok();

    let version = version_output.and_then(|o| {
        if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else {
            None
        }
    });

    Ok(ClaudeCodeStatus {
        installed: true,
        version,
        path: Some(path),
        config_path,
    })
}

#[tauri::command]
async fn read_claude_config() -> Result<ClaudeCodeConfig, String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Err("Claude Code 配置文件不存在".to_string());
    }

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;

    let raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析 JSON 失败: {}", e))?;

    let env = raw.get("env");

    // 检测无效插件引用
    let broken_plugins = find_broken_plugins(&raw, &home);

    Ok(ClaudeCodeConfig {
        model: raw.get("model").and_then(|v| v.as_str()).map(String::from),
        anthropic_auth_token: env.and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN")).and_then(|v| v.as_str()).map(String::from),
        anthropic_base_url: env.and_then(|e| e.get("ANTHROPIC_BASE_URL")).and_then(|v| v.as_str()).map(String::from),
        anthropic_default_haiku_model: env.and_then(|e| e.get("ANTHROPIC_DEFAULT_HAIKU_MODEL")).and_then(|v| v.as_str()).map(String::from),
        anthropic_default_sonnet_model: env.and_then(|e| e.get("ANTHROPIC_DEFAULT_SONNET_MODEL")).and_then(|v| v.as_str()).map(String::from),
        anthropic_default_opus_model: env.and_then(|e| e.get("ANTHROPIC_DEFAULT_OPUS_MODEL")).and_then(|v| v.as_str()).map(String::from),
        api_timeout_ms: env.and_then(|e| e.get("API_TIMEOUT_MS")).and_then(|v| v.as_str()).map(String::from),
        broken_plugins,
    })
}

#[tauri::command]
async fn save_claude_config(
    model: Option<String>,
    anthropic_auth_token: Option<String>,
    anthropic_base_url: Option<String>,
    anthropic_default_haiku_model: Option<String>,
    anthropic_default_sonnet_model: Option<String>,
    anthropic_default_opus_model: Option<String>,
    api_timeout_ms: Option<String>,
) -> Result<(), String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;

    let mut raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析 JSON 失败: {}", e))?;

    // Update model
    if let Some(ref v) = model {
        raw["model"] = serde_json::Value::String(v.clone());
    }

    // Ensure env object exists
    if raw.get("env").is_none() {
        raw["env"] = serde_json::Value::Object(Default::default());
    }

    if let Some(ref v) = anthropic_auth_token {
        raw["env"]["ANTHROPIC_AUTH_TOKEN"] = serde_json::Value::String(v.clone());
    }
    if let Some(ref v) = anthropic_base_url {
        raw["env"]["ANTHROPIC_BASE_URL"] = serde_json::Value::String(v.clone());
    }
    if let Some(ref v) = anthropic_default_haiku_model {
        raw["env"]["ANTHROPIC_DEFAULT_HAIKU_MODEL"] = serde_json::Value::String(v.clone());
    }
    if let Some(ref v) = anthropic_default_sonnet_model {
        raw["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"] = serde_json::Value::String(v.clone());
    }
    if let Some(ref v) = anthropic_default_opus_model {
        raw["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"] = serde_json::Value::String(v.clone());
    }
    if let Some(ref v) = api_timeout_ms {
        raw["env"]["API_TIMEOUT_MS"] = serde_json::Value::String(v.clone());
    }

    let output = serde_json::to_string_pretty(&raw)
        .map_err(|e| format!("序列化 JSON 失败: {}", e))?;

    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;

    Ok(())
}

/// 验证 enabledPlugins 中引用的插件是否有效安装
/// 返回需要移除的插件 key 列表
fn find_broken_plugins(settings: &serde_json::Value, home: &std::path::Path) -> Vec<String> {
    let Some(plugins) = settings.get("enabledPlugins").and_then(|p| p.as_object()) else {
        return vec![];
    };

    // 读取 installed_plugins.json 获取安装路径
    let installed_path = home.join(".claude").join("plugins").join("installed_plugins.json");
    let installed_content = std::fs::read_to_string(&installed_path).unwrap_or_default();
    let installed: serde_json::Value = serde_json::from_str(&installed_content).unwrap_or(serde_json::json!({}));
    let installed_plugins = installed.get("plugins").and_then(|p| p.as_object());

    let mut broken = Vec::new();

    for (key, enabled) in plugins {
        // 只检查启用的插件
        if !enabled.as_bool().unwrap_or(false) {
            continue;
        }

        let is_valid = installed_plugins
            .and_then(|map| map.get(key))
            .and_then(|entries| entries.as_array())
            .map(|entries| {
                entries.iter().any(|entry| {
                    // 检查安装路径是否存在且包含 plugin.json
                    entry.get("installPath")
                        .and_then(|p| p.as_str())
                        .map(|path| {
                            let install_dir = std::path::Path::new(path);
                            install_dir.exists()
                                && (install_dir.join(".claude-plugin").join("plugin.json").exists()
                                    || install_dir.join("plugin.json").exists())
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if !is_valid {
            // 再检查 marketplace 目录中是否有完整安装
            let marketplace_valid = check_marketplace_plugin(key, home);
            if !marketplace_valid {
                broken.push(key.clone());
            }
        }
    }

    broken
}

/// 检查 marketplace 中是否有该插件的完整安装
fn check_marketplace_plugin(key: &str, home: &std::path::Path) -> bool {
    // key 格式: "plugin-name@marketplace-name"
    let parts: Vec<&str> = key.splitn(2, '@').collect();
    if parts.len() != 2 {
        return false;
    }
    let (plugin_name, marketplace) = (parts[0], parts[1]);

    let plugin_dir = home
        .join(".claude")
        .join("plugins")
        .join("marketplaces")
        .join(marketplace)
        .join("plugins")
        .join(plugin_name);

    if !plugin_dir.exists() {
        return false;
    }

    // 必须有 .claude-plugin/plugin.json 才算完整
    plugin_dir.join(".claude-plugin").join("plugin.json").exists()
}

/// 清理 settings.json 中无效的插件引用，确保 SessionStart hook 不被插件加载失败阻塞
fn cleanup_broken_plugins(raw: &mut serde_json::Value, home: &std::path::Path) -> Vec<String> {
    let broken = find_broken_plugins(raw, home);

    if broken.is_empty() {
        return vec![];
    }

    if let Some(plugins) = raw.get_mut("enabledPlugins").and_then(|p| p.as_object_mut()) {
        for key in &broken {
            plugins.remove(key);
        }
        // 如果 enabledPlugins 变空了，移除整个字段
        if plugins.is_empty() {
            if let Some(obj) = raw.as_object_mut() {
                obj.remove("enabledPlugins");
            }
        }
    }

    broken
}

#[tauri::command]
async fn setup_claude_hook(enabled: bool) -> Result<(), String> {
    let home = get_home_dir()?;

    // 解析 zhipukit-status 二进制路径
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取路径失败: {}", e))?
        .parent()
        .ok_or("无法确定 exe 目录")?
        .to_path_buf();
    let status_bin = if cfg!(windows) {
        exe_dir.join("zhipukit-status.exe")
    } else {
        exe_dir.join("zhipukit-status")
    };

    if enabled && !status_bin.exists() {
        return Err(format!(
            "找不到 zhipukit-status 二进制: {}",
            status_bin.display()
        ));
    }

    let config_path = home.join(".claude").join("settings.json");
    let mut raw: serde_json::Value = if config_path.exists() {
        let content = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // 容错：清理无效的插件引用
    let broken = cleanup_broken_plugins(&mut raw, &home);
    if !broken.is_empty() {
        log::warn!("已清理无效插件引用: {:?}", broken);
    }

    // 清理旧的 SessionStart hook（兼容从旧版本升级）
    if let Some(hooks) = raw.get_mut("hooks") {
        if let Some(session_hooks) = hooks.get_mut("SessionStart").and_then(|v| v.as_array_mut()) {
            session_hooks.retain(|entry| {
                if let Some(h) = entry.get("hooks").and_then(|h| h.as_array()) {
                    return !h.iter().any(|h| {
                        h.get("command")
                            .and_then(|c| c.as_str())
                            .map(|s| s.contains("zhipukit-status"))
                            .unwrap_or(false)
                    });
                }
                true
            });
            // 如果 SessionStart 变空了，清理
            if session_hooks.is_empty() {
                if let Some(hooks_obj) = hooks.as_object_mut() {
                    hooks_obj.remove("SessionStart");
                    if hooks_obj.is_empty() {
                        if let Some(obj) = raw.as_object_mut() {
                            obj.remove("hooks");
                        }
                    }
                }
            }
        }
    }

    if enabled {
        // 设置 statusLine：Claude Code 会周期性调用此命令，将 stdout 渲染到输入框下方
        let command_path = status_bin.to_string_lossy().to_string().replace('\\', "/");
        raw["statusLine"] = serde_json::json!({
            "type": "command",
            "command": command_path
        });
    } else {
        // 移除 statusLine
        if let Some(obj) = raw.as_object_mut() {
            obj.remove("statusLine");
        }
    }

    let output = serde_json::to_string_pretty(&raw)
        .map_err(|e| format!("序列化失败: {}", e))?;
    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn check_claude_hook_status() -> Result<serde_json::Value, String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Ok(serde_json::json!({ "installed": false }));
    }

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let raw: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or(serde_json::json!({}));

    // 检查 statusLine 或旧的 SessionStart hook
    let has_statusline = raw
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .map(|s| s.contains("zhipukit-status"))
        .unwrap_or(false);

    let has_hook = raw
        .get("hooks")
        .and_then(|h| h.get("SessionStart"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hooks| {
                        hooks.iter().any(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .map(|s| s.contains("zhipukit-status"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    Ok(serde_json::json!({ "installed": has_statusline || has_hook }))
}

#[tauri::command]
async fn test_zhipukit_status() -> Result<String, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取路径失败: {}", e))?
        .parent()
        .ok_or("无法确定 exe 目录")?
        .to_path_buf();
    let status_bin = if cfg!(windows) {
        exe_dir.join("zhipukit-status.exe")
    } else {
        exe_dir.join("zhipukit-status")
    };

    if !status_bin.exists() {
        return Err(format!("找不到 zhipukit-status: {}", status_bin.display()));
    }

    let mut cmd = tokio::process::Command::new(status_bin.to_string_lossy().to_string());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("执行失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        // 解析 JSON 输出，提取 additionalContext 用于前端展示
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
            let ctx = json
                .get("hookSpecificOutput")
                .and_then(|o| o.get("additionalContext"))
                .and_then(|v| v.as_str())
                .unwrap_or(&stdout);
            Ok(ctx.to_string())
        } else {
            Ok(stdout.trim().to_string())
        }
    } else {
        Err(format!("{}{}", stdout, stderr).trim().to_string())
    }
}

#[derive(Default)]
struct TrayData {
    balance: Option<BalanceInfo>,
    coding_plan: Option<CodingPlanInfo>,
}

struct AppState {
    client: reqwest::Client,
    refresh_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    tray_data: Mutex<TrayData>,
    minimize_to_tray: Mutex<bool>,
}

#[tauri::command]
async fn query_balance(state: tauri::State<'_, AppState>, api_key: String) -> Result<BalanceInfo, String> {
    let client = state.client.clone();
    log::info!("查询余额...");
    let resp = client
        .get("https://www.bigmodel.cn/api/biz/account/query-customer-account-report")
        .header("Authorization", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| {
            log::error!("余额查询请求失败: {}", e);
            format!("请求失败: {}", e)
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        log::error!("余额查询 API 错误 ({}): {}", status, body);
        return Err(format!("API 返回错误 ({}): {}", status, body));
    }

    log::info!("余额查询成功");

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let data = if let Some(d) = json.get("data") {
        d
    } else if json.get("balance").is_some() || json.get("availableBalance").is_some() {
        &json
    } else {
        return Err(format!("未知的响应格式: {}", json));
    };

    let info = BalanceInfo {
        balance: data
            .get("balance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        recharge_amount: data
            .get("rechargeAmount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        give_amount: data
            .get("giveAmount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        total_spend_amount: data
            .get("totalSpendAmount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        frozen_balance: data
            .get("frozenBalance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        available_balance: data
            .get("availableBalance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
    };

    {
        let mut td = state.tray_data.lock().unwrap();
        let existing_plan = td.coding_plan.clone();
        *td = TrayData {
            balance: Some(info.clone()),
            coding_plan: existing_plan,
        };
    }

    Ok(info)
}

#[tauri::command]
async fn query_coding_plan(state: tauri::State<'_, AppState>, api_key: String) -> Result<CodingPlanInfo, String> {
    let client = state.client.clone();
    log::info!("查询 Coding Plan...");
    let resp = client
        .get("https://open.bigmodel.cn/api/monitor/usage/quota/limit")
        .header("Authorization", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| {
            log::error!("Coding Plan 查询请求失败: {}", e);
            format!("请求失败: {}", e)
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        log::error!("Coding Plan API 错误 ({}): {}", status, body);
        return Err(format!("API 返回错误 ({}): {}", status, body));
    }

    log::info!("Coding Plan 查询成功");

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if !success {
        let msg = json
            .get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        return Err(format!("查询失败: {}", msg));
    }

    let data = json
        .get("data")
        .ok_or_else(|| format!("响应中无 data 字段: {}", json))?;

    let level = data
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let limits = data
        .get("limits")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // 解析 limits: TIME_LIMIT (MCP月度), TOKENS_LIMIT (5小时 & 每周)
    let mut hour5_percentage: i64 = 0;
    let mut hour5_next_reset: i64 = 0;
    let mut weekly_percentage: i64 = 0;
    let mut weekly_next_reset: i64 = 0;
    let mut mcp_total: i64 = 0;
    let mut mcp_used: i64 = 0;
    let mut mcp_remaining: i64 = 0;
    let mut mcp_next_reset: i64 = 0;
    let mut tokens_limit_count = 0;

    for limit in &limits {
        let limit_type = limit
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let percentage = limit
            .get("percentage")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let next_reset = limit
            .get("nextResetTime")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        match limit_type {
            "TIME_LIMIT" => {
                mcp_total = limit.get("usage").and_then(|v| v.as_i64()).unwrap_or(0);
                mcp_used = limit.get("currentValue").and_then(|v| v.as_i64()).unwrap_or(0);
                mcp_remaining = limit.get("remaining").and_then(|v| v.as_i64()).unwrap_or(0);
                mcp_next_reset = next_reset;
            }
            "TOKENS_LIMIT" => {
                if tokens_limit_count == 0 {
                    hour5_percentage = percentage;
                    hour5_next_reset = next_reset;
                } else {
                    weekly_percentage = percentage;
                    weekly_next_reset = next_reset;
                }
                tokens_limit_count += 1;
            }
            _ => {}
        }
    }

    let plan = CodingPlanInfo {
        level,
        hour5_percentage,
        hour5_next_reset,
        weekly_percentage,
        weekly_next_reset,
        mcp_total,
        mcp_used,
        mcp_remaining,
        mcp_next_reset,
    };

    {
        let mut td = state.tray_data.lock().unwrap();
        let existing_balance = td.balance.clone();
        *td = TrayData {
            balance: existing_balance,
            coding_plan: Some(plan.clone()),
        };
    }

    Ok(plan)
}

#[tauri::command]
async fn count_tokens(state: tauri::State<'_, AppState>, api_key: String, text: String, model: String) -> Result<TokenCountResult, String> {
    let client = state.client.clone();

    log::info!("Token 计算 (model={}): {}", model, if text.len() > 50 { format!("{}...", &text[..50]) } else { text.clone() });
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": text
            }
        ],
        "max_tokens": 1
    });

    let resp = client
        .post("https://open.bigmodel.cn/api/paas/v4/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            log::error!("Token 计算请求失败: {}", e);
            format!("请求失败: {}", e)
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        log::error!("Token 计算 API 错误 ({}): {}", status, body);
        return Err(format!("API 返回错误 ({}): {}", status, body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let usage = json
        .get("usage")
        .ok_or_else(|| format!("响应中无 usage 字段: {}", json))?;

    let result = TokenCountResult {
        prompt_tokens: usage
            .get("prompt_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        completion_tokens: usage
            .get("completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        total_tokens: usage
            .get("total_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
    };
    log::info!("Token 计算完成: prompt={}, completion={}, total={}", result.prompt_tokens, result.completion_tokens, result.total_tokens);
    Ok(result)
}

#[tauri::command]
async fn start_auto_refresh(app: tauri::AppHandle, state: tauri::State<'_, AppState>, api_key: String, interval_secs: u64) -> Result<(), String> {
    // 停止旧任务
    if let Some(h) = state.refresh_handle.lock().unwrap().take() {
        h.abort();
    }

    let client = state.client.clone();
    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;
            log::info!("[自动刷新] 开始轮询...");

            let balance_resp = client
                .get("https://www.bigmodel.cn/api/biz/account/query-customer-account-report")
                .header("Authorization", &api_key)
                .header("Content-Type", "application/json")
                .send()
                .await;

            match &balance_resp {
                Ok(resp) if resp.status().is_success() => log::info!("[自动刷新] 余额查询成功"),
                Ok(resp) => log::warn!("[自动刷新] 余额查询返回 {}", resp.status()),
                Err(e) => log::error!("[自动刷新] 余额查询失败: {}", e),
            }

            if let Ok(resp) = balance_resp {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let data = json.get("data").cloned().unwrap_or(json);
                    let _ = app.emit("balance-update", &data);
                }
            }

            let plan_resp = client
                .get("https://open.bigmodel.cn/api/monitor/usage/quota/limit")
                .header("Authorization", &api_key)
                .header("Content-Type", "application/json")
                .send()
                .await;

            match &plan_resp {
                Ok(resp) if resp.status().is_success() => log::info!("[自动刷新] Coding Plan 查询成功"),
                Ok(resp) => log::warn!("[自动刷新] Coding Plan 查询返回 {}", resp.status()),
                Err(e) => log::error!("[自动刷新] Coding Plan 查询失败: {}", e),
            }

            if let Ok(resp) = plan_resp {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                        if let Some(data) = json.get("data").cloned() {
                            let _ = app.emit("plan-update", &data);
                        }
                    }
                }
            }
        }
    });

    *state.refresh_handle.lock().unwrap() = Some(handle);
    Ok(())
}

#[tauri::command]
async fn stop_auto_refresh(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if let Some(h) = state.refresh_handle.lock().unwrap().take() {
        h.abort();
    }
    Ok(())
}

#[tauri::command]
async fn open_devtools(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(debug_assertions)]
        window.open_devtools();
        #[cfg(not(debug_assertions))]
        {
            let _ = window;
        }
    }
}

#[tauri::command]
async fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "ZhipuKit",
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
        "family": std::env::consts::FAMILY,
    })
}

#[tauri::command]
async fn update_tray_data(app: tauri::AppHandle, state: tauri::State<'_, AppState>, balance: Option<BalanceInfo>, coding_plan: Option<CodingPlanInfo>) -> Result<(), String> {
    {
        let mut tray_data = state.tray_data.lock().unwrap();
        if balance.is_some() {
            tray_data.balance = balance.clone();
        }
        if coding_plan.is_some() {
            tray_data.coding_plan = coding_plan.clone();
        }
        // 两个数据都有时写入 statusline 缓存
        if let (Some(ref b), Some(ref p)) = (tray_data.balance, tray_data.coding_plan) {
            write_status_cache(b, p);
        }
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

#[tauri::command]
async fn confirm_minimize_to_tray(app: tauri::AppHandle, state: tauri::State<'_, AppState>, minimize: bool) -> Result<(), String> {
    if minimize {
        *state.minimize_to_tray.lock().unwrap() = true;
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        #[cfg(target_os = "macos")]
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
    Ok(())
}

#[tauri::command]
async fn exit_app(app: tauri::AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn start_window_drag(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.start_dragging().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn get_tray_popup_data(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let tray_data = state.tray_data.lock().unwrap();
    Ok(serde_json::json!({
        "balance": tray_data.balance,
        "coding_plan": tray_data.coding_plan,
    }))
}

#[tauri::command]
async fn resize_popup(app: tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("tray-popup") {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)));
        // 尺寸变化后重新定位，避免超出屏幕
        position_popup(&app, &window)?;
    }
    Ok(())
}

#[tauri::command]
async fn tray_show_main(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Some(popup) = app.get_webview_window("tray-popup") {
        let _ = popup.hide();
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_tray_highlight(app: &tauri::AppHandle, highlighted: bool) {
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

fn show_popup(app: &tauri::AppHandle) -> Result<(), String> {
    // 窗口已在 tauri.conf.json 中预定义，直接获取
    let Some(window) = app.get_webview_window("tray-popup") else {
        return Err("tray-popup window not found".into());
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return Ok(());
    }

    position_popup(app, &window)?;
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit_to("tray-popup", "popup-shown", ());

    Ok(())
}

fn position_popup(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> Result<(), String> {
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
        (m.size().width as f64 / m.scale_factor(), m.size().height as f64 / m.scale_factor())
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
        (px / scale + sw / scale / 2.0, py / scale, (py + sh) / scale)
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

    let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));

    Ok(())
}

fn format_amount(v: f64) -> String {
    if v == v.floor() {
        format!("{}", v as i64)
    } else {
        format!("{:.4}", v).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

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
            tray_data: Mutex::new(TrayData::default()),
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
            query_balance,
            query_coding_plan,
            count_tokens,
            start_auto_refresh,
            stop_auto_refresh,
            open_devtools,
            get_app_info,
            update_tray_data,
            confirm_minimize_to_tray,
            exit_app,
            start_window_drag,
            get_tray_popup_data,
            tray_show_main,
            resize_popup,
            detect_claude_code,
            read_claude_config,
            save_claude_config,
            setup_claude_hook,
            check_claude_hook_status,
            test_zhipukit_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
