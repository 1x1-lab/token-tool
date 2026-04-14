// zhipukit-claude-code-plugin: Claude Code statusline 工具
// 读取 ~/.claude/settings.json 中的 ANTHROPIC_AUTH_TOKEN 和 zhipuEndpoint，查询智谱 API，输出套餐信息到 stdout
// 支持 statusline 模式（缓存时间可配置，默认 5 分钟）和独立测试模式

use app_lib::utils::{
    ansi_color, balance_base_url, build_url, format_amount, format_remaining, format_status_bar,
    strip_ansi, API_PATH_BALANCE, API_PATH_CODING_PLAN,
};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

/// 默认 endpoint（国内版）
const DEFAULT_ENDPOINT: &str = "https://open.bigmodel.cn";

/// 固定进度条宽度
const BAR_WIDTH: usize = 10;

/// 带 ANSI 颜色进度条 + 百分比
fn progress_bar_pct(percentage: f64, bar_colors: &BarColors) -> String {
    let bar = format_status_bar(percentage, BAR_WIDTH, Some((bar_colors.normal, bar_colors.warning, bar_colors.danger)));
    format!("{} {}%", bar, percentage)
}

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

/// Autocompact 缓冲比例（经验值，匹配 Claude Code /context 输出）
const AUTOCOMPACT_BUFFER_PERCENT: f64 = 0.165;
/// 缓冲缩放下限：≤5% 使用率时无缓冲
const BUFFER_SCALE_LOW: f64 = 0.05;
/// 缓冲缩放上限：≥50% 使用率时满缓冲
const BUFFER_SCALE_HIGH: f64 = 0.50;

/// 从 Claude Code stdin JSON 解析的上下文窗口信息
struct ClaudeContext {
    used_percentage: Option<i64>,
    context_window_size: i64,
    /// 当前上下文实际占用（input + cache_creation + cache_read）
    current_tokens: i64,
    /// input_tokens 分项
    input_tokens: i64,
    /// cache_creation + cache_read 合计
    cache_tokens: i64,
}

impl ClaudeContext {
    /// 原始百分比：优先用原生值，否则手动计算
    #[allow(dead_code)]
    fn raw_percent(&self) -> i64 {
        if let Some(pct) = self.used_percentage {
            return pct.clamp(0, 100);
        }
        if self.context_window_size <= 0 {
            return 0;
        }
        ((self.current_tokens as f64 / self.context_window_size as f64) * 100.0).round() as i64
    }

    /// 带缓冲的百分比：优先用原生值，否则手动计算 + autocompact 缓冲
    fn buffered_percent(&self) -> i64 {
        if let Some(pct) = self.used_percentage {
            return pct.clamp(0, 100);
        }
        if self.context_window_size <= 0 {
            return 0;
        }
        let raw_ratio = self.current_tokens as f64 / self.context_window_size as f64;
        // 缓冲缩放：低使用率无缓冲，高使用率满缓冲
        let scale = ((raw_ratio - BUFFER_SCALE_LOW) / (BUFFER_SCALE_HIGH - BUFFER_SCALE_LOW))
            .clamp(0.0, 1.0);
        let buffer = self.context_window_size as f64 * AUTOCOMPACT_BUFFER_PERCENT * scale;
        (((self.current_tokens as f64 + buffer) / self.context_window_size as f64 * 100.0)
            .round() as i64)
            .clamp(0, 100)
    }
}

/// stdin 解析结果：上下文窗口信息 + 当前工作目录 + 当前模型
struct StdinData {
    context: ClaudeContext,
    cwd: Option<String>,
    model: Option<String>,
}

/// 解析 Claude Code 通过 stdin 传入的 JSON，提取上下文窗口信息和项目路径
/// 返回 None 如果没有 stdin 数据或解析失败（向后兼容）
fn parse_stdin_data() -> Option<StdinData> {
    if io::stdin().is_terminal() {
        return None;
    }
    let mut buf = String::new();
    if io::stdin().lock().read_to_string(&mut buf).is_err() || buf.trim().is_empty() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(&buf).ok()?;
    let cw = json.get("context_window")?;

    // 从 current_usage 获取实际上下文占用
    let (input_tokens, cache_tokens, current_tokens) = cw
        .get("current_usage")
        .map(|u| {
            let input = u.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let cache_create = u
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let cache_read = u
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let cache = cache_create + cache_read;
            (input, cache, input + cache)
        })
        .unwrap_or((0, 0, 0));

    let context = ClaudeContext {
        used_percentage: cw.get("used_percentage").and_then(|v| v.as_i64()),
        context_window_size: cw
            .get("context_window_size")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        current_tokens,
        input_tokens,
        cache_tokens,
    };

    let cwd = json
        .get("cwd")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let model = json
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| strip_ansi(s));

    Some(StdinData { context, cwd, model })
}

/// 获取指定目录的 git 分支名
fn get_git_branch(dir: &str) -> Option<String> {
    std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(dir)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let branch = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if branch.is_empty() { None } else { Some(branch) }
            } else {
                None
            }
        })
}

/// 对 api_key + endpoint 计算 hash 摘要（前 16 位 hex）
fn config_hash(api_key: &str, endpoint: &str) -> String {
    let mut hasher = DefaultHasher::new();
    format!("{}\n{}", api_key, endpoint).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// 解析模型映射：opus → ANTHROPIC_DEFAULT_OPUS_MODEL 等
/// "opus[1m]" + 映射 "glm-5.1" → "glm-5.1[1m]"
fn resolve_model(model: &str, config: &serde_json::Value) -> String {
    // 分离 base name 和 context 后缀 (如 "[1m]")
    let (base, suffix) = if let Some(idx) = model.find('[') {
        (&model[..idx], &model[idx..])
    } else {
        (model, "")
    };

    let env = config.get("env");
    let mapped = match base.to_lowercase().as_str() {
        "opus" => env.and_then(|e| e.get("ANTHROPIC_DEFAULT_OPUS_MODEL")).and_then(|v| v.as_str()),
        "sonnet" => env.and_then(|e| e.get("ANTHROPIC_DEFAULT_SONNET_MODEL")).and_then(|v| v.as_str()),
        "haiku" => env.and_then(|e| e.get("ANTHROPIC_DEFAULT_HAIKU_MODEL")).and_then(|v| v.as_str()),
        _ => None,
    };

    match mapped {
        Some(m) if !m.is_empty() => format!("{}{}", m, suffix),
        _ => model.to_string(),
    }
}

/// 提前读取 settings.json 中的 api_key、endpoint 和 model（用于缓存校验和模型展示）
fn read_config_keys() -> (Option<String>, Option<String>, Option<String>) {
    let home = match get_home_dir() {
        Ok(h) => h,
        Err(_) => return (None, None, None),
    };
    let config_path = home.join(".claude").join("settings.json");
    if !config_path.exists() {
        return (None, None, None);
    }
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return (None, None, None),
    };
    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return (None, None, None),
    };

    let api_key = config
        .get("env")
        .and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let endpoint = Some(
        config
            .get("zhipuEndpoint")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_ENDPOINT)
            .to_string(),
    );

    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|m| resolve_model(m, &config));

    (api_key, endpoint, model)
}

/// 缓存文件路径
fn cache_path() -> Result<PathBuf, String> {
    Ok(get_home_dir()?.join(".claude").join("zhipukit-cache.json"))
}

/// 从 settings.json 读取缓存有效期（秒），默认 300 秒（5 分钟）
fn read_cache_duration() -> i64 {
    let home = match get_home_dir() {
        Ok(h) => h,
        Err(_) => return 300,
    };
    let config_path = home.join(".claude").join("settings.json");
    if !config_path.exists() {
        return 300;
    }
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return 300,
    };
    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return 300,
    };
    config
        .get("zhipuCacheDuration")
        .and_then(|v| v.as_i64())
        .filter(|&d| d > 0)
        .unwrap_or(300)
}

/// 读取缓存中的 cached_at 时间戳（毫秒）
fn read_cached_at() -> Option<i64> {
    let path = cache_path().ok()?;
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("cached_at").and_then(|v| v.as_i64())
}

/// 尝试读取缓存（有效期内且 key_hash 匹配），返回结构化配额数据
fn read_cache(api_key: &str, endpoint: &str) -> Option<QuotaData> {
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

    // 缓存有效期（从 settings.json 读取，默认 5 分钟）
    let cache_duration_ms = read_cache_duration() * 1000;
    if now - cached_at > cache_duration_ms {
        return None;
    }

    // 校验 key_hash，确保缓存来源与当前配置一致
    let expected_hash = config_hash(api_key, endpoint);
    let cached_hash = json.get("key_hash").and_then(|v| v.as_str()).unwrap_or("");
    if cached_hash != expected_hash {
        return None;
    }

    // 从 JSON 反序列化为 QuotaData（serde 忽略 cached_at/key_hash 等多余字段）
    serde_json::from_value::<QuotaData>(json).ok()
}

/// 读取过期缓存（fetch 失败时的降级）
fn read_cache_expired(api_key: &str, endpoint: &str) -> Option<QuotaData> {
    let path = cache_path().ok()?;
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let expected_hash = config_hash(api_key, endpoint);
    let cached_hash = json.get("key_hash").and_then(|v| v.as_str()).unwrap_or("");
    if cached_hash != expected_hash {
        return None;
    }
    serde_json::from_value::<QuotaData>(json).ok()
}

/// 写入缓存（结构化 QuotaData）
fn write_cache(data: &QuotaData, api_key: &str, endpoint: &str) {
    if let Ok(path) = cache_path() {
        let mut cache_json = serde_json::to_value(data).unwrap_or(serde_json::json!({}));
        if let Some(obj) = cache_json.as_object_mut() {
            obj.insert(
                "cached_at".to_string(),
                serde_json::Value::Number(
                    (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64)
                        .into(),
                ),
            );
            obj.insert(
                "key_hash".to_string(),
                serde_json::Value::String(config_hash(api_key, endpoint)),
            );
        }
        let _ = std::fs::write(&path, cache_json.to_string());
    }
}


#[tokio::main]
async fn main() {
    // ── Phase 1: 数据收集 ──
    let (api_key, endpoint, settings_model) = read_config_keys();
    let stdin_data = parse_stdin_data();
    let seg_cfg = read_segment_config();
    let bar_colors = read_bar_colors();
    let cwd = stdin_data.as_ref().and_then(|d| d.cwd.clone());
    let stdin_model = stdin_data.as_ref().and_then(|d| d.model.clone());
    // 优先使用 stdin 传入的运行时模型名（实际使用的模型），回退到 settings.json 配置
    let effective_model = stdin_model.as_deref().or(settings_model.as_deref());
    let git_branch = cwd.as_deref().and_then(|dir| get_git_branch(dir));

    // ── Phase 2: 获取配额数据 ──
    let is_statusline = !io::stdin().is_terminal();
    let quota: QuotaData = if is_statusline {
        // statusline 模式：优先使用缓存
        if let (Some(ref ak), Some(ref ep)) = (&api_key, &endpoint) {
            if let Some(cached) = read_cache(ak, ep) {
                cached
            } else if let Ok(data) = fetch_quota_data().await {
                write_cache(&data, ak, ep);
                data
            } else if let Some(cached) = read_cache_expired(ak, ep) {
                cached
            } else {
                QuotaData::default()
            }
        } else {
            QuotaData::default()
        }
    } else {
        // 测试模式：总是请求 API
        match fetch_quota_data().await {
            Ok(data) => {
                if let (Some(ref ak), Some(ref ep)) = (&api_key, &endpoint) {
                    write_cache(&data, ak, ep);
                }
                data
            }
            Err(e) => {
                // fetch 失败：尝试过期缓存 → 仅显示上下文 → 报错
                if let (Some(ref ak), Some(ref ep)) = (&api_key, &endpoint) {
                    if let Some(cached) = read_cache_expired(ak, ep) {
                        let ctx = stdin_data.as_ref().map(|d| &d.context);
                        let segments = build_segments(
                            &cached, ctx, effective_model, git_branch.as_deref(),
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as i64,
                            &seg_cfg, &bar_colors,
                        );
                        let output = render_segments(&segments);
                        if !output.is_empty() {
                            println!("{}", output);
                        }
                        return;
                    }
                }
                if let Some(ref data) = stdin_data {
                    let mut parts: Vec<String> = Vec::new();
                    if let Some(ref m) = effective_model {
                        parts.push(format_model(m));
                    }
                    parts.push(format_context_usage(&data.context, &bar_colors));
                    println!("{}", parts.join(" "));
                } else if let Some(ref m) = effective_model {
                    eprintln!("[{}] {}", m, e);
                } else {
                    eprintln!("[ZhipuKit] {}", e);
                }
                return;
            }
        }
    };

    // ── Phase 3: 渲染 ──
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let ctx = stdin_data.as_ref().map(|d| &d.context);
    let segments = build_segments(&quota, ctx, effective_model, git_branch.as_deref(), now_ms, &seg_cfg, &bar_colors);

    let output = render_segments(&segments);
    if !output.is_empty() {
        println!("{}", output);
    }
}

// ── 段格式化函数（每个函数只格式化一个段） ──

fn format_tier(level: &str) -> String {
    format!("ZhipuKit {}", level.to_uppercase())
}

fn format_balance(balance: f64) -> String {
    format!("¥{}", format_amount(balance))
}

fn format_git(branch: &str) -> String {
    format!("Git ({})", branch)
}

fn format_model(model: &str) -> String {
    format!("Model ({})", model)
}

fn format_hour5(pct: i64, next_reset: Option<i64>, now_ms: i64, bar_colors: &BarColors) -> String {
    let mut s = format!("5h {}", progress_bar_pct(pct as f64, bar_colors));
    if let Some(reset) = next_reset {
        let remaining_ms = (reset - now_ms).max(0);
        let elapsed_ms = (5 * 3600 * 1000 - remaining_ms).max(0);
        s.push_str(&format!(" ({}/5h)", format_remaining(elapsed_ms)));
    }
    s
}

fn format_mcp(used: i64, total: i64, next_reset: Option<i64>, now_ms: i64, bar_colors: &BarColors) -> String {
    let pct = (used as f64 * 100.0 / total as f64).min(100.0);
    let mut s = format!("MCP {}", format_status_bar(pct, BAR_WIDTH, Some((bar_colors.normal, bar_colors.warning, bar_colors.danger))));
    let mut time_info = format!("{}/{}", used, total);
    if let Some(reset) = next_reset {
        let remaining_ms = (reset - now_ms).max(0);
        let elapsed_ms = (30 * 24 * 3600 * 1000 - remaining_ms).max(0);
        let d = elapsed_ms / (24 * 3600 * 1000);
        let h = (elapsed_ms % (24 * 3600 * 1000)) / (3600 * 1000);
        time_info.push_str(&format!(", {}d {}h/30d", d, h));
    }
    s.push_str(&format!(" ({})", time_info));
    s
}

fn format_cache_time() -> Option<String> {
    let cached_at = read_cached_at()?;
    // 使用系统 date 命令获取本地时区偏移，或直接从 TZ 环境变量解析
    let offset_secs = tz_offset_secs();
    let local_secs = (cached_at / 1000) + offset_secs;
    let h = ((local_secs / 3600) % 24).rem_euclid(24) as u32;
    let m = ((local_secs % 3600) / 60).rem_euclid(60) as u32;
    let s = (local_secs % 60).rem_euclid(60) as u32;
    Some(format!("CacheTime {:02}:{:02}:{:02}", h, m, s))
}

/// 从 TZ 环境变量获取时区偏移秒数，默认 UTC+8
fn tz_offset_secs() -> i64 {
    // TZ 格式如 "Asia/Shanghai" 或 "UTC+8" 或 "CST-8"
    // 最可靠：读取 /etc/localtime 链接或 date 命令
    // 简单方案：用 date 获取 %z
    if let Ok(output) = std::process::Command::new("date")
        .arg("+%z")
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // 格式: +0800 或 -0500
            if s.len() >= 5 {
                let sign: i64 = if s.starts_with('-') { -1 } else { 1 };
                let hours: i64 = s[1..3].parse().unwrap_or(0);
                let mins: i64 = s[3..5].parse().unwrap_or(0);
                return sign * (hours * 3600 + mins * 60);
            }
        }
    }
    // 默认 UTC+8
    8 * 3600
}

fn format_context_usage(ctx: &ClaudeContext, bar_colors: &BarColors) -> String {
    let pct = ctx.buffered_percent();
    let bar = format_status_bar(pct as f64, BAR_WIDTH, Some((bar_colors.normal, bar_colors.warning, bar_colors.danger)));
    let mut result = if ctx.current_tokens > 0 && ctx.context_window_size > 0 {
        let size_k = ctx.context_window_size / 1000;
        format!(
            "Context {} {}% ({:.1}k/{}k)",
            bar,
            pct,
            ctx.current_tokens as f64 / 1000.0,
            size_k
        )
    } else {
        format!("Context {} {}%", bar, pct)
    };
    if pct >= 85 && (ctx.input_tokens > 0 || ctx.cache_tokens > 0) {
        let in_k = ctx.input_tokens as f64 / 1000.0;
        let cache_k = ctx.cache_tokens as f64 / 1000.0;
        result.push_str(&format!(" (in: {:.1}k, cache: {:.1}k)", in_k, cache_k));
    }
    result
}

// ── 结构化段构建 ──

fn build_segments(
    quota: &QuotaData,
    ctx: Option<&ClaudeContext>,
    model: Option<&str>,
    git_branch: Option<&str>,
    now_ms: i64,
    seg_cfg: &SegmentConfig,
    bar_colors: &BarColors,
) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();

    // Row 0: 状态行 (tier, balance, git)
    let mut row0: Vec<String> = Vec::new();
    if seg_cfg.tier {
        if let Some(level) = &quota.level {
            row0.push(format_tier(level));
        }
    }
    if seg_cfg.balance {
        if let Some(balance) = quota.balance {
            row0.push(format_balance(balance));
        }
    }
    if seg_cfg.git {
        if let Some(branch) = git_branch {
            row0.push(format_git(branch));
        }
    }
    if !row0.is_empty() {
        rows.push(row0);
    }

    // Row 1: 模型 + 上下文（单独一行）
    let mut row1: Vec<String> = Vec::new();
    if seg_cfg.model {
        if let Some(m) = model {
            row1.push(format_model(m));
        }
    }
    if seg_cfg.context {
        if let Some(context) = ctx {
            row1.push(format_context_usage(context, bar_colors));
        }
    }
    if !row1.is_empty() {
        rows.push(row1);
    }

    // Row 2: 配额行 (hour5, mcp, refresh)
    let mut row2: Vec<String> = Vec::new();
    if seg_cfg.hour5 {
        if let Some(pct) = quota.hour5_pct {
            row2.push(format_hour5(pct, quota.hour5_next_reset, now_ms, bar_colors));
        }
    }
    if seg_cfg.mcp {
        if let (Some(used), Some(total)) = (quota.mcp_used, quota.mcp_total) {
            if total > 0 {
                row2.push(format_mcp(used, total, quota.mcp_next_reset, now_ms, bar_colors));
            }
        }
    }
    if seg_cfg.cache_time {
        if let Some(cache_str) = format_cache_time() {
            row2.push(cache_str);
        }
    }
    if !row2.is_empty() {
        rows.push(row2);
    }

    rows
}

// ── 渲染 ──

/// 渲染二维段到输出字符串
fn render_segments(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|segments| segments.join(" | "))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 段展示配置：控制状态栏中各信息段的显隐
#[derive(Debug)]
struct SegmentConfig {
    tier: bool,
    balance: bool,
    git: bool,
    model: bool,
    context: bool,
    hour5: bool,
    mcp: bool,
    cache_time: bool,
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

/// 从 settings.json 读取 zhipuSegments 配置
fn read_segment_config() -> SegmentConfig {
    let home = match get_home_dir() {
        Ok(h) => h,
        Err(_) => return SegmentConfig::default(),
    };
    let config_path = home.join(".claude").join("settings.json");
    if !config_path.exists() {
        return SegmentConfig::default();
    }
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return SegmentConfig::default(),
    };
    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return SegmentConfig::default(),
    };
    let seg = match config.get("zhipuSegments").and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return SegmentConfig::default(),
    };
    let def = SegmentConfig::default();
    SegmentConfig {
        tier: seg.get("tier").and_then(|v| v.as_bool()).unwrap_or(def.tier),
        balance: seg.get("balance").and_then(|v| v.as_bool()).unwrap_or(def.balance),
        git: seg.get("git").and_then(|v| v.as_bool()).unwrap_or(def.git),
        model: seg.get("model").and_then(|v| v.as_bool()).unwrap_or(def.model),
        context: seg.get("context").and_then(|v| v.as_bool()).unwrap_or(def.context),
        hour5: seg.get("hour5").and_then(|v| v.as_bool()).unwrap_or(def.hour5),
        mcp: seg.get("mcp").and_then(|v| v.as_bool()).unwrap_or(def.mcp),
        cache_time: seg.get("cacheTime").and_then(|v| v.as_bool()).unwrap_or(def.cache_time),
    }
}

/// 进度条颜色配置
struct BarColors {
    normal: &'static str,
    warning: &'static str,
    danger: &'static str,
}

impl Default for BarColors {
    fn default() -> Self {
        Self {
            normal: "\x1b[32m",
            warning: "\x1b[33m",
            danger: "\x1b[31m",
        }
    }
}

/// 从 settings.json 读取 zhipuBarColors 配置
fn read_bar_colors() -> BarColors {
    let home = match get_home_dir() {
        Ok(h) => h,
        Err(_) => return BarColors::default(),
    };
    let config_path = home.join(".claude").join("settings.json");
    if !config_path.exists() {
        return BarColors::default();
    }
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return BarColors::default(),
    };
    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return BarColors::default(),
    };
    let colors = match config.get("zhipuBarColors").and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return BarColors::default(),
    };
    let def = BarColors::default();
    BarColors {
        normal: colors
            .get("normal")
            .and_then(|v| v.as_str())
            .map(ansi_color)
            .unwrap_or(def.normal),
        warning: colors
            .get("warning")
            .and_then(|v| v.as_str())
            .map(ansi_color)
            .unwrap_or(def.warning),
        danger: colors
            .get("danger")
            .and_then(|v| v.as_str())
            .map(ansi_color)
            .unwrap_or(def.danger),
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
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

async fn fetch_quota_data() -> Result<QuotaData, String> {
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

    // 从 settings.json 读取 zhipuEndpoint，默认国内版
    let endpoint = config
        .get("zhipuEndpoint")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_ENDPOINT)
        .to_string();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut data = QuotaData::default();

    let balance_url = build_url(&balance_base_url(&endpoint), API_PATH_BALANCE);
    let plan_url = build_url(&endpoint, API_PATH_CODING_PLAN);

    // 查询余额
    if let Ok(resp) = client
        .get(&balance_url)
        .header("Authorization", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let d = json.get("data").cloned().unwrap_or(json);
                data.balance =
                    Some(d.get("availableBalance").and_then(|v| v.as_f64()).unwrap_or(0.0));
            }
        }
    }

    // 查询 Coding Plan
    if let Ok(resp) = client
        .get(&plan_url)
        .header("Authorization", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if json
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    if let Some(plan) = json.get("data") {
                        data.level = plan
                            .get("level")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        if let Some(limits) = plan.get("limits").and_then(|v| v.as_array()) {
                            let mut tokens_count = 0;
                            for limit in limits {
                                match limit
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                {
                                    "TIME_LIMIT" => {
                                        data.mcp_total =
                                            limit.get("usage").and_then(|v| v.as_i64());
                                        data.mcp_used =
                                            limit.get("currentValue").and_then(|v| v.as_i64());
                                        data.mcp_next_reset =
                                            limit.get("nextResetTime").and_then(|v| v.as_i64());
                                    }
                                    "TOKENS_LIMIT" => {
                                        let pct = limit
                                            .get("percentage")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0);
                                        let next_reset =
                                            limit.get("nextResetTime").and_then(|v| v.as_i64());
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

    Ok(data)
}
