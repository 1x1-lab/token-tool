#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

// API 路径常量
pub const API_PATH_BALANCE: &str = "/api/biz/account/query-customer-account-report";
pub const API_PATH_CODING_PLAN: &str = "/api/monitor/usage/quota/limit";
pub const API_PATH_CHAT_COMPLETIONS: &str = "/api/paas/v4/chat/completions";

/// 根据主 endpoint (如 https://open.bigmodel.cn) 推导余额查询的 base URL
/// 国内版余额 API 在 www.bigmodel.cn，国际版直接使用 endpoint
pub fn balance_base_url(endpoint: &str) -> String {
    if endpoint.contains("bigmodel.cn") {
        endpoint.replace("open.bigmodel.cn", "www.bigmodel.cn")
    } else {
        endpoint.to_string()
    }
}

/// 拼接完整 URL
pub fn build_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

pub fn get_home_dir() -> Result<std::path::PathBuf, String> {
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

pub fn format_amount(v: f64) -> String {
    if v == v.floor() {
        format!("{}", v as i64)
    } else {
        format!("{:.4}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// ANSI 彩色进度条（与 zhipukit-claude-code-plugin.exe 保持一致）
pub fn format_status_bar(percentage: i64, length: usize) -> String {
    let pct = percentage.clamp(0, 100);
    // 四分之一圆字符: 空、¼、½、¾、满
    let dots = ['○', '◔', '◑', '◕', '●'];
    let steps = length * 4; // 总粒度 = length × 4
    let filled_steps = ((pct as f64 / 100.0) * steps as f64).round() as usize;
    let color = if percentage >= 85 {
        "\x1b[31m"
    } else if percentage >= 70 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    };
    let reset = "\x1b[0m";
    let mut bar = String::new();
    let mut remaining = filled_steps;
    for i in 0..length {
        let step = remaining.min(4);
        // 已满的用彩色，未满的用默认色
        if i < filled_steps / 4 {
            bar.push_str(&format!("{}{}", color, dots[4]));
        } else if step > 0 {
            bar.push_str(&format!("{}{}", color, dots[step]));
        } else {
            bar.push(dots[0]);
        }
        remaining = remaining.saturating_sub(4);
    }
    format!("{}{}", bar, reset)
}

/// 格式化剩余时间
pub fn format_remaining(ms: i64) -> String {
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
pub fn build_shell_command(program: &str, args: &[&str]) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}
