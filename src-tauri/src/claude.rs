use crate::types::{ClaudeCodeConfig, ClaudeCodeStatus};
use crate::utils::{build_shell_command, get_home_dir, strip_ansi};

#[tauri::command]
pub async fn detect_claude_code() -> Result<ClaudeCodeStatus, String> {
    let config_path = get_home_dir()
        .ok()
        .map(|h| {
            h.join(".claude")
                .join("settings.json")
                .to_string_lossy()
                .to_string()
        });

    // macOS .app 不继承用户 shell PATH，需要用 login shell 执行
    // Windows: cmd /C 不需要 -c 参数，且需要隐藏控制台窗口
    let (shell, which_args, version_args) = if cfg!(windows) {
        (
            "cmd",
            vec!["/C", "where claude"],
            vec!["/C", "claude --version"],
        )
    } else {
        (
            "/bin/zsh",
            vec!["-l", "-c", "which claude"],
            vec!["-l", "-c", "claude --version"],
        )
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

    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

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
pub async fn read_claude_config() -> Result<ClaudeCodeConfig, String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Err("Claude Code 配置文件不存在".to_string());
    }

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;

    let raw: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析 JSON 失败: {}", e))?;

    let env = raw.get("env");

    // 检测无效插件引用
    let broken_plugins = find_broken_plugins(&raw, &home);

    Ok(ClaudeCodeConfig {
        model: raw
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from),
        anthropic_auth_token: env
            .and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN"))
            .and_then(|v| v.as_str())
            .map(String::from),
        anthropic_base_url: env
            .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
            .and_then(|v| v.as_str())
            .map(String::from),
        anthropic_default_haiku_model: env
            .and_then(|e| e.get("ANTHROPIC_DEFAULT_HAIKU_MODEL"))
            .and_then(|v| v.as_str())
            .map(String::from),
        anthropic_default_sonnet_model: env
            .and_then(|e| e.get("ANTHROPIC_DEFAULT_SONNET_MODEL"))
            .and_then(|v| v.as_str())
            .map(String::from),
        anthropic_default_opus_model: env
            .and_then(|e| e.get("ANTHROPIC_DEFAULT_OPUS_MODEL"))
            .and_then(|v| v.as_str())
            .map(String::from),
        api_timeout_ms: env
            .and_then(|e| e.get("API_TIMEOUT_MS"))
            .and_then(|v| v.as_str())
            .map(String::from),
        broken_plugins,
    })
}

#[tauri::command]
pub async fn save_claude_config(
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

    let mut raw: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析 JSON 失败: {}", e))?;

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

    let output =
        serde_json::to_string_pretty(&raw).map_err(|e| format!("序列化 JSON 失败: {}", e))?;

    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;

    Ok(())
}

/// 验证 enabledPlugins 中引用的插件是否有效安装
/// 返回需要移除的插件 key 列表
fn find_broken_plugins(
    settings: &serde_json::Value,
    home: &std::path::Path,
) -> Vec<String> {
    let Some(plugins) = settings
        .get("enabledPlugins")
        .and_then(|p| p.as_object())
    else {
        return vec![];
    };

    // 读取 installed_plugins.json 获取安装路径
    let installed_path = home
        .join(".claude")
        .join("plugins")
        .join("installed_plugins.json");
    let installed_content = std::fs::read_to_string(&installed_path).unwrap_or_default();
    let installed: serde_json::Value =
        serde_json::from_str(&installed_content).unwrap_or(serde_json::json!({}));
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
                    // 检查安装路径是否存在（目录存在即视为有效）
                    entry
                        .get("installPath")
                        .and_then(|p| p.as_str())
                        .map(|path| std::path::Path::new(path).exists())
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

    // 目录存在即视为有效安装（部分插件如 rust-analyzer-lsp 没有 plugin.json）
    plugin_dir.exists()
}

/// 清理 settings.json 中无效的插件引用，确保 SessionStart hook 不被插件加载失败阻塞
fn cleanup_broken_plugins(
    raw: &mut serde_json::Value,
    home: &std::path::Path,
) -> Vec<String> {
    let broken = find_broken_plugins(raw, home);

    if broken.is_empty() {
        return vec![];
    }

    if let Some(plugins) = raw
        .get_mut("enabledPlugins")
        .and_then(|p| p.as_object_mut())
    {
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
pub async fn setup_claude_hook(enabled: bool) -> Result<(), String> {
    let home = get_home_dir()?;

    // 解析 zhipukit-claude-code-plugin 二进制路径
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取路径失败: {}", e))?
        .parent()
        .ok_or("无法确定 exe 目录")?
        .to_path_buf();
    let status_bin = if cfg!(windows) {
        exe_dir.join("zhipukit-claude-code-plugin.exe")
    } else {
        exe_dir.join("zhipukit-claude-code-plugin")
    };

    if enabled && !status_bin.exists() {
        return Err(format!(
            "找不到 zhipukit-claude-code-plugin 二进制: {}",
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
        if let Some(session_hooks) = hooks
            .get_mut("SessionStart")
            .and_then(|v| v.as_array_mut())
        {
            session_hooks.retain(|entry| {
                if let Some(h) = entry.get("hooks").and_then(|h| h.as_array()) {
                    return !h.iter().any(|h| {
                        h.get("command")
                            .and_then(|c| c.as_str())
                            .map(|s| s.contains("zhipukit-claude-code-plugin"))
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

    let output =
        serde_json::to_string_pretty(&raw).map_err(|e| format!("序列化失败: {}", e))?;
    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn check_claude_hook_status() -> Result<serde_json::Value, String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Ok(serde_json::json!({ "installed": false }));
    }

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let raw: serde_json::Value =
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

    // 检查 statusLine 或旧的 SessionStart hook
    let has_statusline = raw
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .map(|s| s.contains("zhipukit-claude-code-plugin"))
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
                                .map(|s| s.contains("zhipukit-claude-code-plugin"))
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
pub async fn test_zhipukit_status() -> Result<String, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取路径失败: {}", e))?
        .parent()
        .ok_or("无法确定 exe 目录")?
        .to_path_buf();
    let status_bin = if cfg!(windows) {
        exe_dir.join("zhipukit-claude-code-plugin.exe")
    } else {
        exe_dir.join("zhipukit-claude-code-plugin")
    };

    if !status_bin.exists() {
        return Err(format!("找不到 zhipukit-claude-code-plugin: {}", status_bin.display()));
    }

    let mut cmd =
        tokio::process::Command::new(status_bin.to_string_lossy().to_string());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(crate::utils::CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("执行失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        // 解析 JSON 输出，提取 additionalContext 用于前端展示
        let display = if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
            json.get("hookSpecificOutput")
                .and_then(|o| o.get("additionalContext"))
                .and_then(|v| v.as_str())
                .unwrap_or(&stdout)
                .to_string()
        } else {
            stdout.trim().to_string()
        };
        Ok(strip_ansi(&display))
    } else {
        Err(format!("{}{}", stdout, stderr).trim().to_string())
    }
}

#[tauri::command]
pub async fn save_zhipu_endpoint(endpoint: String) -> Result<(), String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    let mut raw: serde_json::Value = if config_path.exists() {
        let content = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    raw["zhipuEndpoint"] = serde_json::Value::String(endpoint);

    let output =
        serde_json::to_string_pretty(&raw).map_err(|e| format!("序列化失败: {}", e))?;
    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn save_zhipu_cache_duration(seconds: u64) -> Result<(), String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    let mut raw: serde_json::Value = if config_path.exists() {
        let content = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    raw["zhipuCacheDuration"] = serde_json::Value::Number(seconds.into());

    let output =
        serde_json::to_string_pretty(&raw).map_err(|e| format!("序列化失败: {}", e))?;
    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

/// 段展示配置
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct SegmentConfig {
    pub tier: bool,
    pub balance: bool,
    pub git: bool,
    pub model: bool,
    pub context: bool,
    pub hour5: bool,
    pub mcp: bool,
    pub cache_time: bool,
}

impl Default for SegmentConfig {
    fn default() -> Self {
        Self {
            tier: true,
            balance: true,
            git: true,
            model: true,
            context: true,
            hour5: true,
            mcp: true,
            cache_time: true,
        }
    }
}

#[tauri::command]
pub async fn read_segment_config() -> Result<SegmentConfig, String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Ok(SegmentConfig::default());
    }

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let raw: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析 JSON 失败: {}", e))?;

    let seg = match raw.get("zhipuSegments").and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return Ok(SegmentConfig::default()),
    };

    let def = SegmentConfig::default();
    Ok(SegmentConfig {
        tier: seg.get("tier").and_then(|v| v.as_bool()).unwrap_or(def.tier),
        balance: seg.get("balance").and_then(|v| v.as_bool()).unwrap_or(def.balance),
        git: seg.get("git").and_then(|v| v.as_bool()).unwrap_or(def.git),
        model: seg.get("model").and_then(|v| v.as_bool()).unwrap_or(def.model),
        context: seg.get("context").and_then(|v| v.as_bool()).unwrap_or(def.context),
        hour5: seg.get("hour5").and_then(|v| v.as_bool()).unwrap_or(def.hour5),
        mcp: seg.get("mcp").and_then(|v| v.as_bool()).unwrap_or(def.mcp),
        cache_time: seg.get("cacheTime").and_then(|v| v.as_bool()).unwrap_or(def.cache_time),
    })
}

#[tauri::command]
pub async fn save_segment_config(config: SegmentConfig) -> Result<(), String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    let mut raw: serde_json::Value = if config_path.exists() {
        let content = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    raw["zhipuSegments"] = serde_json::json!({
        "tier": config.tier,
        "balance": config.balance,
        "git": config.git,
        "model": config.model,
        "context": config.context,
        "hour5": config.hour5,
        "mcp": config.mcp,
        "cacheTime": config.cache_time,
    });

    let output =
        serde_json::to_string_pretty(&raw).map_err(|e| format!("序列化失败: {}", e))?;
    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

/// 进度条颜色配置
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct BarColors {
    pub normal: String,
    pub warning: String,
    pub danger: String,
}

impl Default for BarColors {
    fn default() -> Self {
        Self {
            normal: "green".to_string(),
            warning: "yellow".to_string(),
            danger: "red".to_string(),
        }
    }
}

#[tauri::command]
pub async fn read_bar_colors() -> Result<BarColors, String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Ok(BarColors::default());
    }

    let content = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let raw: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析 JSON 失败: {}", e))?;

    let colors = match raw.get("zhipuBarColors").and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return Ok(BarColors::default()),
    };

    let def = BarColors::default();
    Ok(BarColors {
        normal: colors
            .get("normal")
            .and_then(|v| v.as_str())
            .unwrap_or(&def.normal)
            .to_string(),
        warning: colors
            .get("warning")
            .and_then(|v| v.as_str())
            .unwrap_or(&def.warning)
            .to_string(),
        danger: colors
            .get("danger")
            .and_then(|v| v.as_str())
            .unwrap_or(&def.danger)
            .to_string(),
    })
}

#[tauri::command]
pub async fn save_bar_colors(colors: BarColors) -> Result<(), String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    let mut raw: serde_json::Value = if config_path.exists() {
        let content = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if let Some(obj) = raw.as_object_mut() {
        obj.insert(
            "zhipuBarColors".to_string(),
            serde_json::json!({
                "normal": colors.normal,
                "warning": colors.warning,
                "danger": colors.danger,
            }),
        );
    }

    let output =
        serde_json::to_string_pretty(&raw).map_err(|e| format!("序列化失败: {}", e))?;
    tokio::fs::write(&config_path, output)
        .await
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}
