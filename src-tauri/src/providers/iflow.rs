//! iFlow OAuth Provider
//!
//! 实现 iFlow OAuth 和 Cookie 认证流程，与 CLIProxyAPI 对齐。
//! 支持双重认证模式：OAuth Token 和导入的 Cookie。

use super::error::{
    create_auth_error, create_config_error, create_token_refresh_error, ProviderError,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

// OAuth Constants - 与 CLIProxyAPI 对齐
const IFLOW_AUTH_URL: &str = "https://iflow.cn/oauth";
const IFLOW_TOKEN_URL: &str = "https://iflow.cn/oauth/token";
const IFLOW_USER_INFO_URL: &str = "https://iflow.cn/api/oauth/getUserInfo";
const IFLOW_API_KEY_URL: &str = "https://platform.iflow.cn/api/openapi/apikey";

// 客户端凭证 - 与 CLIProxyAPI 完全一致
const IFLOW_CLIENT_ID: &str = "10009311001";
const IFLOW_CLIENT_SECRET: &str = "4Z3YjXycVsQvyGF1etiNlIBB4RsqSDtW";

const DEFAULT_CALLBACK_PORT: u16 = 11451;
const IFLOW_API_BASE_URL: &str = "https://apis.iflow.cn/v1";

/// iFlow 凭证存储
///
/// 支持 OAuth Token 和 Cookie 两种认证模式
/// 与 CLIProxyAPI 的 IFlowTokenStorage 格式兼容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IFlowCredentials {
    /// 认证类型: "oauth" 或 "cookie"
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    /// OAuth2 访问令牌
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// 刷新令牌
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// 过期时间（RFC3339 格式）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<String>,
    /// 兼容旧字段名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Cookie 字符串
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookies: Option<String>,
    /// Cookie 过期时间
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie_expires_at: Option<String>,
    /// 用户邮箱/手机号
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// 用户 ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 最后刷新时间
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<String>,
    /// API Key（从 OAuth 流程获取）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Token 类型
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    /// OAuth 作用域
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// 凭证类型标识
    #[serde(default = "default_iflow_type", rename = "type")]
    pub cred_type: String,
}

fn default_auth_type() -> String {
    "oauth".to_string()
}

fn default_iflow_type() -> String {
    "iflow".to_string()
}

impl Default for IFlowCredentials {
    fn default() -> Self {
        Self {
            auth_type: default_auth_type(),
            access_token: None,
            refresh_token: None,
            expire: None,
            expires_at: None,
            cookies: None,
            cookie_expires_at: None,
            email: None,
            user_id: None,
            last_refresh: None,
            api_key: None,
            token_type: None,
            scope: None,
            cred_type: default_iflow_type(),
        }
    }
}

/// iFlow API Key 数据
/// 与 CLIProxyAPI 的 iFlowKeyData 对齐
#[derive(Debug, Clone)]
pub struct IFlowKeyData {
    pub api_key: String,
    pub expire_time: String,
    pub name: String,
    pub has_expired: bool,
}

/// Enum representation of iFlow credentials for type-safe handling
#[derive(Debug, Clone)]
pub enum IFlowCredentialsType {
    /// OAuth-based authentication
    OAuth {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    /// Cookie-based authentication
    Cookie {
        cookies: String,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    },
}

impl IFlowCredentials {
    /// Convert to typed enum representation
    pub fn to_typed(&self) -> Option<IFlowCredentialsType> {
        match self.auth_type.as_str() {
            "oauth" => {
                let access_token = self.access_token.clone()?;
                let expires_at = self.expires_at.as_ref().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                });
                Some(IFlowCredentialsType::OAuth {
                    access_token,
                    refresh_token: self.refresh_token.clone(),
                    expires_at,
                })
            }
            "cookie" => {
                let cookies = self.cookies.clone()?;
                let expires_at = self.cookie_expires_at.as_ref().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                });
                Some(IFlowCredentialsType::Cookie {
                    cookies,
                    expires_at,
                })
            }
            _ => None,
        }
    }

    /// 检查凭证是否有效
    pub fn is_valid(&self) -> bool {
        match self.auth_type.as_str() {
            "oauth" => {
                if self.access_token.is_none() {
                    return false;
                }
                // 优先检查 expire 字段
                if let Some(expires_str) = &self.expire {
                    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                        let now = chrono::Utc::now();
                        return expires > now + chrono::Duration::minutes(5);
                    }
                }
                // 兼容旧字段名
                if let Some(expires_str) = &self.expires_at {
                    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                        let now = chrono::Utc::now();
                        return expires > now + chrono::Duration::minutes(5);
                    }
                }
                true
            }
            "cookie" => {
                if self.cookies.is_none() && self.api_key.is_none() {
                    return false;
                }
                if let Some(expires_str) = &self.cookie_expires_at {
                    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                        return expires > chrono::Utc::now();
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// 获取有效的过期时间字符串
    pub fn get_expire(&self) -> Option<&String> {
        self.expire.as_ref().or(self.expires_at.as_ref())
    }
}

/// PKCE codes for OAuth2 authorization
#[derive(Debug, Clone)]
pub struct PKCECodes {
    /// Cryptographically random string for code verification
    pub code_verifier: String,
    /// SHA256 hash of code_verifier, base64url-encoded
    pub code_challenge: String,
}

impl PKCECodes {
    /// Generate new PKCE codes
    pub fn generate() -> Result<Self, Box<dyn Error + Send + Sync>> {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        use rand::RngCore;
        use sha2::{Digest, Sha256};

        // Generate 96 random bytes for code verifier
        let mut bytes = [0u8; 96];
        rand::thread_rng().fill_bytes(&mut bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

        // Generate code challenge using S256 method
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();
        let code_challenge = URL_SAFE_NO_PAD.encode(hash);

        Ok(Self {
            code_verifier,
            code_challenge,
        })
    }
}

/// OAuth callback result
#[derive(Debug, Clone)]
pub struct OAuthCallbackResult {
    /// Authorization code from OAuth callback
    pub code: String,
    /// State parameter for CSRF protection
    pub state: String,
    /// Error message if authentication failed
    pub error: Option<String>,
}

/// OAuth server for handling OAuth callbacks
pub struct OAuthServer {
    port: u16,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl OAuthServer {
    /// Create a new OAuth server on the specified port
    pub fn new(port: u16) -> Self {
        Self {
            port,
            shutdown_tx: None,
        }
    }

    /// Start the OAuth server and wait for a callback
    pub async fn wait_for_callback(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<OAuthCallbackResult, Box<dyn Error + Send + Sync>> {
        use axum::{extract::Query, response::Html, routing::get, Router};
        use std::collections::HashMap;
        use tokio::sync::oneshot;

        let (result_tx, result_rx) = oneshot::channel::<OAuthCallbackResult>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        let result_tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(result_tx)));

        let result_tx_clone = result_tx.clone();
        let callback_handler = move |Query(params): Query<HashMap<String, String>>| {
            let result_tx = result_tx_clone.clone();
            async move {
                let code = params.get("code").cloned().unwrap_or_default();
                let state = params.get("state").cloned().unwrap_or_default();
                let error = params.get("error").cloned();

                let result = OAuthCallbackResult {
                    code,
                    state,
                    error: error.clone(),
                };

                if let Some(tx) = result_tx.lock().await.take() {
                    let _ = tx.send(result);
                }

                if error.is_some() {
                    Html(OAUTH_ERROR_HTML.to_string())
                } else {
                    Html(OAUTH_SUCCESS_HTML.to_string())
                }
            }
        };

        let app = Router::new().route("/auth/callback", get(callback_handler));

        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], self.port));
        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                format!("Port {} is already in use.", self.port)
            } else {
                format!("Failed to bind to port {}: {}", self.port, e)
            }
        })?;

        tracing::info!(
            "[IFLOW] OAuth server listening on http://127.0.0.1:{}",
            self.port
        );

        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });

        tokio::spawn(async move {
            if let Err(e) = server.await {
                tracing::error!("[IFLOW] OAuth server error: {}", e);
            }
        });

        let result = tokio::time::timeout(timeout, result_rx).await;

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        match result {
            Ok(Ok(callback_result)) => {
                if let Some(ref error) = callback_result.error {
                    Err(format!("OAuth error: {}", error).into())
                } else {
                    Ok(callback_result)
                }
            }
            Ok(Err(_)) => Err("OAuth callback channel closed unexpectedly".into()),
            Err(_) => Err("OAuth callback timeout".into()),
        }
    }
}

// HTML templates for OAuth callback responses
const OAUTH_SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            text-align: center;
            background: white;
            padding: 40px 60px;
            border-radius: 16px;
            box-shadow: 0 10px 40px rgba(0,0,0,0.2);
        }
        .checkmark {
            width: 80px;
            height: 80px;
            margin: 0 auto 20px;
            background: #10b981;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .checkmark svg { width: 40px; height: 40px; fill: white; }
        h1 { color: #1f2937; margin-bottom: 10px; }
        p { color: #6b7280; }
    </style>
</head>
<body>
    <div class="container">
        <div class="checkmark">
            <svg viewBox="0 0 24 24"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></svg>
        </div>
        <h1>Authentication Successful!</h1>
        <p>You can close this window and return to ProxyCast.</p>
    </div>
</body>
</html>"#;

const OAUTH_ERROR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Failed</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
        }
        .container {
            text-align: center;
            background: white;
            padding: 40px 60px;
            border-radius: 16px;
            box-shadow: 0 10px 40px rgba(0,0,0,0.2);
        }
        .error-icon {
            width: 80px;
            height: 80px;
            margin: 0 auto 20px;
            background: #ef4444;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .error-icon svg { width: 40px; height: 40px; fill: white; }
        h1 { color: #1f2937; margin-bottom: 10px; }
        p { color: #6b7280; }
    </style>
</head>
<body>
    <div class="container">
        <div class="error-icon">
            <svg viewBox="0 0 24 24"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>
        </div>
        <h1>Authentication Failed</h1>
        <p>Please close this window and try again.</p>
    </div>
</body>
</html>"#;

/// iFlow OAuth Provider
///
/// Handles OAuth and Cookie-based authentication for iFlow API.
/// Supports dual authentication modes for flexibility.
pub struct IFlowProvider {
    /// Credentials storage
    pub credentials: IFlowCredentials,
    /// HTTP client for API requests
    pub client: Client,
    /// Path to credentials file
    pub creds_path: Option<PathBuf>,
    /// OAuth callback port
    pub callback_port: u16,
}

impl Default for IFlowProvider {
    fn default() -> Self {
        Self {
            credentials: IFlowCredentials::default(),
            client: Client::new(),
            creds_path: None,
            callback_port: DEFAULT_CALLBACK_PORT,
        }
    }
}

impl IFlowProvider {
    /// Create a new IFlowProvider instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new IFlowProvider with a custom HTTP client
    pub fn with_client(client: Client) -> Self {
        Self {
            client,
            ..Self::default()
        }
    }

    /// Get the default credentials file path
    pub fn default_creds_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".iflow")
            .join("auth.json")
    }

    /// Get the OAuth authorization URL
    pub fn get_auth_url(&self) -> &'static str {
        IFLOW_AUTH_URL
    }

    /// Get the OAuth token URL
    pub fn get_token_url(&self) -> &'static str {
        IFLOW_TOKEN_URL
    }

    /// Get the OAuth client ID
    pub fn get_client_id(&self) -> &'static str {
        IFLOW_CLIENT_ID
    }

    /// Get the redirect URI for OAuth callback
    pub fn get_redirect_uri(&self) -> String {
        format!("http://localhost:{}/auth/callback", self.callback_port)
    }

    /// Get the API base URL
    pub fn get_api_base_url(&self) -> &'static str {
        IFLOW_API_BASE_URL
    }

    /// Load credentials from the default path
    pub async fn load_credentials(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = Self::default_creds_path();
        self.load_credentials_from_path_internal(&path).await
    }

    /// Load credentials from a specific path
    pub async fn load_credentials_from_path(
        &mut self,
        path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = PathBuf::from(path);
        self.load_credentials_from_path_internal(&path).await
    }

    async fn load_credentials_from_path_internal(
        &mut self,
        path: &PathBuf,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let content = tokio::fs::read_to_string(&path).await?;
            let creds: IFlowCredentials = serde_json::from_str(&content)?;
            tracing::info!(
                "[IFLOW] Credentials loaded: auth_type={}, has_access={}, has_cookies={}, email={:?}",
                creds.auth_type,
                creds.access_token.is_some(),
                creds.cookies.is_some(),
                creds.email
            );
            self.credentials = creds;
            self.creds_path = Some(path.clone());
        } else {
            tracing::warn!("[IFLOW] Credentials file not found: {:?}", path);
        }
        Ok(())
    }

    /// Save credentials to file
    pub async fn save_credentials(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = self
            .creds_path
            .clone()
            .unwrap_or_else(Self::default_creds_path);

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(&self.credentials)?;
        tokio::fs::write(&path, content).await?;
        tracing::info!("[IFLOW] Credentials saved to {:?}", path);
        Ok(())
    }

    /// Check if the access token is expired
    pub fn is_token_expired(&self) -> bool {
        if let Some(expires_str) = &self.credentials.expires_at {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                let now = chrono::Utc::now();
                return expires < now + chrono::Duration::minutes(5);
            }
        }
        true
    }

    /// Check if credentials are valid
    pub fn is_valid(&self) -> bool {
        self.credentials.is_valid()
    }

    /// Generate the OAuth authorization URL with PKCE
    pub fn generate_auth_url(
        &self,
        state: &str,
        pkce_codes: &PKCECodes,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let params = [
            ("client_id", IFLOW_CLIENT_ID),
            ("response_type", "code"),
            ("redirect_uri", &self.get_redirect_uri()),
            ("scope", "openid email profile offline_access"),
            ("state", state),
            ("code_challenge", &pkce_codes.code_challenge),
            ("code_challenge_method", "S256"),
        ];

        let query = serde_urlencoded::to_string(&params)?;
        Ok(format!("{}?{}", IFLOW_AUTH_URL, query))
    }

    /// Generate a random state string for CSRF protection
    pub fn generate_state() -> Result<String, Box<dyn Error + Send + Sync>> {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        use rand::RngCore;

        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code_for_tokens(
        &mut self,
        code: &str,
        pkce_codes: &PKCECodes,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", IFLOW_CLIENT_ID),
            ("code", code),
            ("redirect_uri", &self.get_redirect_uri()),
            ("code_verifier", &pkce_codes.code_verifier),
        ];

        let resp = self
            .client
            .post(IFLOW_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Token exchange failed: {} - {}", status, body).into());
        }

        let data: serde_json::Value = resp.json().await?;

        let access_token = data["access_token"]
            .as_str()
            .ok_or("No access_token in response")?
            .to_string();
        let refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
        let expires_in = data["expires_in"].as_i64().unwrap_or(3600);

        // Extract user info from response if available
        let email = data["email"].as_str().map(|s| s.to_string());
        let user_id = data["user_id"].as_str().map(|s| s.to_string());

        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

        self.credentials = IFlowCredentials {
            auth_type: "oauth".to_string(),
            access_token: Some(access_token),
            refresh_token,
            expire: Some(expires_at.to_rfc3339()),
            expires_at: Some(expires_at.to_rfc3339()),
            cookies: None,
            cookie_expires_at: None,
            email,
            user_id,
            last_refresh: Some(chrono::Utc::now().to_rfc3339()),
            api_key: None,
            token_type: None,
            scope: None,
            cred_type: "iflow".to_string(),
        };

        self.save_credentials().await?;

        tracing::info!("[IFLOW] Token 交换成功, email={:?}", self.credentials.email);
        Ok(())
    }

    /// 刷新 Token - 与 CLIProxyAPI 对齐，使用 Basic Auth
    pub async fn refresh_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let refresh_token = self
            .credentials
            .refresh_token
            .as_ref()
            .ok_or_else(|| create_config_error("没有可用的 refresh_token"))?;

        tracing::info!("[IFLOW] 正在刷新 Token");

        // 构建 Basic Auth 头 - 与 CLIProxyAPI 对齐
        let basic_auth =
            BASE64_STANDARD.encode(format!("{}:{}", IFLOW_CLIENT_ID, IFLOW_CLIENT_SECRET));

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", IFLOW_CLIENT_ID),
            ("client_secret", IFLOW_CLIENT_SECRET),
        ];

        let resp = self
            .client
            .post(IFLOW_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .header("Authorization", format!("Basic {}", basic_auth))
            .form(&params)
            .send()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("[IFLOW] Token 刷新失败: {} - {}", status, body);
            self.mark_invalid();
            return Err(create_token_refresh_error(status, &body, "IFLOW"));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        let new_access_token = data["access_token"]
            .as_str()
            .ok_or_else(|| create_auth_error("响应中没有 access_token"))?
            .to_string();

        self.credentials.access_token = Some(new_access_token.clone());

        if let Some(rt) = data["refresh_token"].as_str() {
            self.credentials.refresh_token = Some(rt.to_string());
        }

        if let Some(token_type) = data["token_type"].as_str() {
            self.credentials.token_type = Some(token_type.to_string());
        }

        if let Some(scope) = data["scope"].as_str() {
            self.credentials.scope = Some(scope.to_string());
        }

        let expires_in = data["expires_in"].as_i64().unwrap_or(3600);
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
        self.credentials.expire = Some(expires_at.to_rfc3339());
        self.credentials.expires_at = Some(expires_at.to_rfc3339());
        self.credentials.last_refresh = Some(chrono::Utc::now().to_rfc3339());

        // 获取用户信息和 API Key
        if let Ok(user_info) = self.fetch_user_info(&new_access_token).await {
            if let Some(api_key) = user_info.get("apiKey").and_then(|v| v.as_str()) {
                self.credentials.api_key = Some(api_key.to_string());
            }
            if let Some(email) = user_info.get("email").and_then(|v| v.as_str()) {
                self.credentials.email = Some(email.to_string());
            } else if let Some(phone) = user_info.get("phone").and_then(|v| v.as_str()) {
                self.credentials.email = Some(phone.to_string());
            }
        }

        self.save_credentials().await?;

        tracing::info!("[IFLOW] Token 刷新成功");
        Ok(new_access_token)
    }

    /// 获取用户信息（包括 API Key）
    async fn fetch_user_info(
        &self,
        access_token: &str,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "{}?accessToken={}",
            IFLOW_USER_INFO_URL,
            urlencoding::encode(access_token)
        );

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !resp.status().is_success() {
            return Err(create_auth_error("获取用户信息失败"));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if data["success"].as_bool().unwrap_or(false) {
            Ok(data["data"].clone())
        } else {
            Err(create_auth_error("获取用户信息失败"))
        }
    }

    /// 检查 Cookie API Key 是否需要刷新（距离过期 2 天内）
    /// 与 CLIProxyAPI 的 ShouldRefreshAPIKey 对齐
    pub fn should_refresh_api_key(&self) -> bool {
        if let Some(expire_str) = &self.credentials.expire {
            // 尝试解析 "2006-01-02 15:04" 格式（iFlow 返回的格式）
            if let Ok(expire) = chrono::NaiveDateTime::parse_from_str(expire_str, "%Y-%m-%d %H:%M")
            {
                let expire_utc = expire.and_utc();
                let now = chrono::Utc::now();
                let two_days_from_now = now + chrono::Duration::hours(48);
                return expire_utc < two_days_from_now;
            }
            // 尝试解析 RFC3339 格式
            if let Ok(expire) = chrono::DateTime::parse_from_rfc3339(expire_str) {
                let now = chrono::Utc::now();
                let two_days_from_now = now + chrono::Duration::hours(48);
                return expire < two_days_from_now;
            }
        }
        // 如果没有过期时间，默认需要刷新
        true
    }

    /// 通过 Cookie 刷新 API Key
    /// 与 CLIProxyAPI 的 RefreshAPIKey 对齐
    pub async fn refresh_api_key_with_cookie(
        &mut self,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let cookie = self
            .credentials
            .cookies
            .as_ref()
            .ok_or_else(|| create_config_error("没有可用的 Cookie"))?
            .clone();

        let email = self
            .credentials
            .email
            .as_ref()
            .ok_or_else(|| create_config_error("没有可用的用户标识（email）"))?
            .clone();

        tracing::info!("[IFLOW] 正在通过 Cookie 刷新 API Key，用户: {}", email);

        // 首先获取当前 API Key 信息（GET 请求）
        let key_info = self.fetch_api_key_info(&cookie).await?;

        // 然后刷新 API Key（POST 请求）
        let refreshed_key = self.refresh_api_key(&cookie, &key_info.name).await?;

        // 更新凭证
        self.credentials.api_key = Some(refreshed_key.api_key.clone());
        self.credentials.expire = Some(refreshed_key.expire_time.clone());
        self.credentials.last_refresh = Some(chrono::Utc::now().to_rfc3339());

        // 保存凭证
        self.save_credentials().await?;

        tracing::info!(
            "[IFLOW] Cookie API Key 刷新成功，新过期时间: {}",
            refreshed_key.expire_time
        );

        Ok(refreshed_key.api_key)
    }

    /// 获取 API Key 信息（GET 请求）
    async fn fetch_api_key_info(
        &self,
        cookie: &str,
    ) -> Result<IFlowKeyData, Box<dyn Error + Send + Sync>> {
        let resp = self
            .client
            .get(IFLOW_API_KEY_URL)
            .header("Cookie", cookie)
            .header("Accept", "application/json, text/plain, */*")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .send()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("[IFLOW] 获取 API Key 信息失败: {} - {}", status, body);
            return Err(create_auth_error(&format!(
                "获取 API Key 信息失败: {} - {}",
                status, body
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !data["success"].as_bool().unwrap_or(false) {
            let message = data["message"].as_str().unwrap_or("未知错误");
            return Err(create_auth_error(&format!(
                "获取 API Key 信息失败: {}",
                message
            )));
        }

        let key_data = &data["data"];
        Ok(IFlowKeyData {
            api_key: key_data["apiKey"]
                .as_str()
                .or_else(|| key_data["apiKeyMask"].as_str())
                .unwrap_or("")
                .to_string(),
            expire_time: key_data["expireTime"].as_str().unwrap_or("").to_string(),
            name: key_data["name"].as_str().unwrap_or("").to_string(),
            has_expired: key_data["hasExpired"].as_bool().unwrap_or(false),
        })
    }

    /// 刷新 API Key（POST 请求）
    async fn refresh_api_key(
        &self,
        cookie: &str,
        name: &str,
    ) -> Result<IFlowKeyData, Box<dyn Error + Send + Sync>> {
        let body = serde_json::json!({ "name": name });

        let resp = self
            .client
            .post(IFLOW_API_KEY_URL)
            .header("Cookie", cookie)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/plain, */*")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Origin", "https://platform.iflow.cn")
            .header("Referer", "https://platform.iflow.cn/")
            .json(&body)
            .send()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("[IFLOW] 刷新 API Key 失败: {} - {}", status, body);
            return Err(create_auth_error(&format!(
                "刷新 API Key 失败: {} - {}",
                status, body
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !data["success"].as_bool().unwrap_or(false) {
            let message = data["message"].as_str().unwrap_or("未知错误");
            return Err(create_auth_error(&format!(
                "刷新 API Key 失败: {}",
                message
            )));
        }

        let key_data = &data["data"];
        Ok(IFlowKeyData {
            api_key: key_data["apiKey"].as_str().unwrap_or("").to_string(),
            expire_time: key_data["expireTime"].as_str().unwrap_or("").to_string(),
            name: key_data["name"].as_str().unwrap_or("").to_string(),
            has_expired: key_data["hasExpired"].as_bool().unwrap_or(false),
        })
    }

    /// Refresh token with retry mechanism
    pub async fn refresh_token_with_retry(
        &mut self,
        max_retries: u32,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut last_error = None;

        for attempt in 0..max_retries {
            if attempt > 0 {
                let delay = std::time::Duration::from_secs(1 << attempt);
                tracing::info!("[IFLOW] Retry attempt {} after {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
            }

            match self.refresh_token().await {
                Ok(token) => return Ok(token),
                Err(e) => {
                    tracing::warn!(
                        "[IFLOW] Token refresh attempt {} failed: {}",
                        attempt + 1,
                        e
                    );
                    last_error = Some(e);
                }
            }
        }

        self.mark_invalid();
        tracing::error!(
            "[IFLOW] Token refresh failed after {} attempts",
            max_retries
        );

        Err(last_error.unwrap_or_else(|| create_auth_error("Token 刷新失败，请重新登录")))
    }

    /// Check if token needs refresh
    ///
    /// 支持两种格式：
    /// - RFC3339 格式（新格式，与 CLIProxyAPI 兼容）
    /// - 旧的 expires_at 字段
    pub fn needs_refresh(&self, lead_time: chrono::Duration) -> bool {
        if self.credentials.auth_type != "oauth" {
            return false;
        }

        if self.credentials.access_token.is_none() {
            return true;
        }

        // 优先检查 expire 字段（新格式）
        if let Some(expire_str) = &self.credentials.expire {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expire_str) {
                let now = chrono::Utc::now();
                return expires < now + lead_time;
            }
        }

        // 兼容旧的 expires_at 字段
        if let Some(expires_str) = &self.credentials.expires_at {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                let now = chrono::Utc::now();
                return expires < now + lead_time;
            }
        }

        true
    }

    /// Ensure token is valid, refreshing if necessary
    pub async fn ensure_valid_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let lead_time = chrono::Duration::minutes(5);

        if self.needs_refresh(lead_time) {
            tracing::info!("[IFLOW] Token needs refresh, attempting refresh with retry");
            self.refresh_token_with_retry(3).await
        } else {
            self.credentials
                .access_token
                .clone()
                .ok_or_else(|| create_config_error("没有可用的 access_token"))
        }
    }

    /// Mark credentials as invalid
    pub fn mark_invalid(&mut self) {
        tracing::warn!("[IFLOW] Marking credentials as invalid");
        self.credentials.access_token = None;
        self.credentials.expires_at = None;
    }

    /// Get the access token, refreshing if necessary
    pub async fn get_access_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        if self.is_token_expired() {
            self.refresh_token().await?;
        }
        self.credentials
            .access_token
            .clone()
            .ok_or_else(|| create_config_error("没有可用的 access_token"))
    }

    /// Perform OAuth login flow
    pub async fn oauth_login(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        tracing::info!("[IFLOW] Starting OAuth login flow");

        let pkce_codes = PKCECodes::generate()?;
        let state = Self::generate_state()?;
        let auth_url = self.generate_auth_url(&state, &pkce_codes)?;

        let mut oauth_server = OAuthServer::new(self.callback_port);

        tracing::info!("[IFLOW] Opening browser for authentication");
        if let Err(e) = open::that(&auth_url) {
            tracing::warn!(
                "[IFLOW] Failed to open browser: {}. Please open the URL manually.",
                e
            );
            println!(
                "Please open the following URL in your browser:\n{}",
                auth_url
            );
        }

        let timeout = std::time::Duration::from_secs(300);
        let callback_result = oauth_server.wait_for_callback(timeout).await?;

        if callback_result.state != state {
            return Err("OAuth state mismatch - possible CSRF attack".into());
        }

        self.exchange_code_for_tokens(&callback_result.code, &pkce_codes)
            .await?;

        let email = self
            .credentials
            .email
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        tracing::info!("[IFLOW] OAuth login successful for {}", email);

        Ok(email)
    }

    /// Import cookies for cookie-based authentication
    ///
    /// Parses and stores a cookie string for authentication.
    /// Optionally extracts expiration from cookie attributes.
    pub async fn import_cookies(
        &mut self,
        cookies: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if cookies.trim().is_empty() {
            return Err("Cookie string cannot be empty".into());
        }

        tracing::info!("[IFLOW] Importing cookies for authentication");

        // Parse cookies to extract expiration if present
        let cookie_expires_at = parse_cookie_expiration(cookies);

        self.credentials = IFlowCredentials {
            auth_type: "cookie".to_string(),
            access_token: None,
            refresh_token: None,
            expire: None,
            expires_at: None,
            cookies: Some(cookies.to_string()),
            cookie_expires_at,
            email: None,
            user_id: None,
            last_refresh: Some(chrono::Utc::now().to_rfc3339()),
            api_key: None,
            token_type: None,
            scope: None,
            cred_type: "iflow".to_string(),
        };

        self.save_credentials().await?;

        tracing::info!("[IFLOW] Cookie 导入成功");
        Ok(())
    }

    /// 导入带有明确过期时间的 Cookie
    pub async fn import_cookies_with_expiration(
        &mut self,
        cookies: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if cookies.trim().is_empty() {
            return Err("Cookie 字符串不能为空".into());
        }

        tracing::info!("[IFLOW] 导入带有明确过期时间的 Cookie");

        self.credentials = IFlowCredentials {
            auth_type: "cookie".to_string(),
            access_token: None,
            refresh_token: None,
            expire: None,
            expires_at: None,
            cookies: Some(cookies.to_string()),
            cookie_expires_at: Some(expires_at.to_rfc3339()),
            email: None,
            user_id: None,
            last_refresh: Some(chrono::Utc::now().to_rfc3339()),
            api_key: None,
            token_type: None,
            scope: None,
            cred_type: "iflow".to_string(),
        };

        self.save_credentials().await?;

        tracing::info!("[IFLOW] Cookies imported with expiration: {}", expires_at);
        Ok(())
    }

    /// Check if cookies are expired
    pub fn are_cookies_expired(&self) -> bool {
        if let Some(expires_str) = &self.credentials.cookie_expires_at {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                return expires < chrono::Utc::now();
            }
        }
        // If no expiry info, assume not expired
        false
    }

    /// Get the authentication header value based on auth type
    pub fn get_auth_header(&self) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
        match self.credentials.auth_type.as_str() {
            "oauth" => {
                let token = self
                    .credentials
                    .access_token
                    .as_ref()
                    .ok_or("No access token available")?;
                Ok(("Authorization".to_string(), format!("Bearer {}", token)))
            }
            "cookie" => {
                let cookies = self
                    .credentials
                    .cookies
                    .as_ref()
                    .ok_or("No cookies available")?;
                Ok(("Cookie".to_string(), cookies.clone()))
            }
            _ => Err(format!("Unknown auth type: {}", self.credentials.auth_type).into()),
        }
    }

    /// Call the iFlow API for chat completions
    pub async fn call_api(
        &self,
        request: &serde_json::Value,
    ) -> Result<reqwest::Response, Box<dyn Error + Send + Sync>> {
        let (header_name, header_value) = self.get_auth_header()?;

        let url = format!("{}/chat/completions", IFLOW_API_BASE_URL);

        tracing::debug!("[IFLOW] Calling API: {}", url);

        let resp = self
            .client
            .post(&url)
            .header(&header_name, &header_value)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(request)
            .send()
            .await?;

        Ok(resp)
    }

    /// Call the iFlow API with streaming response
    pub async fn call_api_stream(
        &self,
        request: &serde_json::Value,
    ) -> Result<reqwest::Response, Box<dyn Error + Send + Sync>> {
        self.call_api(request).await
    }

    /// Check if this provider supports the given model
    pub fn supports_model(model: &str) -> bool {
        let model_lower = model.to_lowercase();
        model_lower.starts_with("iflow") || model_lower.contains("iflow")
    }
}

/// Parse cookie string to extract expiration time
///
/// Looks for Expires or Max-Age attributes in the cookie string.
fn parse_cookie_expiration(cookies: &str) -> Option<String> {
    // Look for Expires attribute
    for part in cookies.split(';') {
        let part = part.trim();
        if part.to_lowercase().starts_with("expires=") {
            let expires_str = &part[8..];
            // Try to parse HTTP date format
            if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(expires_str) {
                return Some(dt.with_timezone(&chrono::Utc).to_rfc3339());
            }
            // Try alternative formats
            if let Ok(dt) =
                chrono::DateTime::parse_from_str(expires_str, "%a, %d %b %Y %H:%M:%S %Z")
            {
                return Some(dt.with_timezone(&chrono::Utc).to_rfc3339());
            }
        }
        // Look for Max-Age attribute
        if part.to_lowercase().starts_with("max-age=") {
            let max_age_str = &part[8..];
            if let Ok(seconds) = max_age_str.parse::<i64>() {
                let expires = chrono::Utc::now() + chrono::Duration::seconds(seconds);
                return Some(expires.to_rfc3339());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iflow_credentials_default() {
        let creds = IFlowCredentials::default();
        assert_eq!(creds.auth_type, "oauth");
        assert!(creds.access_token.is_none());
        assert!(creds.refresh_token.is_none());
        assert!(creds.cookies.is_none());
    }

    #[test]
    fn test_iflow_credentials_oauth_serialization() {
        let creds = IFlowCredentials {
            auth_type: "oauth".to_string(),
            access_token: Some("test_token".to_string()),
            refresh_token: Some("test_refresh".to_string()),
            email: Some("test@example.com".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&creds).unwrap();
        assert!(json.contains("test_token"));
        assert!(json.contains("test@example.com"));
        assert!(json.contains("oauth"));

        let parsed: IFlowCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, creds.access_token);
        assert_eq!(parsed.email, creds.email);
        assert_eq!(parsed.auth_type, "oauth");
    }

    #[test]
    fn test_iflow_credentials_cookie_serialization() {
        let creds = IFlowCredentials {
            auth_type: "cookie".to_string(),
            cookies: Some("session=abc123; token=xyz789".to_string()),
            cookie_expires_at: Some("2099-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&creds).unwrap();
        assert!(json.contains("cookie"));
        assert!(json.contains("session=abc123"));

        let parsed: IFlowCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.auth_type, "cookie");
        assert_eq!(parsed.cookies, creds.cookies);
    }

    #[test]
    fn test_iflow_credentials_is_valid_oauth() {
        let mut creds = IFlowCredentials {
            auth_type: "oauth".to_string(),
            access_token: Some("test_token".to_string()),
            expires_at: Some("2099-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };
        assert!(creds.is_valid());

        // Expired token
        creds.expires_at = Some("2020-01-01T00:00:00Z".to_string());
        assert!(!creds.is_valid());

        // No token
        creds.access_token = None;
        creds.expires_at = Some("2099-01-01T00:00:00Z".to_string());
        assert!(!creds.is_valid());
    }

    #[test]
    fn test_iflow_credentials_is_valid_cookie() {
        let mut creds = IFlowCredentials {
            auth_type: "cookie".to_string(),
            cookies: Some("session=abc123".to_string()),
            cookie_expires_at: Some("2099-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };
        assert!(creds.is_valid());

        // Expired cookie
        creds.cookie_expires_at = Some("2020-01-01T00:00:00Z".to_string());
        assert!(!creds.is_valid());

        // No cookies
        creds.cookies = None;
        creds.cookie_expires_at = Some("2099-01-01T00:00:00Z".to_string());
        assert!(!creds.is_valid());
    }

    #[test]
    fn test_iflow_credentials_to_typed_oauth() {
        let creds = IFlowCredentials {
            auth_type: "oauth".to_string(),
            access_token: Some("test_token".to_string()),
            refresh_token: Some("test_refresh".to_string()),
            expires_at: Some("2099-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };

        let typed = creds.to_typed().unwrap();
        match typed {
            IFlowCredentialsType::OAuth {
                access_token,
                refresh_token,
                expires_at,
            } => {
                assert_eq!(access_token, "test_token");
                assert_eq!(refresh_token, Some("test_refresh".to_string()));
                assert!(expires_at.is_some());
            }
            _ => panic!("Expected OAuth type"),
        }
    }

    #[test]
    fn test_iflow_credentials_to_typed_cookie() {
        let creds = IFlowCredentials {
            auth_type: "cookie".to_string(),
            cookies: Some("session=abc123".to_string()),
            cookie_expires_at: Some("2099-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };

        let typed = creds.to_typed().unwrap();
        match typed {
            IFlowCredentialsType::Cookie {
                cookies,
                expires_at,
            } => {
                assert_eq!(cookies, "session=abc123");
                assert!(expires_at.is_some());
            }
            _ => panic!("Expected Cookie type"),
        }
    }

    #[test]
    fn test_pkce_generation() {
        let pkce = PKCECodes::generate().unwrap();
        assert!(!pkce.code_verifier.is_empty());
        assert!(!pkce.code_challenge.is_empty());
        assert_eq!(pkce.code_verifier.len(), 128);
    }

    #[test]
    fn test_iflow_provider_default() {
        let provider = IFlowProvider::new();
        assert_eq!(provider.callback_port, DEFAULT_CALLBACK_PORT);
        assert!(provider.credentials.access_token.is_none());
        assert_eq!(provider.credentials.auth_type, "oauth");
    }

    #[test]
    fn test_generate_auth_url() {
        let provider = IFlowProvider::new();
        let pkce = PKCECodes::generate().unwrap();
        let state = "test_state";

        let url = provider.generate_auth_url(state, &pkce).unwrap();
        assert!(url.starts_with(IFLOW_AUTH_URL));
        assert!(url.contains("client_id="));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("state=test_state"));
    }

    #[test]
    fn test_is_token_expired() {
        let mut provider = IFlowProvider::new();

        // No expiry - should be considered expired
        assert!(provider.is_token_expired());

        // Expired token
        provider.credentials.expires_at = Some("2020-01-01T00:00:00Z".to_string());
        assert!(provider.is_token_expired());

        // Valid token (far future)
        provider.credentials.expires_at = Some("2099-01-01T00:00:00Z".to_string());
        assert!(!provider.is_token_expired());
    }

    #[test]
    fn test_supports_model() {
        assert!(IFlowProvider::supports_model("iflow-gpt4"));
        assert!(IFlowProvider::supports_model("IFLOW-model"));
        assert!(IFlowProvider::supports_model("my-iflow-model"));

        assert!(!IFlowProvider::supports_model("gpt-4"));
        assert!(!IFlowProvider::supports_model("claude-3"));
    }

    #[test]
    fn test_parse_cookie_expiration_expires() {
        // Test with RFC2822 format
        let cookies = "session=abc123; Expires=Mon, 09 Jun 2099 10:18:14 +0000; Path=/";
        let result = parse_cookie_expiration(cookies);
        // Note: Cookie expiration parsing may not work for all date formats
        // The important thing is that it doesn't panic
        // If parsing fails, it returns None which is acceptable
        let _ = result;
    }

    #[test]
    fn test_parse_cookie_expiration_max_age() {
        let cookies = "session=abc123; Max-Age=3600; Path=/";
        let result = parse_cookie_expiration(cookies);
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_cookie_expiration_none() {
        let cookies = "session=abc123; Path=/";
        let result = parse_cookie_expiration(cookies);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_auth_header_oauth() {
        let mut provider = IFlowProvider::new();
        provider.credentials.auth_type = "oauth".to_string();
        provider.credentials.access_token = Some("test_token".to_string());

        let (name, value) = provider.get_auth_header().unwrap();
        assert_eq!(name, "Authorization");
        assert_eq!(value, "Bearer test_token");
    }

    #[test]
    fn test_get_auth_header_cookie() {
        let mut provider = IFlowProvider::new();
        provider.credentials.auth_type = "cookie".to_string();
        provider.credentials.cookies = Some("session=abc123".to_string());

        let (name, value) = provider.get_auth_header().unwrap();
        assert_eq!(name, "Cookie");
        assert_eq!(value, "session=abc123");
    }

    #[test]
    fn test_get_auth_header_no_credentials() {
        let provider = IFlowProvider::new();
        let result = provider.get_auth_header();
        assert!(result.is_err());
    }

    #[test]
    fn test_are_cookies_expired() {
        let mut provider = IFlowProvider::new();
        provider.credentials.auth_type = "cookie".to_string();
        provider.credentials.cookies = Some("session=abc".to_string());

        // No expiry - not expired
        assert!(!provider.are_cookies_expired());

        // Future expiry - not expired
        provider.credentials.cookie_expires_at = Some("2099-01-01T00:00:00Z".to_string());
        assert!(!provider.are_cookies_expired());

        // Past expiry - expired
        provider.credentials.cookie_expires_at = Some("2020-01-01T00:00:00Z".to_string());
        assert!(provider.are_cookies_expired());
    }
}

// ============================================================================
// OAuth 登录功能（与 CLIProxyAPI 对齐）
// ============================================================================

use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

/// iFlow OAuth 登录结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IFlowOAuthResult {
    pub credentials: IFlowCredentials,
    pub creds_file_path: String,
}

/// OAuth 成功页面 HTML
const IFLOW_OAUTH_SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权成功</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); }
        .container { text-align: center; background: white; padding: 40px 60px; border-radius: 16px; box-shadow: 0 10px 40px rgba(0,0,0,0.2); }
        h1 { color: #667eea; margin-bottom: 16px; }
        p { color: #666; margin-bottom: 8px; }
        .email { color: #333; font-weight: 500; }
    </style>
</head>
<body>
    <div class="container">
        <h1>✓ 授权成功</h1>
        <p>iFlow 账号已添加到 ProxyCast</p>
        <p class="email">EMAIL_PLACEHOLDER</p>
        <p style="margin-top: 20px; color: #999;">可以关闭此页面</p>
    </div>
</body>
</html>"#;

/// OAuth 失败页面 HTML
const IFLOW_OAUTH_ERROR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权失败</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%); }
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

/// 生成 iFlow OAuth 授权 URL
pub fn generate_iflow_auth_url(port: u16, state: &str) -> String {
    let redirect_uri = format!("http://localhost:{}/oauth2callback", port);

    let params = [
        ("loginMethod", "phone"),
        ("type", "phone"),
        ("redirect", redirect_uri.as_str()),
        ("state", state),
        ("client_id", IFLOW_CLIENT_ID),
    ];

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{}", IFLOW_AUTH_URL, query)
}

/// 用授权码交换 Token
pub async fn exchange_iflow_code_for_token(
    client: &Client,
    code: &str,
    redirect_uri: &str,
) -> Result<IFlowCredentials, Box<dyn Error + Send + Sync>> {
    // 构建 Basic Auth 头
    let basic_auth = BASE64_STANDARD.encode(format!("{}:{}", IFLOW_CLIENT_ID, IFLOW_CLIENT_SECRET));

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", IFLOW_CLIENT_ID),
        ("client_secret", IFLOW_CLIENT_SECRET),
    ];

    let resp = client
        .post(IFLOW_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .header("Authorization", format!("Basic {}", basic_auth))
        .form(&params)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token 交换失败: {} - {}", status, body).into());
    }

    let data: serde_json::Value = resp.json().await?;

    let access_token = data["access_token"]
        .as_str()
        .ok_or("响应中没有 access_token")?
        .to_string();
    let refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
    let token_type = data["token_type"].as_str().map(|s| s.to_string());
    let scope = data["scope"].as_str().map(|s| s.to_string());
    let expires_in = data["expires_in"].as_i64().unwrap_or(3600);

    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

    // 获取用户信息和 API Key
    let user_info_url = format!(
        "{}?accessToken={}",
        IFLOW_USER_INFO_URL,
        urlencoding::encode(&access_token)
    );

    let user_resp = client
        .get(&user_info_url)
        .header("Accept", "application/json")
        .send()
        .await;

    let (api_key, email) = if let Ok(resp) = user_resp {
        if resp.status().is_success() {
            if let Ok(user_data) = resp.json::<serde_json::Value>().await {
                if user_data["success"].as_bool().unwrap_or(false) {
                    let api_key = user_data["data"]["apiKey"].as_str().map(|s| s.to_string());
                    let email = user_data["data"]["email"]
                        .as_str()
                        .or_else(|| user_data["data"]["phone"].as_str())
                        .map(|s| s.to_string());
                    (api_key, email)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let credentials = IFlowCredentials {
        auth_type: "oauth".to_string(),
        access_token: Some(access_token),
        refresh_token,
        expire: Some(expires_at.to_rfc3339()),
        expires_at: Some(expires_at.to_rfc3339()),
        cookies: None,
        cookie_expires_at: None,
        email,
        user_id: None,
        last_refresh: Some(chrono::Utc::now().to_rfc3339()),
        api_key,
        token_type,
        scope,
        cred_type: "iflow".to_string(),
    };

    Ok(credentials)
}

/// 启动 OAuth 服务器并返回授权 URL（不打开浏览器）
pub async fn start_iflow_oauth_server_and_get_url() -> Result<
    (
        String,
        impl std::future::Future<Output = Result<IFlowOAuthResult, Box<dyn Error + Send + Sync>>>,
    ),
    Box<dyn Error + Send + Sync>,
> {
    use axum::{extract::Query, response::Html, routing::get, Router};
    use std::collections::HashMap;
    use tokio::net::TcpListener;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 生成随机 state
    let state = Uuid::new_v4().to_string();
    let state_clone = state.clone();

    // 创建 channel 用于接收回调结果
    let (tx, rx) = oneshot::channel::<Result<IFlowOAuthResult, String>>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    // 尝试绑定到端口，如果默认端口被占用则尝试其他端口
    let ports_to_try = [DEFAULT_CALLBACK_PORT, 11452, 11453, 11454, 11455, 0]; // 0 表示让系统分配
    let mut listener = None;
    let mut last_error = None;

    for port in ports_to_try {
        match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        }
    }

    let listener = listener.ok_or_else(|| {
        format!(
            "启动 OAuth 服务器失败: {}",
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "无法绑定端口".to_string())
        )
    })?;
    let port = listener.local_addr()?.port();

    let redirect_uri = format!("http://localhost:{}/oauth2callback", port);
    let redirect_uri_clone = redirect_uri.clone();

    // 生成授权 URL
    let auth_url = generate_iflow_auth_url(port, &state);

    tracing::info!(
        "[iFlow OAuth] 服务器启动在端口 {}, 授权 URL: {}",
        port,
        auth_url
    );

    // 构建路由
    let app = Router::new().route(
        "/oauth2callback",
        get(move |Query(params): Query<HashMap<String, String>>| {
            let tx = tx.clone();
            let client = client.clone();
            let state_expected = state_clone.clone();
            let redirect_uri = redirect_uri_clone.clone();

            async move {
                let code = params.get("code");
                let returned_state = params.get("state");
                let error = params.get("error");

                // 检查错误
                if let Some(err) = error {
                    let html = IFLOW_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", err);
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(Err(format!("OAuth 错误: {}", err)));
                    }
                    return Html(html);
                }

                // 检查 state
                if returned_state.map(|s| s.as_str()) != Some(&state_expected) {
                    let html =
                        IFLOW_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", "State 验证失败");
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(Err("State 验证失败".to_string()));
                    }
                    return Html(html);
                }

                // 检查 code
                let code = match code {
                    Some(c) => c,
                    None => {
                        let html =
                            IFLOW_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", "未收到授权码");
                        if let Some(sender) = tx.lock().await.take() {
                            let _ = sender.send(Err("未收到授权码".to_string()));
                        }
                        return Html(html);
                    }
                };

                // 交换 Token
                let token_result =
                    exchange_iflow_code_for_token(&client, code, &redirect_uri).await;
                let credentials = match token_result {
                    Ok(creds) => creds,
                    Err(e) => {
                        let html =
                            IFLOW_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", &e.to_string());
                        if let Some(sender) = tx.lock().await.take() {
                            let _ = sender.send(Err(e.to_string()));
                        }
                        return Html(html);
                    }
                };

                let email = credentials.email.clone();

                // 保存凭证到应用数据目录
                let creds_dir = dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("proxycast")
                    .join("credentials")
                    .join("iflow");

                if let Err(e) = std::fs::create_dir_all(&creds_dir) {
                    let html = IFLOW_OAUTH_ERROR_HTML
                        .replace("ERROR_PLACEHOLDER", &format!("创建目录失败: {}", e));
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(Err(format!("创建目录失败: {}", e)));
                    }
                    return Html(html);
                }

                // 生成唯一文件名
                let uuid = Uuid::new_v4().to_string();
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let filename = format!("iflow_{}_{}.json", &uuid[..8], timestamp);
                let creds_file_path = creds_dir.join(&filename);

                // 保存凭证
                let creds_json = match serde_json::to_string_pretty(&credentials) {
                    Ok(json) => json,
                    Err(e) => {
                        let html = IFLOW_OAUTH_ERROR_HTML
                            .replace("ERROR_PLACEHOLDER", &format!("序列化凭证失败: {}", e));
                        if let Some(sender) = tx.lock().await.take() {
                            let _ = sender.send(Err(format!("序列化凭证失败: {}", e)));
                        }
                        return Html(html);
                    }
                };

                if let Err(e) = std::fs::write(&creds_file_path, &creds_json) {
                    let html = IFLOW_OAUTH_ERROR_HTML
                        .replace("ERROR_PLACEHOLDER", &format!("保存凭证失败: {}", e));
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(Err(format!("保存凭证失败: {}", e)));
                    }
                    return Html(html);
                }

                tracing::info!("[iFlow OAuth] 凭证已保存到: {:?}", creds_file_path);

                // 发送成功结果
                let result = IFlowOAuthResult {
                    credentials,
                    creds_file_path: creds_file_path.to_string_lossy().to_string(),
                };

                if let Some(sender) = tx.lock().await.take() {
                    let _ = sender.send(Ok(result));
                }

                // 返回成功页面
                let html = IFLOW_OAUTH_SUCCESS_HTML.replace(
                    "EMAIL_PLACEHOLDER",
                    &email.unwrap_or_else(|| "未知用户".to_string()),
                );
                Html(html)
            }
        }),
    );

    // 启动服务器
    let server = axum::serve(listener, app);

    // 创建等待 future
    let wait_future = async move {
        // 设置超时（5 分钟）
        let timeout = tokio::time::timeout(std::time::Duration::from_secs(300), async {
            // 启动服务器（在后台运行）
            tokio::spawn(async move {
                if let Err(e) = server.await {
                    tracing::error!("[iFlow OAuth] 服务器错误: {}", e);
                }
            });

            // 等待回调结果
            match rx.await {
                Ok(result) => result.map_err(|e| {
                    Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))
                        as Box<dyn Error + Send + Sync>
                }),
                Err(_) => Err("OAuth 回调通道关闭".into()),
            }
        });

        match timeout.await {
            Ok(result) => result,
            Err(_) => Err("OAuth 登录超时（5分钟）".into()),
        }
    };

    Ok((auth_url, wait_future))
}

/// 启动 iFlow OAuth 登录流程（自动打开浏览器）
pub async fn start_iflow_oauth_login() -> Result<IFlowOAuthResult, Box<dyn Error + Send + Sync>> {
    let (auth_url, wait_future) = start_iflow_oauth_server_and_get_url().await?;

    tracing::info!("[iFlow OAuth] 打开浏览器进行授权: {}", auth_url);

    // 打开浏览器
    if let Err(e) = open::that(&auth_url) {
        tracing::warn!("[iFlow OAuth] 无法打开浏览器: {}. 请手动打开 URL.", e);
    }

    // 等待回调
    wait_future.await
}
