use serde::{Deserialize, Serialize};
use std::sync::Mutex;

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

#[derive(Default)]
pub struct TrayData {
    pub balance: Option<BalanceInfo>,
    pub coding_plan: Option<CodingPlanInfo>,
}

pub struct AppState {
    pub client: reqwest::Client,
    pub refresh_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub tray_data: Mutex<TrayData>,
    pub minimize_to_tray: Mutex<bool>,
    /// 主窗口隐藏时刻，用于判断是否需要重新加载 WebView
    pub main_hidden_at: Mutex<Option<std::time::Instant>>,
}
