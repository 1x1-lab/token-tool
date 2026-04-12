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
/// 每个圆代表 10%，0-100% 映射到整个条，颜色按 70%/85% 变化
/// 填充级别：○ → ◔ → ◑ → ◕ → ●（空 / ¼ / ½ / ¾ / 满）
/// 阈值：<2.5%→○，≥2.5%→◔，≥5%→◑，≥7.5%→◕，10%（段满）→●
pub fn format_status_bar(percentage: f64, length: usize) -> String {
    let pct = percentage.clamp(0.0, 100.0);
    // 每个圆代表 10%，计算当前落在第几个圆（active 位置）
    let active_index = if pct as usize >= length * 10 {
        length // 100% 全满
    } else {
        (pct / 10.0).floor() as usize
    };
    // 当前 10% 段内的余量 (0.0 - 9.99...)
    let sub_pct = pct % 10.0;
    let color = if percentage >= 85.0 {
        "\x1b[31m"
    } else if percentage >= 70.0 {
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
        } else if i == active_index && sub_pct >= 2.5 {
            // 当前段部分填充：按 sub_pct 选择 ¼ / ½ / ¾
            let ch = if sub_pct < 5.0 {
                "◔"
            } else if sub_pct < 7.5 {
                "◑"
            } else {
                "◕"
            };
            bar.push_str(&format!("{}{}", color, ch));
        } else if i == active_index {
            // 当前段 < 2.5%：彩色空心
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

#[cfg(test)]
mod tests {
    use super::*;

    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const RED: &str = "\x1b[31m";

    /// 辅助：从带 ANSI 的进度条中提取纯字符（去掉颜色码）
    fn strip_ansi_bar(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            } else {
                result.push(c);
            }
        }
        result
    }

    // ── 段填充级别测试 ──

    #[test]
    fn test_zero_percent() {
        let bar = strip_ansi_bar(&format_status_bar(0.0, 10));
        assert_eq!(bar, "○○○○○○○○○○");
    }

    #[test]
    fn test_one_percent_empty() {
        // 1% < 2.5% → ○
        let bar = strip_ansi_bar(&format_status_bar(1.0, 10));
        assert_eq!(bar, "○○○○○○○○○○");
    }

    #[test]
    fn test_2_4_percent_empty() {
        // 2.4% < 2.5% → ○
        let bar = strip_ansi_bar(&format_status_bar(2.4, 10));
        assert_eq!(bar, "○○○○○○○○○○");
    }

    #[test]
    fn test_2_5_percent_quarter() {
        // 2.5% = 2.5 阈值 → ◔
        let bar = strip_ansi_bar(&format_status_bar(2.5, 10));
        assert_eq!(bar, "◔○○○○○○○○○");
    }

    #[test]
    fn test_4_9_percent_quarter() {
        // 4.9% < 5.0 → ◔
        let bar = strip_ansi_bar(&format_status_bar(4.9, 10));
        assert_eq!(bar, "◔○○○○○○○○○");
    }

    #[test]
    fn test_five_percent_half() {
        // 5% → sub_pct=5.0, ≥5.0 <7.5 → ◑
        let bar = strip_ansi_bar(&format_status_bar(5.0, 10));
        assert_eq!(bar, "◑○○○○○○○○○");
    }

    #[test]
    fn test_seven_percent_half() {
        // 7% → sub_pct=7.0, ≥5.0 <7.5 → ◑
        let bar = strip_ansi_bar(&format_status_bar(7.0, 10));
        assert_eq!(bar, "◑○○○○○○○○○");
    }

    #[test]
    fn test_7_4_percent_half() {
        // 7.4% < 7.5 → ◑
        let bar = strip_ansi_bar(&format_status_bar(7.4, 10));
        assert_eq!(bar, "◑○○○○○○○○○");
    }

    #[test]
    fn test_7_5_percent_three_quarter() {
        // 7.5% = 7.5 阈值 → ◕
        let bar = strip_ansi_bar(&format_status_bar(7.5, 10));
        assert_eq!(bar, "◕○○○○○○○○○");
    }

    #[test]
    fn test_nine_percent_three_quarter() {
        // 9% → ◕
        let bar = strip_ansi_bar(&format_status_bar(9.0, 10));
        assert_eq!(bar, "◕○○○○○○○○○");
    }

    #[test]
    fn test_ten_percent_full() {
        // 10% → active_index=1, 位置 0 满 → ●
        let bar = strip_ansi_bar(&format_status_bar(10.0, 10));
        assert_eq!(bar, "●○○○○○○○○○");
    }

    // ── 多段填充测试 ──

    #[test]
    fn test_twenty_percent() {
        let bar = strip_ansi_bar(&format_status_bar(20.0, 10));
        assert_eq!(bar, "●●○○○○○○○○");
    }

    #[test]
    fn test_twenty_three_percent() {
        // 23% → active_index=2, sub_pct=3.0 → ◔
        let bar = strip_ansi_bar(&format_status_bar(23.0, 10));
        assert_eq!(bar, "●●◔○○○○○○○");
    }

    #[test]
    fn test_fifty_percent() {
        let bar = strip_ansi_bar(&format_status_bar(50.0, 10));
        assert_eq!(bar, "●●●●●○○○○○");
    }

    #[test]
    fn test_seventy_percent() {
        let bar = strip_ansi_bar(&format_status_bar(70.0, 10));
        assert_eq!(bar, "●●●●●●●○○○");
    }

    #[test]
    fn test_hundred_percent() {
        let bar = strip_ansi_bar(&format_status_bar(100.0, 10));
        assert_eq!(bar, "●●●●●●●●●●");
    }

    // ── MCP 场景：152/4000 = 3.8% ──

    #[test]
    fn test_mcp_152_of_4000() {
        let pct = 152.0 * 100.0 / 4000.0; // = 3.8
        let bar = strip_ansi_bar(&format_status_bar(pct, 10));
        assert_eq!(bar, "◔○○○○○○○○○");
    }

    // ── MCP 场景：100/4000 = 2.5% ──

    #[test]
    fn test_mcp_100_of_4000() {
        let pct = 100.0 * 100.0 / 4000.0; // = 2.5
        let bar = strip_ansi_bar(&format_status_bar(pct, 10));
        assert_eq!(bar, "◔○○○○○○○○○");
    }

    // ── MCP 场景：99/4000 = 2.475% ──

    #[test]
    fn test_mcp_99_of_4000() {
        let pct = 99.0 * 100.0 / 4000.0; // = 2.475
        let bar = strip_ansi_bar(&format_status_bar(pct, 10));
        assert_eq!(bar, "○○○○○○○○○○");
    }

    // ── 颜色测试 ──

    #[test]
    fn test_color_green_below_70() {
        let bar = format_status_bar(10.0, 10);
        assert!(bar.contains(GREEN), "should be green: {}", bar);
        assert!(!bar.contains(YELLOW));
        assert!(!bar.contains(RED));
    }

    #[test]
    fn test_color_yellow_70_to_84() {
        let bar = format_status_bar(75.0, 10);
        assert!(bar.contains(YELLOW), "should be yellow: {}", bar);
        assert!(!bar.contains(RED));
    }

    #[test]
    fn test_color_red_85_plus() {
        let bar = format_status_bar(90.0, 10);
        assert!(bar.contains(RED), "should be red: {}", bar);
        assert!(!bar.contains(YELLOW));
    }

    // ── 边界值测试 ──

    #[test]
    fn test_negative_clamped_to_zero() {
        let bar = strip_ansi_bar(&format_status_bar(-5.0, 10));
        assert_eq!(bar, "○○○○○○○○○○");
    }

    #[test]
    fn test_over_100_clamped() {
        let bar = strip_ansi_bar(&format_status_bar(200.0, 10));
        assert_eq!(bar, "●●●●●●●●●●");
    }

    #[test]
    fn test_color_boundary_69_green() {
        let bar = format_status_bar(69.0, 10);
        assert!(bar.contains(GREEN), "69% should be green");
        assert!(!bar.contains(YELLOW));
    }

    #[test]
    fn test_color_boundary_70_yellow() {
        let bar = format_status_bar(70.0, 10);
        assert!(bar.contains(YELLOW), "70% should be yellow");
        assert!(!bar.contains(RED));
    }

    #[test]
    fn test_color_boundary_84_yellow() {
        let bar = format_status_bar(84.0, 10);
        assert!(bar.contains(YELLOW), "84% should be yellow");
        assert!(!bar.contains(RED));
    }

    #[test]
    fn test_color_boundary_85_red() {
        let bar = format_status_bar(85.0, 10);
        assert!(bar.contains(RED), "85% should be red");
    }
}
