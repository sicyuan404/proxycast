//! Management API 处理器
//!
//! 提供服务器状态查询、凭证管理、配置管理等功能

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::database::dao::provider_pool::ProviderPoolDao;
use crate::server::AppState;

// ============ Types ============

/// 管理 API 状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementStatusResponse {
    /// 服务器是否运行中
    pub running: bool,
    /// 监听地址
    pub host: String,
    /// 监听端口
    pub port: u16,
    /// 处理的请求数
    pub requests: u64,
    /// 运行时间（秒）
    pub uptime_secs: u64,
    /// 版本号
    pub version: String,
    /// TLS 是否启用
    pub tls_enabled: bool,
    /// 默认 Provider
    pub default_provider: String,
}

/// 凭证信息（用于列表显示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialInfo {
    /// 凭证 ID
    pub id: String,
    /// Provider 类型
    pub provider_type: String,
    /// 是否禁用
    pub disabled: bool,
    /// 是否有效
    pub is_valid: bool,
}

/// 凭证列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsListResponse {
    /// 凭证列表
    pub credentials: Vec<CredentialInfo>,
    /// 总数
    pub total: usize,
}

/// 添加凭证请求
#[derive(Debug, Clone, Deserialize)]
pub struct AddCredentialRequest {
    /// Provider 类型
    pub provider_type: String,
    /// 凭证 ID
    pub id: String,
    /// API Key（用于 API Key 类型的凭证）
    #[serde(default)]
    pub api_key: Option<String>,
    /// Token 文件路径（用于 OAuth 类型的凭证）
    #[serde(default)]
    pub token_file: Option<String>,
    /// Base URL
    #[serde(default)]
    pub base_url: Option<String>,
    /// 代理 URL
    #[serde(default)]
    pub proxy_url: Option<String>,
}

/// 添加凭证响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCredentialResponse {
    /// 是否成功
    pub success: bool,
    /// 消息
    pub message: String,
    /// 凭证 ID
    pub id: Option<String>,
}

/// 配置响应（简化版，不包含敏感信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementConfigResponse {
    /// 服务器配置
    pub server: ManagementServerConfigInfo,
    /// 路由配置
    pub routing: ManagementRoutingConfigInfo,
    /// 重试配置
    pub retry: ManagementRetryConfigInfo,
    /// 远程管理配置（不包含 secret_key）
    pub remote_management: ManagementRemoteInfo,
}

/// 服务器配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementServerConfigInfo {
    pub host: String,
    pub port: u16,
    pub tls_enabled: bool,
}

/// 路由配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementRoutingConfigInfo {
    pub default_provider: String,
    pub rules_count: usize,
}

/// 重试配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementRetryConfigInfo {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

/// 远程管理配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementRemoteInfo {
    pub allow_remote: bool,
    pub has_secret_key: bool,
    pub disable_control_panel: bool,
}

/// 更新配置请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateConfigRequest {
    /// 默认 Provider
    #[serde(default)]
    pub default_provider: Option<String>,
    /// 是否允许远程访问
    #[serde(default)]
    pub allow_remote: Option<bool>,
}

/// 更新配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfigResponse {
    pub success: bool,
    pub message: String,
}

// ============ Handlers ============

/// GET /v0/management/status - 获取服务器状态
pub async fn management_status(State(state): State<AppState>) -> impl IntoResponse {
    let default_provider = state.default_provider.read().await.clone();

    // 获取请求数量
    let requests = state.processor.stats.read().len() as u64;

    let response = ManagementStatusResponse {
        running: true,
        host: "0.0.0.0".to_string(),
        port: 8999,
        requests,
        uptime_secs: 0, // TODO: Track actual uptime
        version: env!("CARGO_PKG_VERSION").to_string(),
        tls_enabled: false,
        default_provider,
    };

    Json(response)
}

/// GET /v0/management/credentials - 获取凭证列表
pub async fn management_list_credentials(State(state): State<AppState>) -> impl IntoResponse {
    let mut credentials = Vec::new();

    // 从数据库获取凭证列表
    if let Some(ref db) = state.db {
        if let Ok(conn) = db.lock() {
            if let Ok(pool_credentials) = ProviderPoolDao::get_all(&conn) {
                for cred in pool_credentials {
                    credentials.push(CredentialInfo {
                        id: cred.uuid.clone(),
                        provider_type: cred.provider_type.to_string(),
                        disabled: cred.is_disabled,
                        is_valid: cred.is_healthy,
                    });
                }
            }
        }
    }

    let total = credentials.len();
    Json(CredentialsListResponse { credentials, total })
}

/// POST /v0/management/credentials - 添加凭证
pub async fn management_add_credential(
    State(state): State<AppState>,
    Json(request): Json<AddCredentialRequest>,
) -> impl IntoResponse {
    use crate::models::provider_pool_model::{
        CredentialData, PoolProviderType, ProviderCredential,
    };

    // 验证请求
    if request.id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AddCredentialResponse {
                success: false,
                message: "Credential ID is required".to_string(),
                id: None,
            }),
        );
    }

    if request.provider_type.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AddCredentialResponse {
                success: false,
                message: "Provider type is required".to_string(),
                id: None,
            }),
        );
    }

    // 解析 provider 类型
    let provider_type: PoolProviderType = match request.provider_type.parse() {
        Ok(pt) => pt,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AddCredentialResponse {
                    success: false,
                    message: format!("Invalid provider type: {}", request.provider_type),
                    id: None,
                }),
            );
        }
    };

    // 根据 provider 类型创建凭证数据
    let credential_data = match provider_type {
        PoolProviderType::OpenAI => {
            if let Some(api_key) = request.api_key {
                CredentialData::OpenAIKey {
                    api_key,
                    base_url: request.base_url,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "API key is required for OpenAI provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::Claude => {
            if let Some(api_key) = request.api_key {
                CredentialData::ClaudeKey {
                    api_key,
                    base_url: request.base_url,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "API key is required for Claude provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::Vertex => {
            if let Some(api_key) = request.api_key {
                CredentialData::VertexKey {
                    api_key,
                    base_url: request.base_url,
                    model_aliases: std::collections::HashMap::new(),
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "API key is required for Vertex provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::Kiro => {
            if let Some(token_file) = request.token_file {
                CredentialData::KiroOAuth {
                    creds_file_path: token_file,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "Token file is required for Kiro provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::Gemini => {
            if let Some(token_file) = request.token_file {
                CredentialData::GeminiOAuth {
                    creds_file_path: token_file,
                    project_id: None,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "Token file is required for Gemini provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::Qwen => {
            if let Some(token_file) = request.token_file {
                CredentialData::QwenOAuth {
                    creds_file_path: token_file,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "Token file is required for Qwen provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::Antigravity => {
            if let Some(token_file) = request.token_file {
                CredentialData::AntigravityOAuth {
                    creds_file_path: token_file,
                    project_id: None,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "Token file is required for Antigravity provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::GeminiApiKey => {
            if let Some(api_key) = request.api_key {
                CredentialData::GeminiApiKey {
                    api_key,
                    base_url: request.base_url,
                    excluded_models: Vec::new(),
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "API key is required for Gemini API Key provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::Codex => {
            if let Some(token_file) = request.token_file {
                CredentialData::CodexOAuth {
                    creds_file_path: token_file,
                    api_base_url: request.base_url.clone(),
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "Token file is required for Codex provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::ClaudeOAuth => {
            if let Some(token_file) = request.token_file {
                CredentialData::ClaudeOAuth {
                    creds_file_path: token_file,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "Token file is required for Claude OAuth provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
        PoolProviderType::IFlow => {
            if let Some(token_file) = request.token_file {
                // 默认使用 OAuth 类型，Cookie 类型需要通过其他方式添加
                CredentialData::IFlowOAuth {
                    creds_file_path: token_file,
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddCredentialResponse {
                        success: false,
                        message: "Token file is required for iFlow provider".to_string(),
                        id: None,
                    }),
                );
            }
        }
    };

    // 创建凭证
    let mut credential = ProviderCredential::new(provider_type, credential_data);
    credential.uuid = request.id.clone();
    credential.name = Some(request.id.clone());

    // 添加凭证到数据库
    if let Some(ref db) = state.db {
        if let Ok(conn) = db.lock() {
            match ProviderPoolDao::insert(&conn, &credential) {
                Ok(_) => {
                    tracing::info!(
                        "[MANAGEMENT] Added credential: {} ({})",
                        request.id,
                        request.provider_type
                    );
                    return (
                        StatusCode::CREATED,
                        Json(AddCredentialResponse {
                            success: true,
                            message: "Credential added successfully".to_string(),
                            id: Some(request.id),
                        }),
                    );
                }
                Err(e) => {
                    tracing::error!("[MANAGEMENT] Failed to add credential: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AddCredentialResponse {
                            success: false,
                            message: format!("Failed to add credential: {}", e),
                            id: None,
                        }),
                    );
                }
            }
        }
    }

    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(AddCredentialResponse {
            success: false,
            message: "Database not available".to_string(),
            id: None,
        }),
    )
}

/// GET /v0/management/config - 获取配置
pub async fn management_get_config(State(state): State<AppState>) -> impl IntoResponse {
    let default_provider = state.default_provider.read().await.clone();

    // 获取路由规则数量
    let rules_count = state.processor.router.read().await.rules().len();

    let response = ManagementConfigResponse {
        server: ManagementServerConfigInfo {
            host: "0.0.0.0".to_string(),
            port: 8999,
            tls_enabled: false,
        },
        routing: ManagementRoutingConfigInfo {
            default_provider,
            rules_count,
        },
        retry: ManagementRetryConfigInfo {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        },
        remote_management: ManagementRemoteInfo {
            allow_remote: false,
            has_secret_key: true,
            disable_control_panel: false,
        },
    };

    Json(response)
}

/// PUT /v0/management/config - 更新配置
pub async fn management_update_config(
    State(state): State<AppState>,
    Json(request): Json<UpdateConfigRequest>,
) -> impl IntoResponse {
    let mut updated = false;

    // 更新默认 Provider
    if let Some(provider) = request.default_provider {
        // 验证 provider 类型
        if provider.parse::<crate::ProviderType>().is_ok() {
            let mut dp = state.default_provider.write().await;
            *dp = provider.clone();
            tracing::info!("[MANAGEMENT] Updated default_provider to: {}", provider);
            updated = true;
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(UpdateConfigResponse {
                    success: false,
                    message: format!("Invalid provider type: {}", provider),
                }),
            );
        }
    }

    if updated {
        (
            StatusCode::OK,
            Json(UpdateConfigResponse {
                success: true,
                message: "Configuration updated successfully".to_string(),
            }),
        )
    } else {
        (
            StatusCode::OK,
            Json(UpdateConfigResponse {
                success: true,
                message: "No changes applied".to_string(),
            }),
        )
    }
}
