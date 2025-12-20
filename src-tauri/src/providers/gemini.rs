//! Gemini CLI OAuth Provider
//!
//! 实现 Google Gemini OAuth 认证流程，与 CLIProxyAPI 对齐。
//! 支持 Token 刷新、重试机制和统一凭证格式。

use super::error::{
    create_auth_error, create_config_error, create_token_refresh_error, ProviderError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

// Constants - 与 CLIProxyAPI 对齐
const CODE_ASSIST_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com";
const CODE_ASSIST_API_VERSION: &str = "v1internal";
const CREDENTIALS_DIR: &str = ".gemini";
const CREDENTIALS_FILE: &str = "oauth_creds.json";

// OAuth 端点
const GEMINI_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

// OAuth 凭证从环境变量读取
fn get_oauth_client_id() -> String {
    std::env::var("GEMINI_OAUTH_CLIENT_ID").unwrap_or_default()
}

fn get_oauth_client_secret() -> String {
    std::env::var("GEMINI_OAUTH_CLIENT_SECRET").unwrap_or_default()
}

#[allow(dead_code)]
pub const GEMINI_MODELS: &[&str] = &[
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
    "gemini-2.5-pro",
    "gemini-2.5-pro-preview-06-05",
    "gemini-2.5-flash-preview-09-2025",
    "gemini-3-pro-preview",
];

/// Gemini OAuth 凭证存储
///
/// 与 CLIProxyAPI 的 GeminiTokenStorage 格式兼容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiCredentials {
    /// 访问令牌
    pub access_token: Option<String>,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// 令牌类型
    pub token_type: Option<String>,
    /// 过期时间戳（毫秒）- 兼容旧格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<i64>,
    /// 过期时间（RFC3339 格式）- 新格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire: Option<String>,
    /// OAuth 作用域
    pub scope: Option<String>,
    /// 用户邮箱
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// 最后刷新时间（RFC3339 格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<String>,
    /// 凭证类型标识
    #[serde(default = "default_gemini_type", rename = "type")]
    pub cred_type: String,
    /// 嵌套的 token 对象（兼容 CLIProxyAPI 格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<GeminiTokenInfo>,
}

fn default_gemini_type() -> String {
    "gemini".to_string()
}

/// 嵌套的 Token 信息（兼容 CLIProxyAPI 格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiTokenInfo {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scopes: Option<Vec<String>>,
}

impl Default for GeminiCredentials {
    fn default() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
            token_type: Some("Bearer".to_string()),
            expiry_date: None,
            expire: None,
            scope: None,
            email: None,
            last_refresh: None,
            cred_type: default_gemini_type(),
            token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub model: String,
    pub project: String,
    pub request: GeminiRequestBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequestBody {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata")]
    pub usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    pub content: Option<GeminiContent>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUsageMetadata {
    pub prompt_token_count: Option<i32>,
    pub candidates_token_count: Option<i32>,
    pub total_token_count: Option<i32>,
}

pub struct GeminiProvider {
    pub credentials: GeminiCredentials,
    pub project_id: Option<String>,
    pub client: Client,
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self {
            credentials: GeminiCredentials::default(),
            project_id: None,
            client: Client::new(),
        }
    }
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_creds_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CREDENTIALS_DIR)
            .join(CREDENTIALS_FILE)
    }

    pub async fn load_credentials(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = Self::default_creds_path();

        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let content = tokio::fs::read_to_string(&path).await?;
            let creds: GeminiCredentials = serde_json::from_str(&content)?;
            self.credentials = creds;
        }

        Ok(())
    }

    pub async fn load_credentials_from_path(
        &mut self,
        path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let content = tokio::fs::read_to_string(path).await?;
        let creds: GeminiCredentials = serde_json::from_str(&content)?;
        self.credentials = creds;
        Ok(())
    }

    pub async fn save_credentials(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = Self::default_creds_path();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = serde_json::to_string_pretty(&self.credentials)?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    /// 检查 Token 是否有效
    pub fn is_token_valid(&self) -> bool {
        if self.credentials.access_token.is_none() {
            return false;
        }

        // 优先检查 RFC3339 格式的过期时间
        if let Some(expire_str) = &self.credentials.expire {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expire_str) {
                let now = chrono::Utc::now();
                // Token 有效期需要超过 5 分钟
                return expires > now + chrono::Duration::minutes(5);
            }
        }

        // 兼容旧的毫秒时间戳格式
        if let Some(expiry) = self.credentials.expiry_date {
            let now = chrono::Utc::now().timestamp_millis();
            return expiry > now + 300_000;
        }

        true
    }

    /// 刷新 Token
    pub async fn refresh_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let refresh_token = self
            .credentials
            .refresh_token
            .as_ref()
            .ok_or_else(|| create_config_error("没有可用的 refresh_token"))?;

        let client_id = get_oauth_client_id();
        let client_secret = get_oauth_client_secret();

        tracing::info!("[GEMINI] 正在刷新 Token");

        let params = [
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];

        let resp = self
            .client
            .post(GEMINI_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("[GEMINI] Token 刷新失败: {} - {}", status, body);
            return Err(create_token_refresh_error(status, &body, "GEMINI"));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        let new_token = data["access_token"]
            .as_str()
            .ok_or_else(|| create_auth_error("响应中没有 access_token"))?;

        self.credentials.access_token = Some(new_token.to_string());

        // 更新过期时间（同时保存两种格式以兼容）
        if let Some(expires_in) = data["expires_in"].as_i64() {
            let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
            self.credentials.expire = Some(expires_at.to_rfc3339());
            self.credentials.expiry_date = Some(expires_at.timestamp_millis());
        }

        // 更新最后刷新时间
        self.credentials.last_refresh = Some(chrono::Utc::now().to_rfc3339());

        // 保存刷新后的凭证
        self.save_credentials().await?;

        tracing::info!("[GEMINI] Token 刷新成功");
        Ok(new_token.to_string())
    }

    /// 带重试机制的 Token 刷新
    ///
    /// 最多重试 `max_retries` 次，使用指数退避策略
    pub async fn refresh_token_with_retry(
        &mut self,
        max_retries: u32,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut last_error = None;

        for attempt in 0..max_retries {
            if attempt > 0 {
                // 指数退避: 1s, 2s, 4s, ...
                let delay = std::time::Duration::from_secs(1 << attempt);
                tracing::info!("[GEMINI] 第 {} 次重试，等待 {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
            }

            match self.refresh_token().await {
                Ok(token) => return Ok(token),
                Err(e) => {
                    tracing::warn!("[GEMINI] Token 刷新第 {} 次尝试失败: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        tracing::error!("[GEMINI] Token 刷新在 {} 次尝试后失败", max_retries);
        Err(last_error.unwrap_or_else(|| create_auth_error("Token 刷新失败，请重新登录")))
    }

    /// 确保 Token 有效，必要时自动刷新
    pub async fn ensure_valid_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        if !self.is_token_valid() {
            tracing::info!("[GEMINI] Token 需要刷新");
            self.refresh_token_with_retry(3).await
        } else {
            self.credentials
                .access_token
                .clone()
                .ok_or_else(|| "没有可用的 access_token".into())
        }
    }

    pub fn get_api_url(&self, action: &str) -> String {
        format!("{CODE_ASSIST_ENDPOINT}/{CODE_ASSIST_API_VERSION}:{action}")
    }

    pub async fn call_api(
        &self,
        action: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let token = self
            .credentials
            .access_token
            .as_ref()
            .ok_or("No access token")?;

        let url = self.get_api_url(action);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("API call failed: {status} - {body}").into());
        }

        let data: serde_json::Value = resp.json().await?;
        Ok(data)
    }

    pub async fn discover_project(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        if let Some(ref project_id) = self.project_id {
            return Ok(project_id.clone());
        }

        let body = serde_json::json!({
            "cloudaicompanionProject": "",
            "metadata": {
                "ideType": "IDE_UNSPECIFIED",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI",
                "duetProject": ""
            }
        });

        let resp = self.call_api("loadCodeAssist", &body).await?;

        if let Some(project) = resp["cloudaicompanionProject"].as_str() {
            if !project.is_empty() {
                self.project_id = Some(project.to_string());
                return Ok(project.to_string());
            }
        }

        // Need to onboard
        let onboard_body = serde_json::json!({
            "tierId": "free-tier",
            "cloudaicompanionProject": "",
            "metadata": {
                "ideType": "IDE_UNSPECIFIED",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI",
                "duetProject": ""
            }
        });

        let mut lro_resp = self.call_api("onboardUser", &onboard_body).await?;

        // Poll until done
        for _ in 0..30 {
            if lro_resp["done"].as_bool().unwrap_or(false) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            lro_resp = self.call_api("onboardUser", &onboard_body).await?;
        }

        let project_id = lro_resp["response"]["cloudaicompanionProject"]["id"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if project_id.is_empty() {
            return Err("Failed to discover project ID".into());
        }

        self.project_id = Some(project_id.clone());
        Ok(project_id)
    }
}

// ============ Gemini API Key Provider ============

/// Default Gemini API base URL
pub const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com";

/// Gemini API Key Provider for multi-account load balancing
///
/// This provider supports:
/// - Multiple API keys with round-robin load balancing
/// - Per-key custom base URLs
/// - Model exclusion filtering (to be implemented in task 11.2)
#[derive(Debug, Clone)]
pub struct GeminiApiKeyCredential {
    /// Credential ID
    pub id: String,
    /// API Key
    pub api_key: String,
    /// Custom base URL (optional)
    pub base_url: Option<String>,
    /// Excluded models (supports wildcards)
    pub excluded_models: Vec<String>,
    /// Per-key proxy URL (optional)
    pub proxy_url: Option<String>,
    /// Whether this credential is disabled
    pub disabled: bool,
}

impl GeminiApiKeyCredential {
    /// Create a new Gemini API Key credential
    pub fn new(id: String, api_key: String) -> Self {
        Self {
            id,
            api_key,
            base_url: None,
            excluded_models: Vec::new(),
            proxy_url: None,
            disabled: false,
        }
    }

    /// Set custom base URL
    pub fn with_base_url(mut self, base_url: Option<String>) -> Self {
        self.base_url = base_url;
        self
    }

    /// Set excluded models
    pub fn with_excluded_models(mut self, excluded_models: Vec<String>) -> Self {
        self.excluded_models = excluded_models;
        self
    }

    /// Set proxy URL
    pub fn with_proxy_url(mut self, proxy_url: Option<String>) -> Self {
        self.proxy_url = proxy_url;
        self
    }

    /// Set disabled state
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Get the effective base URL (custom or default)
    pub fn get_base_url(&self) -> &str {
        self.base_url.as_deref().unwrap_or(GEMINI_API_BASE_URL)
    }

    /// Check if this credential is available (not disabled)
    pub fn is_available(&self) -> bool {
        !self.disabled
    }

    /// Check if this credential supports the given model
    /// Returns false if the model matches any exclusion pattern
    pub fn supports_model(&self, model: &str) -> bool {
        !self.excluded_models.iter().any(|pattern| {
            if pattern.contains('*') {
                // Simple wildcard matching
                let pattern = pattern.replace('*', ".*");
                regex::Regex::new(&format!("^{}$", pattern))
                    .map(|re| re.is_match(model))
                    .unwrap_or(false)
            } else {
                pattern == model
            }
        })
    }

    /// Build the API URL for a given model and action
    pub fn build_api_url(&self, model: &str, action: &str) -> String {
        format!("{}/v1beta/models/{}:{}", self.get_base_url(), model, action)
    }
}

/// Gemini API Key Provider
///
/// Manages multiple Gemini API keys with load balancing support.
/// Integrates with the credential pool system for round-robin selection.
pub struct GeminiApiKeyProvider {
    /// HTTP client
    pub client: Client,
}

impl Default for GeminiApiKeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiApiKeyProvider {
    /// Create a new Gemini API Key provider
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Create a provider with a custom HTTP client
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Make a generateContent request using the given credential
    pub async fn generate_content(
        &self,
        credential: &GeminiApiKeyCredential,
        model: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let url = credential.build_api_url(model, "generateContent");

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &credential.api_key)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini API call failed: {status} - {body}").into());
        }

        let data: serde_json::Value = resp.json().await?;
        Ok(data)
    }

    /// Make a streamGenerateContent request using the given credential
    pub async fn stream_generate_content(
        &self,
        credential: &GeminiApiKeyCredential,
        model: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "{}?alt=sse",
            credential.build_api_url(model, "streamGenerateContent")
        );

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &credential.api_key)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini API stream call failed: {status} - {body}").into());
        }

        Ok(resp)
    }

    /// List available models using the given credential
    pub async fn list_models(
        &self,
        credential: &GeminiApiKeyCredential,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let url = format!("{}/v1beta/models", credential.get_base_url());

        let resp = self
            .client
            .get(&url)
            .header("x-goog-api-key", &credential.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini API list models failed: {status} - {body}").into());
        }

        let data: serde_json::Value = resp.json().await?;
        Ok(data)
    }
}

#[cfg(test)]
mod gemini_api_key_tests {
    use super::*;

    #[test]
    fn test_gemini_api_key_credential_new() {
        let cred = GeminiApiKeyCredential::new("test-id".to_string(), "test-key".to_string());
        assert_eq!(cred.id, "test-id");
        assert_eq!(cred.api_key, "test-key");
        assert!(cred.base_url.is_none());
        assert!(cred.excluded_models.is_empty());
        assert!(cred.proxy_url.is_none());
        assert!(!cred.disabled);
    }

    #[test]
    fn test_gemini_api_key_credential_with_base_url() {
        let cred = GeminiApiKeyCredential::new("test-id".to_string(), "test-key".to_string())
            .with_base_url(Some("https://custom.api.com".to_string()));
        assert_eq!(cred.get_base_url(), "https://custom.api.com");
    }

    #[test]
    fn test_gemini_api_key_credential_default_base_url() {
        let cred = GeminiApiKeyCredential::new("test-id".to_string(), "test-key".to_string());
        assert_eq!(cred.get_base_url(), GEMINI_API_BASE_URL);
    }

    #[test]
    fn test_gemini_api_key_credential_is_available() {
        let cred = GeminiApiKeyCredential::new("test-id".to_string(), "test-key".to_string());
        assert!(cred.is_available());

        let disabled_cred = cred.with_disabled(true);
        assert!(!disabled_cred.is_available());
    }

    #[test]
    fn test_gemini_api_key_credential_supports_model() {
        let cred = GeminiApiKeyCredential::new("test-id".to_string(), "test-key".to_string())
            .with_excluded_models(vec![
                "gemini-2.5-pro".to_string(),
                "gemini-*-preview".to_string(),
            ]);

        // Exact match exclusion
        assert!(!cred.supports_model("gemini-2.5-pro"));

        // Wildcard exclusion
        assert!(!cred.supports_model("gemini-3-preview"));
        assert!(!cred.supports_model("gemini-2.5-preview"));

        // Not excluded
        assert!(cred.supports_model("gemini-2.5-flash"));
        assert!(cred.supports_model("gemini-2.0-flash"));
    }

    #[test]
    fn test_gemini_api_key_credential_build_api_url() {
        let cred = GeminiApiKeyCredential::new("test-id".to_string(), "test-key".to_string());
        let url = cred.build_api_url("gemini-2.5-flash", "generateContent");
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent"
        );

        let custom_cred = cred.with_base_url(Some("https://custom.api.com".to_string()));
        let custom_url = custom_cred.build_api_url("gemini-2.5-flash", "generateContent");
        assert_eq!(
            custom_url,
            "https://custom.api.com/v1beta/models/gemini-2.5-flash:generateContent"
        );
    }

    #[test]
    fn test_gemini_api_key_provider_new() {
        let provider = GeminiApiKeyProvider::new();
        // Just verify it can be created
        assert!(true);
        let _ = provider;
    }
}

// ============================================================================
// Gemini OAuth 登录功能
// ============================================================================

use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

// Gemini CLI OAuth 配置 - 与 claude-relay-service 对齐
pub const GEMINI_OAUTH_CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
pub const GEMINI_OAUTH_CLIENT_SECRET: &str = "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";
pub const GEMINI_OAUTH_SCOPES: &[&str] = &["https://www.googleapis.com/auth/cloud-platform"];
pub const GEMINI_OAUTH_REDIRECT_URI: &str = "https://codeassist.google.com/authcode";

/// OAuth 登录成功后的凭证信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiOAuthResult {
    pub credentials: GeminiCredentials,
    pub creds_file_path: String,
}

/// 生成 PKCE code_verifier 和 code_challenge
fn generate_pkce() -> (String, String) {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};

    // 生成 43-128 字符的随机字符串作为 code_verifier
    let code_verifier: String = (0..64)
        .map(|_| {
            let idx = rand::random::<u8>() % 66;
            let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
            chars[idx as usize] as char
        })
        .collect();

    // 计算 code_challenge = BASE64URL(SHA256(code_verifier))
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    (code_verifier, code_challenge)
}

/// 生成 OAuth 授权 URL（使用 PKCE）
pub fn generate_gemini_auth_url(state: &str, code_challenge: &str) -> String {
    let scopes = GEMINI_OAUTH_SCOPES.join(" ");

    let params = [
        ("access_type", "offline"),
        ("client_id", GEMINI_OAUTH_CLIENT_ID),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("prompt", "select_account"),
        ("redirect_uri", GEMINI_OAUTH_REDIRECT_URI),
        ("response_type", "code"),
        ("scope", &scopes),
        ("state", state),
    ];

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("https://accounts.google.com/o/oauth2/v2/auth?{}", query)
}

/// Gemini OAuth 会话信息（用于存储 PKCE code_verifier）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiOAuthSession {
    pub session_id: String,
    pub code_verifier: String,
    pub state: String,
    pub created_at: i64,
}

/// 生成 Gemini OAuth 授权 URL 和会话信息
///
/// 返回 (auth_url, session) 元组
/// - auth_url: 用户需要在浏览器中打开的授权 URL
/// - session: 包含 code_verifier 的会话信息，用于后续交换 token
pub fn generate_gemini_auth_url_with_session() -> (String, GeminiOAuthSession) {
    let (code_verifier, code_challenge) = generate_pkce();
    let state = Uuid::new_v4().to_string();
    let session_id = Uuid::new_v4().to_string();

    let auth_url = generate_gemini_auth_url(&state, &code_challenge);

    let session = GeminiOAuthSession {
        session_id,
        code_verifier,
        state,
        created_at: chrono::Utc::now().timestamp(),
    };

    (auth_url, session)
}

/// 用授权码交换 Token 并创建凭证
///
/// 完整流程：
/// 1. 用 code + code_verifier 交换 tokens
/// 2. 获取用户邮箱
/// 3. 获取项目 ID
/// 4. 保存凭证到文件
pub async fn exchange_gemini_code_and_create_credentials(
    code: &str,
    code_verifier: &str,
) -> Result<GeminiOAuthResult, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    tracing::info!("[Gemini OAuth] 正在用授权码交换 Token...");

    // 交换 Token
    let token_data = exchange_gemini_code_for_token(&client, code, code_verifier).await?;

    let access_token = token_data["access_token"]
        .as_str()
        .ok_or("响应中没有 access_token")?
        .to_string();
    let refresh_token = token_data["refresh_token"].as_str().map(|s| s.to_string());
    let expires_in = token_data["expires_in"].as_i64();

    tracing::info!("[Gemini OAuth] Token 交换成功");

    // 获取用户邮箱
    let email = fetch_gemini_user_email(&client, &access_token)
        .await
        .ok()
        .flatten();

    tracing::info!("[Gemini OAuth] 用户邮箱: {:?}", email);

    // 获取项目 ID
    let _project_id = fetch_gemini_project_id(&client, &access_token)
        .await
        .ok()
        .flatten();

    // 构建凭证
    let now = chrono::Utc::now();
    let expires_at = expires_in.map(|secs| now + chrono::Duration::seconds(secs));

    let credentials = GeminiCredentials {
        access_token: Some(access_token),
        refresh_token,
        token_type: Some("Bearer".to_string()),
        expiry_date: expires_at.map(|t| t.timestamp_millis()),
        expire: expires_at.map(|t| t.to_rfc3339()),
        scope: Some(GEMINI_OAUTH_SCOPES.join(" ")),
        email,
        last_refresh: Some(now.to_rfc3339()),
        cred_type: "gemini".to_string(),
        token: None,
    };

    // 保存凭证到文件
    let file_path = save_gemini_credentials_to_file(&credentials).await?;

    tracing::info!("[Gemini OAuth] 凭证已保存到: {}", file_path);

    Ok(GeminiOAuthResult {
        credentials,
        creds_file_path: file_path,
    })
}

/// 用授权码交换 Token（使用 PKCE）
pub async fn exchange_gemini_code_for_token(
    client: &Client,
    code: &str,
    code_verifier: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let params = [
        ("code", code),
        ("client_id", GEMINI_OAUTH_CLIENT_ID),
        ("client_secret", GEMINI_OAUTH_CLIENT_SECRET),
        ("code_verifier", code_verifier),
        ("redirect_uri", GEMINI_OAUTH_REDIRECT_URI),
        ("grant_type", "authorization_code"),
    ];

    let resp = client.post(GEMINI_TOKEN_URL).form(&params).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token 交换失败: {} - {}", status, body).into());
    }

    let data: serde_json::Value = resp.json().await?;
    Ok(data)
}

/// 获取用户邮箱
pub async fn fetch_gemini_user_email(
    client: &Client,
    access_token: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let resp = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if resp.status().is_success() {
        let data: serde_json::Value = resp.json().await?;
        Ok(data["email"].as_str().map(|s| s.to_string()))
    } else {
        Ok(None)
    }
}

/// 获取项目 ID（通过 loadCodeAssist 接口）
pub async fn fetch_gemini_project_id(
    client: &Client,
    access_token: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("[Gemini OAuth] 正在获取 projectId...");

    let resp = client
        .post(format!(
            "{}/{CODE_ASSIST_API_VERSION}:loadCodeAssist",
            CODE_ASSIST_ENDPOINT
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "cloudaicompanionProject": "",
            "metadata": {
                "ideType": "IDE_UNSPECIFIED",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI",
                "duetProject": ""
            }
        }))
        .send()
        .await?;

    let status = resp.status();
    tracing::info!("[Gemini OAuth] loadCodeAssist 响应状态: {}", status);

    if status.is_success() {
        let data: serde_json::Value = resp.json().await?;
        if let Some(project) = data["cloudaicompanionProject"].as_str() {
            if !project.is_empty() {
                tracing::info!("[Gemini OAuth] 获取到 projectId: {}", project);
                return Ok(Some(project.to_string()));
            }
        }
        tracing::info!("[Gemini OAuth] cloudaicompanionProject 为空");
        Ok(None)
    } else {
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(
            "[Gemini OAuth] loadCodeAssist 请求失败: {} - {}",
            status,
            body
        );
        Ok(None)
    }
}

/// OAuth 成功页面 HTML
const GEMINI_OAUTH_SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权成功</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: linear-gradient(135deg, #4285f4 0%, #34a853 100%); }
        .container { text-align: center; background: white; padding: 40px 60px; border-radius: 16px; box-shadow: 0 10px 40px rgba(0,0,0,0.2); }
        h1 { color: #22c55e; margin-bottom: 16px; }
        p { color: #666; margin-bottom: 8px; }
        .email { color: #333; font-weight: 500; }
    </style>
</head>
<body>
    <div class="container">
        <h1>✓ 授权成功</h1>
        <p>Gemini 账号已添加到 ProxyCast</p>
        <p class="email">EMAIL_PLACEHOLDER</p>
        <p style="margin-top: 20px; color: #999;">可以关闭此页面</p>
    </div>
</body>
</html>"#;

/// OAuth 失败页面 HTML
const GEMINI_OAUTH_ERROR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权失败</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: linear-gradient(135deg, #4285f4 0%, #34a853 100%); }
        .container { text-align: center; background: white; padding: 40px 60px; border-radius: 16px; box-shadow: 0 10px 40px rgba(0,0,0,0.2); }
        h1 { color: #ef4444; margin-bottom: 16px; }
        p { color: #666; }
        .error { color: #ef4444; font-size: 14px; margin-top: 16px; }
    </style>
</head>
<body>
    <div class="container">
        <h1>✗ 授权失败</h1>
        <p>ERROR_PLACEHOLDER</p>
        <p style="margin-top: 20px; color: #999;">请关闭此页面后重试</p>
    </div>
</body>
</html>"#;

/// 保存 Gemini 凭证到文件
async fn save_gemini_credentials_to_file(
    credentials: &GeminiCredentials,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // 生成唯一文件名
    let uuid = Uuid::new_v4().to_string();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let filename = format!("gemini_{}_{}_gemini.json", &uuid[..8], timestamp);

    // 获取凭证存储目录
    let credentials_dir = dirs::data_dir()
        .ok_or_else(|| "无法获取应用数据目录")?
        .join("proxycast")
        .join("credentials");

    // 确保目录存在
    tokio::fs::create_dir_all(&credentials_dir).await?;

    let file_path = credentials_dir.join(&filename);

    // 写入凭证
    let content = serde_json::to_string_pretty(credentials)?;
    tokio::fs::write(&file_path, content).await?;

    Ok(file_path.to_string_lossy().to_string())
}

/// 启动 OAuth 服务器并返回授权 URL（不打开浏览器）
/// 服务器会在后台等待回调，成功后返回凭证
pub async fn start_gemini_oauth_server_and_get_url() -> Result<
    (
        String,
        impl std::future::Future<
            Output = Result<GeminiOAuthResult, Box<dyn std::error::Error + Send + Sync>>,
        >,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    use axum::{extract::Query, response::Html, routing::get, Router};
    use std::collections::HashMap;
    use tokio::net::TcpListener;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 生成 PKCE
    let (code_verifier, code_challenge) = generate_pkce();

    // 生成随机 state
    let state = Uuid::new_v4().to_string();
    let state_clone = state.clone();
    let code_verifier_clone = code_verifier.clone();

    // 创建 channel 用于接收回调结果
    let (tx, rx) = oneshot::channel::<Result<GeminiOAuthResult, String>>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    // 尝试绑定到多个端口
    let ports_to_try = [11451, 11452, 11453, 11454, 11455, 0];
    let mut listener = None;
    let mut bound_port = 0;

    for port in ports_to_try {
        match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
            Ok(l) => {
                bound_port = l.local_addr()?.port();
                listener = Some(l);
                tracing::info!("[Gemini OAuth] 成功绑定到端口 {}", bound_port);
                break;
            }
            Err(e) => {
                tracing::warn!("[Gemini OAuth] 端口 {} 绑定失败: {}", port, e);
                continue;
            }
        }
    }

    let listener = listener.ok_or("无法绑定到任何可用端口")?;

    // 生成授权 URL
    let auth_url = generate_gemini_auth_url(&state, &code_challenge);

    tracing::info!(
        "[Gemini OAuth] 服务器启动在端口 {}, 授权 URL: {}",
        bound_port,
        auth_url
    );

    // 构建路由
    let app = Router::new().route(
        "/oauth-callback",
        get(move |Query(params): Query<HashMap<String, String>>| {
            let tx = tx.clone();
            let client = client.clone();
            let state_expected = state_clone.clone();
            let code_verifier = code_verifier_clone.clone();

            async move {
                let code = params.get("code");
                let returned_state = params.get("state");
                let error = params.get("error");

                // 检查错误
                if let Some(err) = error {
                    let error_desc = params
                        .get("error_description")
                        .map(|s| s.as_str())
                        .unwrap_or("未知错误");
                    let error_msg = format!("{}: {}", err, error_desc);
                    tracing::error!("[Gemini OAuth] 授权失败: {}", error_msg);

                    if let Some(tx) = tx.lock().await.take() {
                        let _ = tx.send(Err(error_msg.clone()));
                    }

                    let html = GEMINI_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", &error_msg);
                    return Html(html);
                }

                // 验证 state
                if returned_state.map(|s| s.as_str()) != Some(&state_expected) {
                    let error_msg = "State 验证失败";
                    tracing::error!("[Gemini OAuth] {}", error_msg);

                    if let Some(tx) = tx.lock().await.take() {
                        let _ = tx.send(Err(error_msg.to_string()));
                    }

                    let html = GEMINI_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", error_msg);
                    return Html(html);
                }

                // 获取授权码
                let code = match code {
                    Some(c) => c,
                    None => {
                        let error_msg = "未收到授权码";
                        tracing::error!("[Gemini OAuth] {}", error_msg);

                        if let Some(tx) = tx.lock().await.take() {
                            let _ = tx.send(Err(error_msg.to_string()));
                        }

                        let html = GEMINI_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", error_msg);
                        return Html(html);
                    }
                };

                tracing::info!("[Gemini OAuth] 收到授权码，正在交换 Token...");

                // 交换 Token
                let token_result =
                    exchange_gemini_code_for_token(&client, code, &code_verifier).await;

                match token_result {
                    Ok(token_data) => {
                        let access_token = token_data["access_token"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let refresh_token =
                            token_data["refresh_token"].as_str().map(|s| s.to_string());
                        let expires_in = token_data["expires_in"].as_i64();

                        // 获取用户邮箱
                        let email = fetch_gemini_user_email(&client, &access_token)
                            .await
                            .ok()
                            .flatten();

                        // 获取项目 ID
                        let project_id = fetch_gemini_project_id(&client, &access_token)
                            .await
                            .ok()
                            .flatten();

                        // 构建凭证
                        let now = chrono::Utc::now();
                        let expires_at =
                            expires_in.map(|secs| now + chrono::Duration::seconds(secs));

                        let credentials = GeminiCredentials {
                            access_token: Some(access_token),
                            refresh_token,
                            token_type: Some("Bearer".to_string()),
                            expiry_date: expires_at.map(|t| t.timestamp_millis()),
                            expire: expires_at.map(|t| t.to_rfc3339()),
                            scope: Some(GEMINI_OAUTH_SCOPES.join(" ")),
                            email: email.clone(),
                            last_refresh: Some(now.to_rfc3339()),
                            cred_type: "gemini".to_string(),
                            token: None,
                        };

                        // 保存凭证到文件
                        match save_gemini_credentials_to_file(&credentials).await {
                            Ok(file_path) => {
                                tracing::info!("[Gemini OAuth] 凭证已保存到: {}", file_path);

                                let result = GeminiOAuthResult {
                                    credentials: credentials.clone(),
                                    creds_file_path: file_path,
                                };

                                if let Some(tx) = tx.lock().await.take() {
                                    let _ = tx.send(Ok(result));
                                }

                                let email_display = email.unwrap_or_else(|| "未知邮箱".to_string());
                                let project_display = project_id
                                    .map(|p| format!("<p>Project ID: {}</p>", p))
                                    .unwrap_or_default();
                                let html = GEMINI_OAUTH_SUCCESS_HTML
                                    .replace("EMAIL_PLACEHOLDER", &email_display)
                                    .replace(
                                        "</div>\n</body>",
                                        &format!("{}</div>\n</body>", project_display),
                                    );
                                Html(html)
                            }
                            Err(e) => {
                                let error_msg = format!("保存凭证失败: {}", e);
                                tracing::error!("[Gemini OAuth] {}", error_msg);

                                if let Some(tx) = tx.lock().await.take() {
                                    let _ = tx.send(Err(error_msg.clone()));
                                }

                                let html = GEMINI_OAUTH_ERROR_HTML
                                    .replace("ERROR_PLACEHOLDER", &error_msg);
                                Html(html)
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Token 交换失败: {}", e);
                        tracing::error!("[Gemini OAuth] {}", error_msg);

                        if let Some(tx) = tx.lock().await.take() {
                            let _ = tx.send(Err(error_msg.clone()));
                        }

                        let html = GEMINI_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", &error_msg);
                        Html(html)
                    }
                }
            }
        }),
    );

    // 启动服务器
    let server_future = async move {
        axum::serve(listener, app)
            .await
            .map_err(|e| format!("服务器错误: {}", e))
    };

    // 启动服务器任务
    tokio::spawn(server_future);

    // 返回授权 URL 和等待结果的 Future
    let wait_future = async move {
        match rx.await {
            Ok(result) => result.map_err(|e| e.into()),
            Err(_) => Err("OAuth 回调通道关闭".into()),
        }
    };

    Ok((auth_url, wait_future))
}

/// 启动 Gemini OAuth 登录流程（自动打开浏览器）
pub async fn start_gemini_oauth_login(
) -> Result<GeminiOAuthResult, Box<dyn std::error::Error + Send + Sync>> {
    let (auth_url, wait_future) = start_gemini_oauth_server_and_get_url().await?;

    // 打开浏览器
    tracing::info!("[Gemini OAuth] 正在打开浏览器...");
    if let Err(e) = open::that(&auth_url) {
        tracing::warn!("[Gemini OAuth] 无法自动打开浏览器: {}", e);
    }

    // 等待回调
    wait_future.await
}
