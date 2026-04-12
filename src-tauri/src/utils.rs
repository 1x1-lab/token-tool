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
/// 每个圆代表 5%，0-50% 映射到整个条（50%+ 全满，颜色仍按 70%/85% 变化）
/// 填充级别：○ → ◔ → ◑ → ◕ → ●（空 / ¼ / ½ / ¾ / 满）
pub fn format_status_bar(percentage: i64, length: usize) -> String {
    let pct = percentage.clamp(0, 100);
    // 每个圆代表 5%，计算当前落在第几个圆（active 位置）
    let active_index = if pct >= 50 {
        length // 50%+ 全满
    } else {
        (pct as usize) / 5
    };
    // 当前 5% 段内的余量 (0-4)
    let sub_pct = pct % 5;
    let color = if percentage >= 85 {
        "\x1b[31m"
    } else if percentage >= 70 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    };
    let reset = "\x1b[0m";
    let mut bar = String::new();
    for i in 0..length {
        if i < active_index {
            // 已完成的圆：彩色实心
            bar.push_str(&format!("{}●", color));
        } else if i == active_index && sub_pct > 0 {
            // 当前段部分填充：根据 sub_pct 选择 ¼ / ½ / ¾
            let ch = match sub_pct {
                1 => "◔",  // 1/5 ≈ 25%
                2 => "◑",  // 2/5 = 50%
                _ => "◕",  // 3-4/5 = 75%+
            };
            bar.push_str(&format!("{}{}", color, ch));
        } else if i == active_index {
            // 当前段无填充：彩色空心
            bar.push_str(&format!("{}○", color));
        } else {
            // 未到达：默认色空心
            bar.push_str(&format!("{}○", reset));
        }
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
