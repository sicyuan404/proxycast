//! 配置管理命令
//!
//! 包含配置读取、保存、Provider 设置等命令。

use crate::app::types::{AppState, LogState};
use crate::config;

/// 获取配置
#[tauri::command]
pub async fn get_config(state: tauri::State<'_, AppState>) -> Result<config::Config, String> {
    let s = state.read().await;
    Ok(s.config.clone())
}

/// 保存配置
#[tauri::command]
pub async fn save_config(
    state: tauri::State<'_, AppState>,
    config: config::Config,
) -> Result<(), String> {
    // P0 安全修复：禁止危险的网络配置
    let host = config.server.host.to_lowercase();
    if host == "0.0.0.0" || host == "::" {
        return Err(
            "安全限制：不允许监听所有网络接口 (0.0.0.0 或 ::)。请使用 127.0.0.1 或 localhost"
                .to_string(),
        );
    }

    // 禁止开启远程管理
    if config.remote_management.allow_remote {
        return Err("安全限制：不允许开启远程管理功能".to_string());
    }

    let mut s = state.write().await;
    s.config = config.clone();
    config::save_config(&config).map_err(|e| e.to_string())
}

/// 获取默认 Provider
#[tauri::command]
pub async fn get_default_provider(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let s = state.read().await;
    Ok(s.config.default_provider.clone())
}

/// 设置默认 Provider
#[tauri::command]
pub async fn set_default_provider(
    state: tauri::State<'_, AppState>,
    logs: tauri::State<'_, LogState>,
    provider: String,
) -> Result<String, String> {
    // 允许任意 Provider ID（包括自定义 Provider 的 UUID）
    // 不再强制验证为已知的 ProviderType

    let mut s = state.write().await;
    s.config.default_provider = provider.clone();

    // 同时更新运行中服务器的 default_provider_ref
    {
        let mut dp = s.default_provider_ref.write().await;
        *dp = provider.clone();
    }

    config::save_config(&s.config).map_err(|e| e.to_string())?;
    logs.write()
        .await
        .add("info", &format!("默认 Provider 已切换为: {provider}"));
    Ok(provider)
}

/// 获取端点 Provider 配置
#[tauri::command]
pub async fn get_endpoint_providers(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let s = state.read().await;
    let ep = &s.config.endpoint_providers;
    Ok(serde_json::json!({
        "cursor": ep.cursor.clone(),
        "claude_code": ep.claude_code.clone(),
        "codex": ep.codex.clone(),
        "windsurf": ep.windsurf.clone(),
        "kiro": ep.kiro.clone(),
        "other": ep.other.clone()
    }))
}

/// 设置端点 Provider 配置
#[tauri::command]
pub async fn set_endpoint_provider(
    state: tauri::State<'_, AppState>,
    logs: tauri::State<'_, LogState>,
    endpoint: String,
    provider: Option<String>,
) -> Result<String, String> {
    // 允许任意 Provider ID（包括自定义 Provider 的 UUID）
    // 不再强制验证为已知的 ProviderType

    let mut s = state.write().await;

    // 使用 set_provider 方法设置对应的 provider
    if !s
        .config
        .endpoint_providers
        .set_provider(&endpoint, provider.clone())
    {
        return Err(format!("未知的客户端类型: {}", endpoint));
    }

    config::save_config(&s.config).map_err(|e| e.to_string())?;

    let provider_display = provider.as_deref().unwrap_or("默认");
    logs.write().await.add(
        "info",
        &format!(
            "客户端 {} 的 Provider 已设置为: {}",
            endpoint, provider_display
        ),
    );

    Ok(provider_display.to_string())
}
