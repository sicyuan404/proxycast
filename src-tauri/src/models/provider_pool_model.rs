//! Provider Pool 数据模型
//!
//! 支持多凭证池管理，包括健康检测、负载均衡、故障转移等功能。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Provider 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PoolProviderType {
    Kiro,
    Gemini,
    Qwen,
    #[serde(rename = "openai")]
    OpenAI,
    Claude,
    Antigravity,
}

impl std::fmt::Display for PoolProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolProviderType::Kiro => write!(f, "kiro"),
            PoolProviderType::Gemini => write!(f, "gemini"),
            PoolProviderType::Qwen => write!(f, "qwen"),
            PoolProviderType::OpenAI => write!(f, "openai"),
            PoolProviderType::Claude => write!(f, "claude"),
            PoolProviderType::Antigravity => write!(f, "antigravity"),
        }
    }
}

impl std::str::FromStr for PoolProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "kiro" => Ok(PoolProviderType::Kiro),
            "gemini" => Ok(PoolProviderType::Gemini),
            "qwen" => Ok(PoolProviderType::Qwen),
            "openai" => Ok(PoolProviderType::OpenAI),
            "claude" => Ok(PoolProviderType::Claude),
            "antigravity" => Ok(PoolProviderType::Antigravity),
            _ => Err(format!("Invalid provider type: {s}")),
        }
    }
}

/// 凭证数据，根据 Provider 类型不同而不同
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CredentialData {
    /// Kiro OAuth 凭证（文件路径）
    KiroOAuth { creds_file_path: String },
    /// Gemini OAuth 凭证（文件路径）
    GeminiOAuth {
        creds_file_path: String,
        project_id: Option<String>,
    },
    /// Qwen OAuth 凭证（文件路径）
    QwenOAuth { creds_file_path: String },
    /// Antigravity OAuth 凭证（文件路径）- Google 内部 Gemini 3 Pro
    AntigravityOAuth {
        creds_file_path: String,
        project_id: Option<String>,
    },
    /// OpenAI API Key 凭证
    OpenAIKey {
        api_key: String,
        base_url: Option<String>,
    },
    /// Claude API Key 凭证
    ClaudeKey {
        api_key: String,
        base_url: Option<String>,
    },
}

impl CredentialData {
    /// 获取凭证的显示名称（隐藏敏感信息）
    pub fn display_name(&self) -> String {
        match self {
            CredentialData::KiroOAuth { creds_file_path } => {
                format!("Kiro OAuth: {}", mask_path(creds_file_path))
            }
            CredentialData::GeminiOAuth {
                creds_file_path, ..
            } => {
                format!("Gemini OAuth: {}", mask_path(creds_file_path))
            }
            CredentialData::QwenOAuth { creds_file_path } => {
                format!("Qwen OAuth: {}", mask_path(creds_file_path))
            }
            CredentialData::AntigravityOAuth {
                creds_file_path, ..
            } => {
                format!("Antigravity OAuth: {}", mask_path(creds_file_path))
            }
            CredentialData::OpenAIKey { api_key, .. } => {
                format!("OpenAI: {}", mask_key(api_key))
            }
            CredentialData::ClaudeKey { api_key, .. } => {
                format!("Claude: {}", mask_key(api_key))
            }
        }
    }

    /// 获取 Provider 类型
    pub fn provider_type(&self) -> PoolProviderType {
        match self {
            CredentialData::KiroOAuth { .. } => PoolProviderType::Kiro,
            CredentialData::GeminiOAuth { .. } => PoolProviderType::Gemini,
            CredentialData::QwenOAuth { .. } => PoolProviderType::Qwen,
            CredentialData::AntigravityOAuth { .. } => PoolProviderType::Antigravity,
            CredentialData::OpenAIKey { .. } => PoolProviderType::OpenAI,
            CredentialData::ClaudeKey { .. } => PoolProviderType::Claude,
        }
    }
}

/// 单个凭证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredential {
    /// 唯一标识符
    pub uuid: String,
    /// Provider 类型
    pub provider_type: PoolProviderType,
    /// 凭证数据
    pub credential: CredentialData,
    /// 备注/名称
    pub name: Option<String>,
    /// 是否健康
    #[serde(default = "default_true")]
    pub is_healthy: bool,
    /// 是否禁用（手动禁用）
    #[serde(default)]
    pub is_disabled: bool,
    /// 是否启用自动健康检查
    #[serde(default = "default_true")]
    pub check_health: bool,
    /// 自定义健康检查模型
    pub check_model_name: Option<String>,
    /// 不支持的模型列表（黑名单）
    #[serde(default)]
    pub not_supported_models: Vec<String>,
    /// 使用次数
    #[serde(default)]
    pub usage_count: u64,
    /// 错误次数
    #[serde(default)]
    pub error_count: u32,
    /// 最后使用时间
    pub last_used: Option<DateTime<Utc>>,
    /// 最后错误时间
    pub last_error_time: Option<DateTime<Utc>>,
    /// 最后错误消息
    pub last_error_message: Option<String>,
    /// 最后健康检查时间
    pub last_health_check_time: Option<DateTime<Utc>>,
    /// 最后健康检查使用的模型
    pub last_health_check_model: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// Token 缓存信息
    #[serde(default)]
    pub cached_token: Option<CachedTokenInfo>,
}

fn default_true() -> bool {
    true
}

impl ProviderCredential {
    /// 创建新凭证
    pub fn new(provider_type: PoolProviderType, credential: CredentialData) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::new_v4().to_string(),
            provider_type,
            credential,
            name: None,
            is_healthy: true,
            is_disabled: false,
            check_health: true,
            check_model_name: None,
            not_supported_models: Vec::new(),
            usage_count: 0,
            error_count: 0,
            last_used: None,
            last_error_time: None,
            last_error_message: None,
            last_health_check_time: None,
            last_health_check_model: None,
            created_at: now,
            updated_at: now,
            cached_token: None,
        }
    }

    /// 是否可用（健康且未禁用）
    pub fn is_available(&self) -> bool {
        self.is_healthy && !self.is_disabled
    }

    /// 是否支持指定模型
    pub fn supports_model(&self, model: &str) -> bool {
        !self.not_supported_models.contains(&model.to_string())
    }

    /// 标记为健康
    pub fn mark_healthy(&mut self, check_model: Option<String>) {
        self.is_healthy = true;
        self.error_count = 0;
        self.last_health_check_time = Some(Utc::now());
        self.last_health_check_model = check_model;
        self.updated_at = Utc::now();
    }

    /// 标记为不健康
    pub fn mark_unhealthy(&mut self, error_message: Option<String>) {
        self.error_count += 1;
        self.last_error_time = Some(Utc::now());
        self.last_error_message = error_message;
        self.updated_at = Utc::now();
        // 错误次数达到阈值则标记为不健康
        if self.error_count >= 3 {
            self.is_healthy = false;
        }
    }

    /// 记录使用
    pub fn record_usage(&mut self) {
        self.usage_count += 1;
        self.last_used = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// 重置计数器
    pub fn reset_counters(&mut self) {
        self.usage_count = 0;
        self.error_count = 0;
        self.is_healthy = true;
        self.last_error_time = None;
        self.last_error_message = None;
        self.updated_at = Utc::now();
    }
}

/// 凭证池统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    /// 总凭证数
    pub total_count: usize,
    /// 健康凭证数
    pub healthy_count: usize,
    /// 禁用凭证数
    pub disabled_count: usize,
    /// 总使用次数
    pub total_usage: u64,
    /// 总错误次数
    pub total_errors: u64,
    /// 最后更新时间
    pub last_update: DateTime<Utc>,
}

impl PoolStats {
    pub fn from_credentials(credentials: &[ProviderCredential]) -> Self {
        Self {
            total_count: credentials.len(),
            healthy_count: credentials.iter().filter(|c| c.is_healthy).count(),
            disabled_count: credentials.iter().filter(|c| c.is_disabled).count(),
            total_usage: credentials.iter().map(|c| c.usage_count).sum(),
            total_errors: credentials.iter().map(|c| c.error_count as u64).sum(),
            last_update: Utc::now(),
        }
    }
}

/// 健康检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub uuid: String,
    pub success: bool,
    pub model: Option<String>,
    pub message: Option<String>,
    pub duration_ms: u64,
}

/// OAuth 凭证状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthStatus {
    /// 是否有 access_token
    pub has_access_token: bool,
    /// 是否有 refresh_token
    pub has_refresh_token: bool,
    /// token 是否有效
    pub is_token_valid: bool,
    /// 过期信息
    pub expiry_info: Option<String>,
    /// 凭证文件路径
    pub creds_path: String,
}

/// Token 缓存状态（用于前端展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheStatus {
    /// 是否有缓存的 token
    pub has_cached_token: bool,
    /// Token 是否有效
    pub is_valid: bool,
    /// Token 是否即将过期（5分钟内）
    pub is_expiring_soon: bool,
    /// 过期时间
    pub expiry_time: Option<String>,
    /// 最后刷新时间
    pub last_refresh: Option<String>,
    /// 连续刷新失败次数
    pub refresh_error_count: u32,
    /// 最后刷新错误信息
    pub last_refresh_error: Option<String>,
}

/// Token 缓存信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachedTokenInfo {
    /// 缓存的 access_token
    pub access_token: Option<String>,
    /// 缓存的 refresh_token（刷新后可能变化）
    pub refresh_token: Option<String>,
    /// Token 过期时间
    pub expiry_time: Option<DateTime<Utc>>,
    /// 最后刷新时间
    pub last_refresh: Option<DateTime<Utc>>,
    /// 连续刷新失败次数
    #[serde(default)]
    pub refresh_error_count: u32,
    /// 最后刷新错误信息
    pub last_refresh_error: Option<String>,
}

impl CachedTokenInfo {
    /// 检查 token 是否有效（存在且未过期）
    pub fn is_valid(&self) -> bool {
        if self.access_token.is_none() {
            return false;
        }
        match &self.expiry_time {
            Some(expiry) => *expiry > Utc::now(),
            None => true, // 没有过期时间，假设有效
        }
    }

    /// 检查 token 是否即将过期（5分钟内）
    pub fn is_expiring_soon(&self) -> bool {
        match &self.expiry_time {
            Some(expiry) => {
                let threshold = Utc::now() + chrono::Duration::minutes(5);
                *expiry <= threshold
            }
            None => false, // 没有过期时间，假设不会过期
        }
    }

    /// 检查 token 是否需要刷新（无效或即将过期）
    pub fn needs_refresh(&self) -> bool {
        !self.is_valid() || self.is_expiring_soon()
    }
}

/// 默认健康检查模型
pub fn get_default_check_model(provider_type: PoolProviderType) -> &'static str {
    match provider_type {
        PoolProviderType::Kiro => "claude-haiku-4-5",
        PoolProviderType::Gemini => "gemini-2.5-flash",
        PoolProviderType::Qwen => "qwen3-coder-flash",
        PoolProviderType::OpenAI => "gpt-3.5-turbo",
        PoolProviderType::Claude => "claude-3-5-haiku-latest",
        PoolProviderType::Antigravity => "gemini-3-pro-preview",
    }
}

/// 凭证池前端展示数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialDisplay {
    pub uuid: String,
    pub provider_type: String,
    pub credential_type: String,
    pub name: Option<String>,
    pub display_credential: String,
    pub is_healthy: bool,
    pub is_disabled: bool,
    pub check_health: bool,
    pub check_model_name: Option<String>,
    pub not_supported_models: Vec<String>,
    pub usage_count: u64,
    pub error_count: u32,
    pub last_used: Option<String>,
    pub last_error_time: Option<String>,
    pub last_error_message: Option<String>,
    pub last_health_check_time: Option<String>,
    pub last_health_check_model: Option<String>,
    pub oauth_status: Option<OAuthStatus>,
    pub token_cache_status: Option<TokenCacheStatus>,
    pub created_at: String,
    pub updated_at: String,
}

/// 获取凭证类型字符串
fn get_credential_type(cred: &CredentialData) -> String {
    match cred {
        CredentialData::KiroOAuth { .. } => "kiro_oauth".to_string(),
        CredentialData::GeminiOAuth { .. } => "gemini_oauth".to_string(),
        CredentialData::QwenOAuth { .. } => "qwen_oauth".to_string(),
        CredentialData::AntigravityOAuth { .. } => "antigravity_oauth".to_string(),
        CredentialData::OpenAIKey { .. } => "openai_key".to_string(),
        CredentialData::ClaudeKey { .. } => "claude_key".to_string(),
    }
}

/// 获取 OAuth 凭证的文件路径
pub fn get_oauth_creds_path(cred: &CredentialData) -> Option<String> {
    match cred {
        CredentialData::KiroOAuth { creds_file_path } => Some(creds_file_path.clone()),
        CredentialData::GeminiOAuth {
            creds_file_path, ..
        } => Some(creds_file_path.clone()),
        CredentialData::QwenOAuth { creds_file_path } => Some(creds_file_path.clone()),
        CredentialData::AntigravityOAuth {
            creds_file_path, ..
        } => Some(creds_file_path.clone()),
        _ => None,
    }
}

impl From<&ProviderCredential> for CredentialDisplay {
    fn from(cred: &ProviderCredential) -> Self {
        // 构建 token 缓存状态
        let token_cache_status = cred.cached_token.as_ref().map(|cache| TokenCacheStatus {
            has_cached_token: cache.access_token.is_some(),
            is_valid: cache.is_valid(),
            is_expiring_soon: cache.is_expiring_soon(),
            expiry_time: cache.expiry_time.map(|t| t.to_rfc3339()),
            last_refresh: cache.last_refresh.map(|t| t.to_rfc3339()),
            refresh_error_count: cache.refresh_error_count,
            last_refresh_error: cache.last_refresh_error.clone(),
        });

        Self {
            uuid: cred.uuid.clone(),
            provider_type: cred.provider_type.to_string(),
            credential_type: get_credential_type(&cred.credential),
            name: cred.name.clone(),
            display_credential: cred.credential.display_name(),
            is_healthy: cred.is_healthy,
            is_disabled: cred.is_disabled,
            check_health: cred.check_health,
            check_model_name: cred.check_model_name.clone(),
            not_supported_models: cred.not_supported_models.clone(),
            usage_count: cred.usage_count,
            error_count: cred.error_count,
            last_used: cred.last_used.map(|t| t.to_rfc3339()),
            last_error_time: cred.last_error_time.map(|t| t.to_rfc3339()),
            last_error_message: cred.last_error_message.clone(),
            last_health_check_time: cred.last_health_check_time.map(|t| t.to_rfc3339()),
            last_health_check_model: cred.last_health_check_model.clone(),
            oauth_status: None, // 需要单独调用获取
            token_cache_status,
            created_at: cred.created_at.to_rfc3339(),
            updated_at: cred.updated_at.to_rfc3339(),
        }
    }
}

/// Provider 池概览（按类型分组的统计）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPoolOverview {
    pub provider_type: String,
    pub stats: PoolStats,
    pub credentials: Vec<CredentialDisplay>,
}

// 辅助函数：隐藏路径中的用户名
fn mask_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        path.replace(&*home_str, "~")
    } else {
        path.to_string()
    }
}

// 辅助函数：隐藏 API Key
fn mask_key(key: &str) -> String {
    if key.len() <= 12 {
        "****".to_string()
    } else {
        format!("{}...{}", &key[..6], &key[key.len() - 4..])
    }
}

/// 添加凭证的请求结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCredentialRequest {
    pub provider_type: String,
    pub credential: CredentialData,
    pub name: Option<String>,
    pub check_health: Option<bool>,
    pub check_model_name: Option<String>,
}

/// 更新凭证的请求结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCredentialRequest {
    pub name: Option<String>,
    pub is_disabled: Option<bool>,
    pub check_health: Option<bool>,
    pub check_model_name: Option<String>,
    pub not_supported_models: Option<Vec<String>>,
    /// 新的凭证文件路径（仅适用于OAuth凭证，用于重新上传文件）
    pub new_creds_file_path: Option<String>,
    /// OAuth相关：新的project_id（仅适用于Gemini）
    pub new_project_id: Option<String>,
}

pub type ProviderPools = HashMap<PoolProviderType, Vec<ProviderCredential>>;
