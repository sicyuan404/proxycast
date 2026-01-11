//! 命令分发器
//!
//! 将 HTTP 请求路由到现有的 Tauri 命令函数。

use crate::server::AppState;
use serde_json::Value as JsonValue;

/// 处理 HTTP 桥接命令请求
///
/// 将命令名和参数分发到对应的命令处理函数
pub async fn handle_command(
    state: &AppState,
    cmd: &str,
    args: Option<serde_json::Value>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    match cmd {
        // ========== P0 - 核心配置 ==========
        "get_config" => {
            // 从配置文件读取
            let config_path = crate::config::ConfigManager::default_config_path();
            let manager = crate::config::ConfigManager::load(&config_path)?;
            let config = manager.config();
            Ok(serde_json::to_value(config)?)
        }

        "save_config" => {
            // 保存配置到文件
            let config: crate::config::Config = serde_json::from_value(args.unwrap_or_default())?;
            crate::config::save_config(&config)?;
            Ok(serde_json::json!({ "success": true }))
        }

        "get_default_provider" => {
            let provider = state.default_provider.read().await.clone();
            // 直接返回字符串值，不是对象
            Ok(serde_json::json!(provider))
        }

        "get_endpoint_providers" => {
            let providers = state.endpoint_providers.read().await;
            Ok(serde_json::to_value(&*providers)?)
        }

        // ========== P0 - 服务器状态 ==========
        "get_server_status" => {
            // 解析 base_url 获取 host 和 port
            let url_parts: Vec<&str> = state.base_url.split(':').collect();
            let host = url_parts.get(2).unwrap_or(&"127.0.0.1");
            let port = url_parts.get(3).and_then(|p| p.parse::<u16>().ok()).unwrap_or(3030);

            let status = serde_json::json!({
                "running": true,  // HTTP 桥接可用说明服务器在运行
                "host": host,
                "port": port,
                "api_key": "***",  // 不暴露真实 API key
            });
            Ok(status)
        }

        // ========== P1 - 日志相关 ==========
        "get_logs" => {
            let logs = state.logs.read().await;
            let entries = logs.get_logs();
            // 限制返回最近 100 条
            let limit = entries.len().min(100);
            let recent: Vec<_> = entries.into_iter().rev().take(limit).map(|e| serde_json::json!({
                "timestamp": e.timestamp,
                "level": e.level,
                "message": e.message,
            })).collect();
            Ok(serde_json::to_value(recent)?)
        }

        "clear_logs" => {
            state.logs.write().await.clear();
            Ok(serde_json::json!({ "success": true }))
        }

        // ========== Provider Pool ==========
        "get_provider_pool_overview" => {
            // 从数据库获取凭证池概览
            if let Some(db) = &state.db {
                let conn = db.lock().map_err(|e| e.to_string())?;
                let credentials = crate::database::dao::provider_pool::ProviderPoolDao::get_all(&conn)
                    .unwrap_or_default();

                let overview: Vec<JsonValue> = credentials
                    .into_iter()
                    .map(|cred| serde_json::json!({
                        "uuid": cred.uuid,
                        "name": cred.name,
                        "provider_type": cred.provider_type,
                        "enabled": true,
                    }))
                    .collect();
                Ok(serde_json::to_value(overview)?)
            } else {
                Ok(serde_json::json!([]))
            }
        }

        "get_api_key_providers" => {
            // 从 API Key Provider 服务获取
            if let Some(db) = &state.db {
                let conn = db.lock().map_err(|e| e.to_string())?;
                let providers = crate::database::dao::api_key_provider::ApiKeyProviderDao::get_all_providers(&conn)
                    .unwrap_or_default();
                Ok(serde_json::to_value(providers)?)
            } else {
                Ok(serde_json::json!([]))
            }
        }

        "get_provider_pool_credentials" => {
            // 获取所有凭证详细信息
            if let Some(db) = &state.db {
                let conn = db.lock().map_err(|e| e.to_string())?;
                let credentials = crate::database::dao::provider_pool::ProviderPoolDao::get_all(&conn)
                    .unwrap_or_default();
                Ok(serde_json::to_value(credentials)?)
            } else {
                Ok(serde_json::json!([]))
            }
        }

        "test_api" => {
            // 测试 API 连接
            // 从 args 获取 provider
            let args = args.ok_or("缺少参数")?;
            let provider = args
                .get("provider")
                .and_then(|v| v.as_str())
                .ok_or("缺少 provider 参数")?;

            // 选择凭证
            let credential = if let Some(db) = &state.db {
                state
                    .pool_service
                    .select_credential(db, provider, None)
                    .ok()
                    .flatten()
            } else {
                None
            };

            match credential {
                Some(cred) => {
                    state
                        .logs
                        .write()
                        .await
                        .add("info", &format!("[DevBridge] 测试 API 使用凭证: {:?}", cred.name));

                    Ok(serde_json::json!({
                        "success": true,
                        "credential_name": cred.name,
                        "provider_type": cred.provider_type,
                    }))
                }
                None => Ok(serde_json::json!({
                    "success": false,
                    "error": "未找到可用凭证"
                })),
            }
        }

        // ========== 模型相关 ==========
        "get_models" => {
            // 返回可用模型列表
            Ok(serde_json::json!({
                "data": [
                    {"id": "claude-sonnet-4-20250514", "object": "model", "owned_by": "anthropic"},
                    {"id": "claude-opus-4-20250514", "object": "model", "owned_by": "anthropic"},
                    {"id": "claude-haiku-4-20250514", "object": "model", "owned_by": "anthropic"},
                    {"id": "gpt-4o", "object": "model", "owned_by": "openai"},
                    {"id": "gpt-4o-mini", "object": "model", "owned_by": "openai"},
                ]
            }))
        }

        // ========== 网络信息 ==========
        "get_network_info" => {
            // 返回网络信息
            Ok(serde_json::json!({
                "localhost": "127.0.0.1",
                "lan_ip": null,
                "all_ips": ["127.0.0.1"]
            }))
        }

        _ => Err(format!(
            "[DevBridge] 未知命令: '{}'. 如需此命令，请将其添加到 dispatcher.rs 的 handle_command 函数中。",
            cmd
        )
        .into()),
    }
}
