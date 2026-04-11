// zhipukit-status: Claude Code statusline 工具
// 读取 ~/.claude/settings.json 中的 ANTHROPIC_AUTH_TOKEN，查询智谱 API，输出套餐信息到 stdout
// 支持 statusline 模式（缓存 5 分钟）和独立测试模式

use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

fn get_home_dir() -> Result<PathBuf, String> {
    if cfg!(windows) {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .map_err(|_| "Cannot determine home directory".to_string())
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| "Cannot determine home directory".to_string())
    }
}

fn format_amount(v: f64) -> String {
    if v == v.floor() {
        format!("{}", v as i64)
    } else {
        format!("{:.4}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// ANSI 颜色进度条，已用部分根据百分比变色，不带百分比文字
fn progress_bar(percentage: i64, length: usize) -> String {
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

/// 带 ANSI 颜色进度条 + 百分比
fn progress_bar_pct(percentage: i64, length: usize) -> String {
    let bar = progress_bar(percentage, length);
    format!("{} {}%", bar, percentage)
}

/// 格式化剩余时间：将毫秒时间戳差值转为 "Xh Xm" 格式
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

/// 检测是否有 stdin 管道输入（Claude Code statusline 会通过管道传数据）
fn has_stdin_data() -> bool {
    !io::stdin().is_terminal()
}

/// 读取并丢弃 stdin 数据（Claude Code statusline 会传 JSON，我们不需要）
fn drain_stdin() {
    if has_stdin_data() {
        let _ = io::stdin().lock().read_to_end(&mut Vec::new());
    }
}

/// 缓存文件路径
fn cache_path() -> Result<PathBuf, String> {
    Ok(get_home_dir()?.join(".claude").join("zhipukit-cache.json"))
}

/// 尝试读取缓存（5 分钟内有效），返回缓存的输出文本
fn read_cache() -> Option<String> {
    let path = cache_path().ok()?;
    if !path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let cached_at = json.get("cached_at").and_then(|v| v.as_i64())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as i64;

    // 缓存有效期 5 分钟
    if now - cached_at > 5 * 60 * 1000 {
        return None;
    }

    json.get("output").and_then(|v| v.as_str()).map(String::from)
}

/// 写入缓存
fn write_cache(output: &str) {
    if let Ok(path) = cache_path() {
        let json = serde_json::json!({
            "cached_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
            "output": output
        });
        let _ = std::fs::write(&path, json.to_string());
    }
}

#[tokio::main]
async fn main() {
    // statusline 模式：有 stdin 管道数据，先丢弃
    if has_stdin_data() {
        drain_stdin();
        // statusline 调用频繁，优先使用缓存
        if let Some(cached) = read_cache() {
            println!("{}", cached);
            return;
        }
    }

    let result = fetch_and_format().await;
    match result {
        Ok(output) => {
            write_cache(&output);
            println!("{}", output);
        }
        Err(e) => {
            // 出错时也尝试用过期缓存
            if let Some(cached) = read_cache() {
                println!("{}", cached);
            } else {
                eprintln!("[ZhipuKit] {}", e);
            }
        }
    }
}

struct QuotaData {
    balance: Option<f64>,
    level: Option<String>,
    hour5_pct: Option<i64>,
    hour5_next_reset: Option<i64>,
    weekly_pct: Option<i64>,
    weekly_next_reset: Option<i64>,
    mcp_used: Option<i64>,
    mcp_total: Option<i64>,
    mcp_next_reset: Option<i64>,
}

async fn fetch_and_format() -> Result<String, String> {
    let home = get_home_dir()?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Err("未找到 Claude Code 配置文件".to_string());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;

    let api_key = config
        .get("env")
        .and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if api_key.is_empty() {
        return Err("ANTHROPIC_AUTH_TOKEN 未配置".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut data = QuotaData {
        balance: None,
        level: None,
        hour5_pct: None,
        hour5_next_reset: None,
        weekly_pct: None,
        weekly_next_reset: None,
        mcp_used: None,
        mcp_total: None,
        mcp_next_reset: None,
    };

    // 查询余额
    if let Ok(resp) = client
        .get("https://www.bigmodel.cn/api/biz/account/query-customer-account-report")
        .header("Authorization", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let d = json.get("data").cloned().unwrap_or(json);
                data.balance = Some(d.get("availableBalance").and_then(|v| v.as_f64()).unwrap_or(0.0));
            }
        }
    }

    // 查询 Coding Plan
    if let Ok(resp) = client
        .get("https://open.bigmodel.cn/api/monitor/usage/quota/limit")
        .header("Authorization", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if json.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                    if let Some(plan) = json.get("data") {
                        data.level = plan.get("level").and_then(|v| v.as_str()).map(String::from);
                        if let Some(limits) = plan.get("limits").and_then(|v| v.as_array()) {
                            let mut tokens_count = 0;
                            for limit in limits {
                                match limit.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                                    "TIME_LIMIT" => {
                                        data.mcp_total = limit.get("usage").and_then(|v| v.as_i64());
                                        data.mcp_used = limit.get("currentValue").and_then(|v| v.as_i64());
                                        data.mcp_next_reset = limit.get("nextResetTime").and_then(|v| v.as_i64());
                                    }
                                    "TOKENS_LIMIT" => {
                                        let pct = limit.get("percentage").and_then(|v| v.as_i64()).unwrap_or(0);
                                        let next_reset = limit.get("nextResetTime").and_then(|v| v.as_i64());
                                        if tokens_count == 0 {
                                            data.hour5_pct = Some(pct);
                                            data.hour5_next_reset = next_reset;
                                        } else {
                                            data.weekly_pct = Some(pct);
                                            data.weekly_next_reset = next_reset;
                                        }
                                        tokens_count += 1;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 格式化输出
    let level_str = data.level.as_deref().unwrap_or("unknown");
    let mut line1 = format!("ZhipuKit {}", level_str.to_uppercase());
    if let Some(balance) = data.balance {
        line1.push_str(&format!(" | ¥{}", format_amount(balance)));
    }
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

    if let Some(pct) = data.hour5_pct {
        let mut s = format!("5h {}", progress_bar_pct(pct, 8));
        if let Some(reset) = data.hour5_next_reset {
            let remaining_ms = (reset - now).max(0);
            let elapsed_ms = (5 * 3600 * 1000 - remaining_ms).max(0);
            s.push_str(&format!(" ({}/5h)", format_remaining(elapsed_ms)));
        }
        quota_parts.push(s);
    }

    if let (Some(used), Some(total)) = (data.mcp_used, data.mcp_total) {
        if total > 0 {
            let pct = (used * 100 / total).min(100);
            let mut s = format!("MCP {}", progress_bar(pct, 8));
            let mut time_info = format!("{}/{}", used, total);
            if let Some(reset) = data.mcp_next_reset {
                let remaining_ms = (reset - now).max(0);
                let elapsed_ms = (30 * 24 * 3600 * 1000 - remaining_ms).max(0);
                let d = elapsed_ms / (24 * 3600 * 1000);
                let h = (elapsed_ms % (24 * 3600 * 1000)) / (3600 * 1000);
                time_info.push_str(&format!(" | {}d {}h/30d", d, h));
            }
            s.push_str(&format!(" ({})", time_info));
            quota_parts.push(s);
        }
    }

    if quota_parts.is_empty() {
        Ok(line1)
    } else {
        Ok(format!("{}\n{}", line1, quota_parts.join(" | ")))
    }
}
