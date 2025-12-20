//! Qwen (通义千问) OAuth Provider
//!
//! 实现 Qwen OAuth 认证流程，与 CLIProxyAPI 对齐。
//! 支持 Token 刷新、重试机制和统一凭证格式。

use super::error::{
    create_auth_error, create_config_error, create_token_refresh_error, ProviderError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

// Constants - 与 CLIProxyAPI 对齐
const QWEN_DIR: &str = ".qwen";
const CREDENTIALS_FILE: &str = "oauth_creds.json";
const QWEN_BASE_URL: &str = "https://portal.qwen.ai/v1";

// OAuth 端点和凭证 - 与 CLIProxyAPI 完全一致
const QWEN_TOKEN_URL: &str = "https://chat.qwen.ai/api/v1/oauth2/token";
const QWEN_CLIENT_ID: &str = "f0304373b74a44d2b584a3fb70ca9e56";

pub const QWEN_MODELS: &[&str] = &["qwen3-coder-plus", "qwen3-coder-flash"];

/// Qwen OAuth 凭证存储
///
/// 与 CLIProxyAPI 的 QwenTokenStorage 格式兼容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenCredentials {
    /// 访问令牌
    pub access_token: Option<String>,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// 令牌类型
    pub token_type: Option<String>,
    /// 资源 URL
    pub resource_url: Option<String>,
    /// 过期时间戳（毫秒）- 兼容旧格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<i64>,
    /// 过期时间（RFC3339 格式）- 新格式，与 CLIProxyAPI 一致
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire: Option<String>,
    /// 最后刷新时间（RFC3339 格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<String>,
    /// 凭证类型标识
    #[serde(default = "default_qwen_type", rename = "type")]
    pub cred_type: String,
}

fn default_qwen_type() -> String {
    "qwen".to_string()
}

impl Default for QwenCredentials {
    fn default() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
            token_type: Some("Bearer".to_string()),
            resource_url: None,
            expiry_date: None,
            expire: None,
            last_refresh: None,
            cred_type: default_qwen_type(),
        }
    }
}

pub struct QwenProvider {
    pub credentials: QwenCredentials,
    pub client: Client,
}

impl Default for QwenProvider {
    fn default() -> Self {
        Self {
            credentials: QwenCredentials::default(),
            client: Client::new(),
        }
    }
}

impl QwenProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_creds_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(QWEN_DIR)
            .join(CREDENTIALS_FILE)
    }

    pub async fn load_credentials(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = Self::default_creds_path();

        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let content = tokio::fs::read_to_string(&path).await?;
            let creds: QwenCredentials = serde_json::from_str(&content)?;
            self.credentials = creds;
        }

        Ok(())
    }

    pub async fn load_credentials_from_path(
        &mut self,
        path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let content = tokio::fs::read_to_string(path).await?;
        let creds: QwenCredentials = serde_json::from_str(&content)?;
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
                // Token 有效期需要超过 30 秒
                return expires > now + chrono::Duration::seconds(30);
            }
        }

        // 兼容旧的毫秒时间戳格式
        if let Some(expiry) = self.credentials.expiry_date {
            let now = chrono::Utc::now().timestamp_millis();
            return expiry > now + 30_000;
        }

        true
    }

    pub fn get_base_url(&self) -> String {
        self.credentials
            .resource_url
            .as_ref()
            .map(|url| {
                let normalized = if url.starts_with("http") {
                    url.clone()
                } else {
                    format!("https://{url}")
                };
                if normalized.ends_with("/v1") {
                    normalized
                } else {
                    format!("{normalized}/v1")
                }
            })
            .unwrap_or_else(|| QWEN_BASE_URL.to_string())
    }

    /// 刷新 Token - 与 CLIProxyAPI 对齐，使用 form-urlencoded 格式
    pub async fn refresh_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let refresh_token = self
            .credentials
            .refresh_token
            .as_ref()
            .ok_or_else(|| create_config_error("没有可用的 refresh_token"))?;

        let client_id =
            std::env::var("QWEN_OAUTH_CLIENT_ID").unwrap_or_else(|_| QWEN_CLIENT_ID.to_string());

        tracing::info!("[QWEN] 正在刷新 Token");

        // 与 CLIProxyAPI 对齐：使用 application/x-www-form-urlencoded 格式
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", client_id.as_str()),
        ];

        let resp = self
            .client
            .post(QWEN_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("[QWEN] Token 刷新失败: {} - {}", status, body);
            return Err(create_token_refresh_error(status, &body, "QWEN"));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        let new_token = data["access_token"]
            .as_str()
            .ok_or_else(|| create_auth_error("响应中没有 access_token"))?;

        self.credentials.access_token = Some(new_token.to_string());

        if let Some(rt) = data["refresh_token"].as_str() {
            self.credentials.refresh_token = Some(rt.to_string());
        }

        if let Some(resource_url) = data["resource_url"].as_str() {
            self.credentials.resource_url = Some(resource_url.to_string());
        }

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

        tracing::info!("[QWEN] Token 刷新成功");
        Ok(new_token.to_string())
    }

    /// 带重试机制的 Token 刷新
    pub async fn refresh_token_with_retry(
        &mut self,
        max_retries: u32,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut last_error = None;

        for attempt in 0..max_retries {
            if attempt > 0 {
                let delay = std::time::Duration::from_secs(1 << attempt);
                tracing::info!("[QWEN] 第 {} 次重试，等待 {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
            }

            match self.refresh_token().await {
                Ok(token) => return Ok(token),
                Err(e) => {
                    tracing::warn!("[QWEN] Token 刷新第 {} 次尝试失败: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        tracing::error!("[QWEN] Token 刷新在 {} 次尝试后失败", max_retries);
        Err(last_error.unwrap_or_else(|| create_auth_error("Token 刷新失败，请重新登录")))
    }

    /// 确保 Token 有效，必要时自动刷新
    pub async fn ensure_valid_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        if !self.is_token_valid() {
            tracing::info!("[QWEN] Token 需要刷新");
            self.refresh_token_with_retry(3).await
        } else {
            self.credentials
                .access_token
                .clone()
                .ok_or_else(|| create_config_error("没有可用的 access_token"))
        }
    }

    pub async fn chat_completions(
        &self,
        request: &serde_json::Value,
    ) -> Result<reqwest::Response, Box<dyn Error + Send + Sync>> {
        let token = self
            .credentials
            .access_token
            .as_ref()
            .ok_or_else(|| create_config_error("没有可用的 access_token"))?;

        let base_url = self.get_base_url();
        let url = format!("{base_url}/chat/completions");

        // Ensure model is valid
        let mut req_body = request.clone();
        if let Some(model) = req_body.get("model").and_then(|m| m.as_str()) {
            if !QWEN_MODELS.contains(&model) {
                req_body["model"] = serde_json::json!(QWEN_MODELS[0]);
            }
        }

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("X-DashScope-AuthType", "qwen-oauth")
            .json(&req_body)
            .send()
            .await?;

        Ok(resp)
    }
}

// ============================================================================
// Device Code Flow 登录功能（与 CLIProxyAPI 对齐）
// ============================================================================

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

// Device Code Flow 端点
const QWEN_DEVICE_CODE_URL: &str = "https://chat.qwen.ai/api/v1/oauth2/device/code";
const QWEN_OAUTH_SCOPE: &str = "openid profile email model.completion";
const QWEN_DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// Device Code Flow 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    /// 设备码（用于轮询）
    #[serde(alias = "deviceCode")]
    pub device_code: String,
    /// 用户码（用户在浏览器中输入）
    #[serde(alias = "userCode")]
    pub user_code: String,
    /// 验证 URL
    #[serde(alias = "verificationUri")]
    pub verification_uri: String,
    /// 完整验证 URL（包含 user_code）
    #[serde(default, alias = "verificationUriComplete")]
    pub verification_uri_complete: Option<String>,
    /// 过期时间（秒）
    #[serde(alias = "expiresIn")]
    pub expires_in: i64,
    /// 轮询间隔（秒），默认 5 秒
    #[serde(default = "default_interval")]
    pub interval: i64,
}

fn default_interval() -> i64 {
    5
}

/// Qwen OAuth 登录结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenOAuthResult {
    pub credentials: QwenCredentials,
    pub creds_file_path: String,
}

/// PKCE 代码生成
fn generate_pkce_pair() -> Result<(String, String), Box<dyn Error + Send + Sync>> {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    Ok((code_verifier, code_challenge))
}

/// 发起 Device Code Flow
pub async fn initiate_device_flow(
    client: &Client,
) -> Result<(DeviceCodeResponse, String), Box<dyn Error + Send + Sync>> {
    let (code_verifier, code_challenge) = generate_pkce_pair()?;

    let params = [
        ("client_id", QWEN_CLIENT_ID),
        ("scope", QWEN_OAUTH_SCOPE),
        ("code_challenge", code_challenge.as_str()),
        ("code_challenge_method", "S256"),
    ];

    let resp = client
        .post(QWEN_DEVICE_CODE_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Device code 请求失败: {} - {}", status, body).into());
    }

    // 先获取响应体文本，以便在解析失败时提供详细错误信息
    let body = resp.text().await?;
    tracing::debug!("[QWEN] Device Code 响应: {}", body);

    let device_response: DeviceCodeResponse = serde_json::from_str(&body)
        .map_err(|e| format!("解析 Device Code 响应失败: {} - 响应内容: {}", e, body))?;

    if device_response.device_code.is_empty() {
        return Err("Device code 响应中没有 device_code".into());
    }

    tracing::info!(
        "[QWEN] Device Code Flow 已启动，user_code: {}, verification_uri: {}",
        device_response.user_code,
        device_response.verification_uri
    );

    Ok((device_response, code_verifier))
}

/// 轮询 Token 端点
pub async fn poll_for_token(
    client: &Client,
    device_code: &str,
    code_verifier: &str,
    interval: u64,
    max_attempts: u32,
) -> Result<QwenCredentials, Box<dyn Error + Send + Sync>> {
    let poll_interval = std::time::Duration::from_secs(interval.max(5));

    for attempt in 0..max_attempts {
        if attempt > 0 {
            tokio::time::sleep(poll_interval).await;
        }

        tracing::debug!("[QWEN] 轮询 Token，第 {} 次尝试", attempt + 1);

        let params = [
            ("grant_type", QWEN_DEVICE_GRANT_TYPE),
            ("client_id", QWEN_CLIENT_ID),
            ("device_code", device_code),
            ("code_verifier", code_verifier),
        ];

        let resp = client
            .post(QWEN_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[QWEN] 轮询请求失败: {}", e);
                continue;
            }
        };

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        if status.is_success() {
            // 成功获取 Token
            let data: serde_json::Value = serde_json::from_str(&body)?;

            let access_token = data["access_token"]
                .as_str()
                .ok_or("响应中没有 access_token")?
                .to_string();
            let refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
            let token_type = data["token_type"].as_str().map(|s| s.to_string());
            let resource_url = data["resource_url"].as_str().map(|s| s.to_string());
            let expires_in = data["expires_in"].as_i64().unwrap_or(3600);

            let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

            let credentials = QwenCredentials {
                access_token: Some(access_token),
                refresh_token,
                token_type,
                resource_url,
                expiry_date: Some(expires_at.timestamp_millis()),
                expire: Some(expires_at.to_rfc3339()),
                last_refresh: Some(chrono::Utc::now().to_rfc3339()),
                cred_type: "qwen".to_string(),
            };

            tracing::info!("[QWEN] Token 获取成功");
            return Ok(credentials);
        }

        // 解析错误响应
        if let Ok(error_data) = serde_json::from_str::<serde_json::Value>(&body) {
            let error_type = error_data["error"].as_str().unwrap_or("");

            match error_type {
                "authorization_pending" => {
                    // 用户尚未完成授权，继续轮询
                    tracing::debug!("[QWEN] 等待用户授权...");
                    continue;
                }
                "slow_down" => {
                    // 轮询太频繁，增加间隔
                    tracing::debug!("[QWEN] 服务器要求降低轮询频率");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                "expired_token" => {
                    return Err("Device code 已过期，请重新开始授权流程".into());
                }
                "access_denied" => {
                    return Err("用户拒绝了授权请求".into());
                }
                _ => {
                    let error_desc = error_data["error_description"]
                        .as_str()
                        .unwrap_or("未知错误");
                    return Err(format!("Token 获取失败: {} - {}", error_type, error_desc).into());
                }
            }
        }

        // 其他错误
        if status.as_u16() != 400 {
            return Err(format!("Token 请求失败: {} - {}", status, body).into());
        }
    }

    Err("授权超时，请重新开始授权流程".into())
}

/// 启动 Qwen Device Code Flow 登录
pub async fn start_qwen_device_code_login() -> Result<QwenOAuthResult, Box<dyn Error + Send + Sync>>
{
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 发起 Device Code Flow
    let (device_response, code_verifier) = initiate_device_flow(&client).await?;

    // 打开浏览器
    let verification_url = device_response
        .verification_uri_complete
        .as_ref()
        .unwrap_or(&device_response.verification_uri);

    tracing::info!("[QWEN] 打开浏览器进行授权: {}", verification_url);

    if let Err(e) = open::that(verification_url) {
        tracing::warn!("[QWEN] 无法打开浏览器: {}. 请手动打开 URL.", e);
    }

    // 轮询 Token
    let credentials = poll_for_token(
        &client,
        &device_response.device_code,
        &code_verifier,
        device_response.interval as u64,
        60, // 最多轮询 60 次（约 5 分钟）
    )
    .await?;

    // 保存凭证到应用数据目录
    let creds_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("proxycast")
        .join("credentials")
        .join("qwen");

    std::fs::create_dir_all(&creds_dir)?;

    let uuid = Uuid::new_v4().to_string();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("qwen_{}_{}.json", &uuid[..8], timestamp);
    let creds_file_path = creds_dir.join(&filename);

    let creds_json = serde_json::to_string_pretty(&credentials)?;
    std::fs::write(&creds_file_path, &creds_json)?;

    tracing::info!("[QWEN] 凭证已保存到: {:?}", creds_file_path);

    Ok(QwenOAuthResult {
        credentials,
        creds_file_path: creds_file_path.to_string_lossy().to_string(),
    })
}

/// 启动 Qwen Device Code Flow 并返回设备码信息（不自动打开浏览器）
pub async fn start_qwen_device_code_and_get_info() -> Result<
    (
        DeviceCodeResponse,
        impl std::future::Future<Output = Result<QwenOAuthResult, Box<dyn Error + Send + Sync>>>,
    ),
    Box<dyn Error + Send + Sync>,
> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 发起 Device Code Flow
    let (device_response, code_verifier) = initiate_device_flow(&client).await?;

    let device_code = device_response.device_code.clone();
    let interval = device_response.interval as u64;

    // 创建等待 future
    let wait_future = async move {
        // 轮询 Token
        let credentials =
            poll_for_token(&client, &device_code, &code_verifier, interval, 60).await?;

        // 保存凭证到应用数据目录
        let creds_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("proxycast")
            .join("credentials")
            .join("qwen");

        std::fs::create_dir_all(&creds_dir)?;

        let uuid = Uuid::new_v4().to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("qwen_{}_{}.json", &uuid[..8], timestamp);
        let creds_file_path = creds_dir.join(&filename);

        let creds_json = serde_json::to_string_pretty(&credentials)?;
        std::fs::write(&creds_file_path, &creds_json)?;

        tracing::info!("[QWEN] 凭证已保存到: {:?}", creds_file_path);

        Ok(QwenOAuthResult {
            credentials,
            creds_file_path: creds_file_path.to_string_lossy().to_string(),
        })
    };

    Ok((device_response, wait_future))
}
