//! Provider Pool 数据模型
//!
//! 支持多凭证池管理，包括健康检测、负载均衡、故障转移等功能。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 凭证来源枚举
/// 用于标识凭证是如何添加到凭证池的
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    /// 手动添加（通过 UI 添加）
    #[default]
    Manual,
    /// 导入（从文件导入）
    Imported,
    /// 私有凭证（从高级设置迁移）
    Private,
}

/// Provider 类型别名
///
/// 为了向后兼容，PoolProviderType 是 crate::ProviderType 的类型别名。
/// 所有 Provider 类型定义已统一到 lib.rs 中的 ProviderType。
pub type PoolProviderType = crate::ProviderType;

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
    /// Vertex AI API Key 凭证
    VertexKey {
        api_key: String,
        base_url: Option<String>,
        /// Model alias mappings (alias -> upstream model name)
        #[serde(default)]
        model_aliases: std::collections::HashMap<String, String>,
    },
    /// Gemini API Key 凭证（多账号负载均衡）
    GeminiApiKey {
        api_key: String,
        base_url: Option<String>,
        /// 排除的模型列表（支持通配符）
        #[serde(default)]
        excluded_models: Vec<String>,
    },
    /// Codex OAuth 凭证（OpenAI Codex）
    CodexOAuth {
        creds_file_path: String,
        /// API Base URL（可选，默认使用凭证文件中的配置）
        #[serde(default)]
        api_base_url: Option<String>,
    },
    /// Claude OAuth 凭证（Anthropic OAuth）
    ClaudeOAuth { creds_file_path: String },
    /// iFlow OAuth 凭证
    IFlowOAuth { creds_file_path: String },
    /// iFlow Cookie 凭证
    IFlowCookie { creds_file_path: String },
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
            CredentialData::VertexKey { api_key, .. } => {
                format!("Vertex AI: {}", mask_key(api_key))
            }
            CredentialData::GeminiApiKey { api_key, .. } => {
                format!("Gemini API Key: {}", mask_key(api_key))
            }
            CredentialData::CodexOAuth {
                creds_file_path, ..
            } => {
                format!("Codex OAuth: {}", mask_path(creds_file_path))
            }
            CredentialData::ClaudeOAuth { creds_file_path } => {
                format!("Claude OAuth: {}", mask_path(creds_file_path))
            }
            CredentialData::IFlowOAuth { creds_file_path } => {
                format!("iFlow OAuth: {}", mask_path(creds_file_path))
            }
            CredentialData::IFlowCookie { creds_file_path } => {
                format!("iFlow Cookie: {}", mask_path(creds_file_path))
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
            CredentialData::VertexKey { .. } => PoolProviderType::Vertex,
            CredentialData::GeminiApiKey { .. } => PoolProviderType::GeminiApiKey,
            CredentialData::CodexOAuth { .. } => PoolProviderType::Codex,
            CredentialData::ClaudeOAuth { .. } => PoolProviderType::ClaudeOAuth,
            CredentialData::IFlowOAuth { .. } => PoolProviderType::IFlow,
            CredentialData::IFlowCookie { .. } => PoolProviderType::IFlow,
        }
    }
}

/// 通配符模式匹配
///
/// 支持的通配符模式：
/// - 精确匹配: `claude-sonnet-4-5`
/// - 前缀匹配: `claude-*`
/// - 后缀匹配: `*-preview`
/// - 包含匹配: `*flash*`
pub fn pattern_matches(pattern: &str, model: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == model;
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    match parts.as_slice() {
        [prefix, ""] => model.starts_with(prefix),
        ["", suffix] => model.ends_with(suffix),
        ["", middle, ""] => model.contains(middle),
        [prefix, suffix] => model.starts_with(prefix) && model.ends_with(suffix),
        _ => false,
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
    /// 凭证来源（手动添加/导入/私有）
    #[serde(default)]
    pub source: CredentialSource,
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
            source: CredentialSource::Manual,
        }
    }

    /// 创建带来源的新凭证
    pub fn new_with_source(
        provider_type: PoolProviderType,
        credential: CredentialData,
        source: CredentialSource,
    ) -> Self {
        let mut cred = Self::new(provider_type, credential);
        cred.source = source;
        cred
    }

    /// 是否可用（健康且未禁用）
    pub fn is_available(&self) -> bool {
        self.is_healthy && !self.is_disabled
    }

    /// 是否支持指定模型
    ///
    /// 检查两个来源的排除列表：
    /// 1. `not_supported_models` - 通用的不支持模型列表（精确匹配）
    /// 2. `excluded_models` - 来自 CredentialData::GeminiApiKey 的排除列表（支持通配符）
    /// 3. Antigravity 凭证只支持特定的模型列表
    pub fn supports_model(&self, model: &str) -> bool {
        // 检查通用的不支持模型列表（精确匹配）
        if self.not_supported_models.contains(&model.to_string()) {
            return false;
        }

        // 检查 GeminiApiKey 的 excluded_models（支持通配符）
        if let CredentialData::GeminiApiKey {
            excluded_models, ..
        } = &self.credential
        {
            for pattern in excluded_models {
                if pattern_matches(pattern, model) {
                    return false;
                }
            }
        }

        // Antigravity 凭证只支持特定的模型
        if let CredentialData::AntigravityOAuth { .. } = &self.credential {
            // Antigravity 支持的模型列表
            const ANTIGRAVITY_SUPPORTED_MODELS: &[&str] = &[
                "gemini-3-pro-preview",
                "gemini-3-pro-image-preview",
                "gemini-2.5-computer-use-preview-10-2025",
                "gemini-claude-sonnet-4-5",
                "gemini-claude-sonnet-4-5-thinking",
            ];
            return ANTIGRAVITY_SUPPORTED_MODELS.contains(&model);
        }

        true
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
        // 使用 claude-sonnet-4-5-20250929，兼容更多代理服务器
        PoolProviderType::Claude => "claude-sonnet-4-5-20250929",
        PoolProviderType::Antigravity => "gemini-3-pro-preview",
        PoolProviderType::Vertex => "gemini-2.0-flash",
        PoolProviderType::GeminiApiKey => "gemini-2.5-flash",
        PoolProviderType::Codex => "gpt-4o-mini",
        PoolProviderType::ClaudeOAuth => "claude-sonnet-4-5-20250929",
        PoolProviderType::IFlow => "deepseek-chat",
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
    /// 凭证来源（手动添加/导入/私有）
    pub source: CredentialSource,
    /// API Key 凭证的 base_url（仅用于 OpenAI/Claude API Key 类型）
    pub base_url: Option<String>,
    /// API Key 凭证的完整 api_key（仅用于 OpenAI/Claude API Key 类型，用于编辑）
    pub api_key: Option<String>,
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
        CredentialData::VertexKey { .. } => "vertex_key".to_string(),
        CredentialData::GeminiApiKey { .. } => "gemini_api_key".to_string(),
        CredentialData::CodexOAuth { .. } => "codex_oauth".to_string(),
        CredentialData::ClaudeOAuth { .. } => "claude_oauth".to_string(),
        CredentialData::IFlowOAuth { .. } => "iflow_oauth".to_string(),
        CredentialData::IFlowCookie { .. } => "iflow_cookie".to_string(),
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
        CredentialData::CodexOAuth {
            creds_file_path, ..
        } => Some(creds_file_path.clone()),
        CredentialData::ClaudeOAuth { creds_file_path } => Some(creds_file_path.clone()),
        CredentialData::IFlowOAuth { creds_file_path } => Some(creds_file_path.clone()),
        CredentialData::IFlowCookie { creds_file_path } => Some(creds_file_path.clone()),
        _ => None,
    }
}

/// 从 CredentialData 中提取 base_url（仅适用于 API Key 类型）
fn get_base_url(cred: &CredentialData) -> Option<String> {
    match cred {
        CredentialData::OpenAIKey { base_url, .. } => base_url.clone(),
        CredentialData::ClaudeKey { base_url, .. } => base_url.clone(),
        _ => None,
    }
}

/// 从 CredentialData 中提取 api_key（仅适用于 API Key 类型）
fn get_api_key(cred: &CredentialData) -> Option<String> {
    match cred {
        CredentialData::OpenAIKey { api_key, .. } => Some(api_key.clone()),
        CredentialData::ClaudeKey { api_key, .. } => Some(api_key.clone()),
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
            source: cred.source,
            base_url: get_base_url(&cred.credential),
            api_key: get_api_key(&cred.credential),
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
    /// API Key 相关：新的 base_url（仅适用于 API Key 凭证）
    pub new_base_url: Option<String>,
    /// API Key 相关：新的 api_key（仅适用于 API Key 凭证）
    pub new_api_key: Option<String>,
}

pub type ProviderPools = HashMap<PoolProviderType, Vec<ProviderCredential>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matches_exact() {
        assert!(pattern_matches("gemini-2.5-pro", "gemini-2.5-pro"));
        assert!(!pattern_matches("gemini-2.5-pro", "gemini-2.5-flash"));
    }

    #[test]
    fn test_pattern_matches_prefix() {
        assert!(pattern_matches("gemini-*", "gemini-2.5-pro"));
        assert!(pattern_matches("gemini-*", "gemini-2.5-flash"));
        assert!(!pattern_matches("gemini-*", "claude-sonnet"));
    }

    #[test]
    fn test_pattern_matches_suffix() {
        assert!(pattern_matches("*-preview", "gemini-3-pro-preview"));
        assert!(pattern_matches("*-preview", "claude-preview"));
        assert!(!pattern_matches("*-preview", "gemini-2.5-pro"));
    }

    #[test]
    fn test_pattern_matches_contains() {
        assert!(pattern_matches("*flash*", "gemini-2.5-flash"));
        assert!(pattern_matches("*flash*", "gemini-2.5-flash-lite"));
        assert!(!pattern_matches("*flash*", "gemini-2.5-pro"));
    }

    #[test]
    fn test_pattern_matches_prefix_and_suffix() {
        assert!(pattern_matches("gemini-*-pro", "gemini-2.5-pro"));
        assert!(pattern_matches("gemini-*-pro", "gemini-3-pro"));
        assert!(!pattern_matches("gemini-*-pro", "gemini-2.5-flash"));
    }

    #[test]
    fn test_supports_model_not_supported_models() {
        let cred = ProviderCredential {
            uuid: "test-uuid".to_string(),
            provider_type: PoolProviderType::Kiro,
            credential: CredentialData::KiroOAuth {
                creds_file_path: "/path/to/creds".to_string(),
            },
            name: None,
            is_healthy: true,
            is_disabled: false,
            check_health: true,
            check_model_name: None,
            not_supported_models: vec!["claude-opus".to_string()],
            usage_count: 0,
            error_count: 0,
            last_used: None,
            last_error_time: None,
            last_error_message: None,
            last_health_check_time: None,
            last_health_check_model: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cached_token: None,
            source: CredentialSource::Manual,
        };

        assert!(!cred.supports_model("claude-opus"));
        assert!(cred.supports_model("claude-sonnet"));
    }

    #[test]
    fn test_supports_model_gemini_api_key_excluded_models_exact() {
        let cred = ProviderCredential {
            uuid: "test-uuid".to_string(),
            provider_type: PoolProviderType::GeminiApiKey,
            credential: CredentialData::GeminiApiKey {
                api_key: "test-key".to_string(),
                base_url: None,
                excluded_models: vec!["gemini-2.5-pro".to_string()],
            },
            name: None,
            is_healthy: true,
            is_disabled: false,
            check_health: true,
            check_model_name: None,
            not_supported_models: vec![],
            usage_count: 0,
            error_count: 0,
            last_used: None,
            last_error_time: None,
            last_error_message: None,
            last_health_check_time: None,
            last_health_check_model: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cached_token: None,
            source: CredentialSource::Manual,
        };

        // Exact match exclusion
        assert!(!cred.supports_model("gemini-2.5-pro"));
        // Not excluded
        assert!(cred.supports_model("gemini-2.5-flash"));
    }

    #[test]
    fn test_supports_model_gemini_api_key_excluded_models_wildcard() {
        let cred = ProviderCredential {
            uuid: "test-uuid".to_string(),
            provider_type: PoolProviderType::GeminiApiKey,
            credential: CredentialData::GeminiApiKey {
                api_key: "test-key".to_string(),
                base_url: None,
                excluded_models: vec!["gemini-2.5-*".to_string(), "*-preview".to_string()],
            },
            name: None,
            is_healthy: true,
            is_disabled: false,
            check_health: true,
            check_model_name: None,
            not_supported_models: vec![],
            usage_count: 0,
            error_count: 0,
            last_used: None,
            last_error_time: None,
            last_error_message: None,
            last_health_check_time: None,
            last_health_check_model: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cached_token: None,
            source: CredentialSource::Manual,
        };

        // Prefix wildcard exclusion
        assert!(!cred.supports_model("gemini-2.5-pro"));
        assert!(!cred.supports_model("gemini-2.5-flash"));
        // Suffix wildcard exclusion
        assert!(!cred.supports_model("gemini-3-pro-preview"));
        // Not excluded
        assert!(cred.supports_model("gemini-2.0-flash"));
        assert!(cred.supports_model("gemini-3-pro"));
    }

    #[test]
    fn test_supports_model_gemini_api_key_excluded_models_contains() {
        let cred = ProviderCredential {
            uuid: "test-uuid".to_string(),
            provider_type: PoolProviderType::GeminiApiKey,
            credential: CredentialData::GeminiApiKey {
                api_key: "test-key".to_string(),
                base_url: None,
                excluded_models: vec!["*flash*".to_string()],
            },
            name: None,
            is_healthy: true,
            is_disabled: false,
            check_health: true,
            check_model_name: None,
            not_supported_models: vec![],
            usage_count: 0,
            error_count: 0,
            last_used: None,
            last_error_time: None,
            last_error_message: None,
            last_health_check_time: None,
            last_health_check_model: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cached_token: None,
            source: CredentialSource::Manual,
        };

        // Contains wildcard exclusion
        assert!(!cred.supports_model("gemini-2.5-flash"));
        assert!(!cred.supports_model("gemini-2.5-flash-lite"));
        // Not excluded
        assert!(cred.supports_model("gemini-2.5-pro"));
    }

    #[test]
    fn test_supports_model_combined_exclusions() {
        let cred = ProviderCredential {
            uuid: "test-uuid".to_string(),
            provider_type: PoolProviderType::GeminiApiKey,
            credential: CredentialData::GeminiApiKey {
                api_key: "test-key".to_string(),
                base_url: None,
                excluded_models: vec!["gemini-2.5-*".to_string()],
            },
            name: None,
            is_healthy: true,
            is_disabled: false,
            check_health: true,
            check_model_name: None,
            not_supported_models: vec!["gemini-3-pro".to_string()],
            usage_count: 0,
            error_count: 0,
            last_used: None,
            last_error_time: None,
            last_error_message: None,
            last_health_check_time: None,
            last_health_check_model: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cached_token: None,
            source: CredentialSource::Manual,
        };

        // Excluded by not_supported_models (exact match)
        assert!(!cred.supports_model("gemini-3-pro"));
        // Excluded by excluded_models (wildcard)
        assert!(!cred.supports_model("gemini-2.5-pro"));
        assert!(!cred.supports_model("gemini-2.5-flash"));
        // Not excluded
        assert!(cred.supports_model("gemini-2.0-flash"));
    }

    #[test]
    fn test_supports_model_non_gemini_api_key_ignores_excluded_models() {
        // For non-GeminiApiKey credentials, excluded_models in CredentialData is not checked
        let cred = ProviderCredential {
            uuid: "test-uuid".to_string(),
            provider_type: PoolProviderType::Kiro,
            credential: CredentialData::KiroOAuth {
                creds_file_path: "/path/to/creds".to_string(),
            },
            name: None,
            is_healthy: true,
            is_disabled: false,
            check_health: true,
            check_model_name: None,
            not_supported_models: vec![],
            usage_count: 0,
            error_count: 0,
            last_used: None,
            last_error_time: None,
            last_error_message: None,
            last_health_check_time: None,
            last_health_check_model: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cached_token: None,
            source: CredentialSource::Manual,
        };

        // All models should be supported since not_supported_models is empty
        assert!(cred.supports_model("claude-sonnet"));
        assert!(cred.supports_model("claude-opus"));
    }
}
