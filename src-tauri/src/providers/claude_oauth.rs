//! Claude OAuth Provider
//!
//! 实现 Anthropic Claude OAuth 认证流程，与 CLIProxyAPI 对齐。
//! 支持 Token 刷新、重试机制和统一凭证格式。

use super::error::{
    create_auth_error, create_config_error, create_token_refresh_error, ProviderError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

// OAuth 端点和凭证 - 与 CLIProxyAPI 完全一致
const CLAUDE_AUTH_URL: &str = "https://claude.ai/oauth/authorize";
const CLAUDE_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const CLAUDE_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const DEFAULT_CALLBACK_PORT: u16 = 54545;

/// Claude OAuth 凭证存储
///
/// 与 CLIProxyAPI 的 ClaudeTokenStorage 格式兼容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeOAuthCredentials {
    /// 访问令牌
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// 刷新令牌
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// 用户邮箱
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// 过期时间（RFC3339 格式）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<String>,
    /// 最后刷新时间（RFC3339 格式）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<String>,
    /// 凭证类型标识
    #[serde(default = "default_claude_type", rename = "type")]
    pub cred_type: String,
}

fn default_claude_type() -> String {
    "claude_oauth".to_string()
}

impl Default for ClaudeOAuthCredentials {
    fn default() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
            email: None,
            expire: None,
            last_refresh: None,
            cred_type: default_claude_type(),
        }
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

        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

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

/// Claude OAuth Provider
///
/// 处理 Anthropic Claude 的 OAuth 认证和 API 调用
pub struct ClaudeOAuthProvider {
    /// OAuth 凭证
    pub credentials: ClaudeOAuthCredentials,
    /// HTTP 客户端
    pub client: Client,
    /// 凭证文件路径
    pub creds_path: Option<PathBuf>,
    /// OAuth 回调端口
    pub callback_port: u16,
}

impl Default for ClaudeOAuthProvider {
    fn default() -> Self {
        Self {
            credentials: ClaudeOAuthCredentials::default(),
            client: Client::new(),
            creds_path: None,
            callback_port: DEFAULT_CALLBACK_PORT,
        }
    }
}

impl ClaudeOAuthProvider {
    /// 创建新的 ClaudeOAuthProvider 实例
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用自定义 HTTP 客户端创建
    pub fn with_client(client: Client) -> Self {
        Self {
            client,
            ..Self::default()
        }
    }

    /// 获取默认凭证文件路径
    pub fn default_creds_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude")
            .join("oauth_creds.json")
    }

    /// 从默认路径加载凭证
    pub async fn load_credentials(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = Self::default_creds_path();
        self.load_credentials_from_path_internal(&path).await
    }

    /// 从指定路径加载凭证
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
            let creds: ClaudeOAuthCredentials = serde_json::from_str(&content)?;
            tracing::info!(
                "[CLAUDE_OAUTH] 凭证已加载: has_access={}, has_refresh={}, email={:?}",
                creds.access_token.is_some(),
                creds.refresh_token.is_some(),
                creds.email
            );
            self.credentials = creds;
            self.creds_path = Some(path.clone());
        } else {
            tracing::warn!("[CLAUDE_OAUTH] 凭证文件不存在: {:?}", path);
        }
        Ok(())
    }

    /// 保存凭证到文件
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
        tracing::info!("[CLAUDE_OAUTH] 凭证已保存到 {:?}", path);
        Ok(())
    }

    /// 检查 Token 是否有效
    pub fn is_token_valid(&self) -> bool {
        if self.credentials.access_token.is_none() {
            return false;
        }

        if let Some(expire_str) = &self.credentials.expire {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expire_str) {
                let now = chrono::Utc::now();
                return expires > now + chrono::Duration::minutes(5);
            }
        }

        true
    }

    /// 刷新 Token - 与 CLIProxyAPI 对齐，使用 JSON 格式
    pub async fn refresh_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let refresh_token = self
            .credentials
            .refresh_token
            .as_ref()
            .ok_or_else(|| create_config_error("没有可用的 refresh_token"))?;

        tracing::info!("[CLAUDE_OAUTH] 正在刷新 Token");

        // 与 CLIProxyAPI 对齐：使用 JSON 格式请求体
        let body = serde_json::json!({
            "client_id": CLAUDE_CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token
        });

        let resp = self
            .client
            .post(CLAUDE_TOKEN_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| Box::new(ProviderError::from(e)) as Box<dyn Error + Send + Sync>)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("[CLAUDE_OAUTH] Token 刷新失败: {} - {}", status, body);
            self.mark_invalid();
            return Err(create_token_refresh_error(status, &body, "CLAUDE_OAUTH"));
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

        // 从响应中提取用户邮箱
        if let Some(email) = data["account"]["email_address"].as_str() {
            self.credentials.email = Some(email.to_string());
        }

        // 更新过期时间
        let expires_in = data["expires_in"].as_i64().unwrap_or(3600);
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
        self.credentials.expire = Some(expires_at.to_rfc3339());
        self.credentials.last_refresh = Some(chrono::Utc::now().to_rfc3339());

        self.save_credentials().await?;

        tracing::info!("[CLAUDE_OAUTH] Token 刷新成功");
        Ok(new_access_token)
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
                tracing::info!("[CLAUDE_OAUTH] 第 {} 次重试，等待 {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
            }

            match self.refresh_token().await {
                Ok(token) => return Ok(token),
                Err(e) => {
                    tracing::warn!(
                        "[CLAUDE_OAUTH] Token 刷新第 {} 次尝试失败: {}",
                        attempt + 1,
                        e
                    );
                    last_error = Some(e);
                }
            }
        }

        self.mark_invalid();
        tracing::error!("[CLAUDE_OAUTH] Token 刷新在 {} 次尝试后失败", max_retries);
        Err(last_error.unwrap_or_else(|| create_auth_error("Token 刷新失败，请重新登录")))
    }

    /// 确保 Token 有效，必要时自动刷新
    pub async fn ensure_valid_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        if !self.is_token_valid() {
            tracing::info!("[CLAUDE_OAUTH] Token 需要刷新");
            self.refresh_token_with_retry(3).await
        } else {
            self.credentials
                .access_token
                .clone()
                .ok_or_else(|| create_config_error("没有可用的 access_token"))
        }
    }

    /// 标记凭证为无效
    pub fn mark_invalid(&mut self) {
        tracing::warn!("[CLAUDE_OAUTH] 标记凭证为无效");
        self.credentials.access_token = None;
        self.credentials.expire = None;
    }

    /// 获取 OAuth 授权 URL
    pub fn get_auth_url(&self) -> &'static str {
        CLAUDE_AUTH_URL
    }

    /// 获取 OAuth Token URL
    pub fn get_token_url(&self) -> &'static str {
        CLAUDE_TOKEN_URL
    }

    /// 获取 OAuth Client ID
    pub fn get_client_id(&self) -> &'static str {
        CLAUDE_CLIENT_ID
    }

    /// 获取回调 URI
    pub fn get_redirect_uri(&self) -> String {
        format!("http://localhost:{}/callback", self.callback_port)
    }
}

// ============================================================================
// OAuth 登录功能
// ============================================================================

use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

/// OAuth 登录成功后的凭证信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeOAuthResult {
    pub credentials: ClaudeOAuthCredentials,
    pub creds_file_path: String,
}

/// 生成 Claude OAuth 授权 URL
pub fn generate_claude_auth_url(port: u16, state: &str, code_challenge: &str) -> String {
    let redirect_uri = format!("http://localhost:{}/oauth-callback", port);

    let params = [
        ("client_id", CLAUDE_CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri.as_str()),
        ("scope", "user:inference user:profile"),
        ("state", state),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{}", CLAUDE_AUTH_URL, query)
}

/// 用授权码交换 Token
pub async fn exchange_claude_code_for_token(
    client: &Client,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": CLAUDE_CLIENT_ID,
        "code": code,
        "redirect_uri": redirect_uri,
        "code_verifier": code_verifier
    });

    let resp = client
        .post(CLAUDE_TOKEN_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token 交换失败: {} - {}", status, body).into());
    }

    let data: serde_json::Value = resp.json().await?;
    Ok(data)
}

/// OAuth 成功页面 HTML
const CLAUDE_OAUTH_SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权成功</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: linear-gradient(135deg, #d97706 0%, #b45309 100%); }
        .container { text-align: center; background: white; padding: 40px 60px; border-radius: 16px; box-shadow: 0 10px 40px rgba(0,0,0,0.2); }
        h1 { color: #d97706; margin-bottom: 16px; }
        p { color: #666; margin-bottom: 8px; }
        .email { color: #333; font-weight: 500; }
    </style>
</head>
<body>
    <div class="container">
        <h1>✓ 授权成功</h1>
        <p>Claude 账号已添加到 ProxyCast</p>
        <p class="email">EMAIL_PLACEHOLDER</p>
        <p style="margin-top: 20px; color: #999;">可以关闭此页面</p>
    </div>
</body>
</html>"#;

/// OAuth 失败页面 HTML
const CLAUDE_OAUTH_ERROR_HTML: &str = r#"<!DOCTYPE html>
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

/// 启动 OAuth 服务器并返回授权 URL（不打开浏览器）
pub async fn start_claude_oauth_server_and_get_url() -> Result<
    (
        String,
        impl std::future::Future<Output = Result<ClaudeOAuthResult, Box<dyn Error + Send + Sync>>>,
    ),
    Box<dyn Error + Send + Sync>,
> {
    use axum::{extract::Query, response::Html, routing::get, Router};
    use std::collections::HashMap;
    use tokio::net::TcpListener;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 生成 PKCE codes
    let pkce_codes = PKCECodes::generate()?;
    let code_verifier = pkce_codes.code_verifier.clone();
    let code_challenge = pkce_codes.code_challenge.clone();

    // 生成随机 state
    let state = Uuid::new_v4().to_string();
    let state_clone = state.clone();

    // 创建 channel 用于接收回调结果
    let (tx, rx) = oneshot::channel::<Result<ClaudeOAuthResult, String>>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    // 绑定到随机端口
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    let redirect_uri = format!("http://localhost:{}/oauth-callback", port);
    let redirect_uri_clone = redirect_uri.clone();

    // 生成授权 URL
    let auth_url = generate_claude_auth_url(port, &state, &code_challenge);

    tracing::info!(
        "[Claude OAuth] 服务器启动在端口 {}, 授权 URL: {}",
        port,
        auth_url
    );

    // 构建路由
    let app = Router::new().route(
        "/oauth-callback",
        get(move |Query(params): Query<HashMap<String, String>>| {
            let tx = tx.clone();
            let client = client.clone();
            let state_expected = state_clone.clone();
            let redirect_uri = redirect_uri_clone.clone();
            let code_verifier = code_verifier.clone();

            async move {
                let code = params.get("code");
                let returned_state = params.get("state");
                let error = params.get("error");

                // 检查错误
                if let Some(err) = error {
                    let html = CLAUDE_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", err);
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(Err(format!("OAuth 错误: {}", err)));
                    }
                    return Html(html);
                }

                // 检查 state
                if returned_state.map(|s| s.as_str()) != Some(&state_expected) {
                    let html =
                        CLAUDE_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", "State 验证失败");
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
                            CLAUDE_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", "未收到授权码");
                        if let Some(sender) = tx.lock().await.take() {
                            let _ = sender.send(Err("未收到授权码".to_string()));
                        }
                        return Html(html);
                    }
                };

                // 交换 Token
                let token_result =
                    exchange_claude_code_for_token(&client, code, &code_verifier, &redirect_uri)
                        .await;
                let token_data = match token_result {
                    Ok(data) => data,
                    Err(e) => {
                        let html =
                            CLAUDE_OAUTH_ERROR_HTML.replace("ERROR_PLACEHOLDER", &e.to_string());
                        if let Some(sender) = tx.lock().await.take() {
                            let _ = sender.send(Err(e.to_string()));
                        }
                        return Html(html);
                    }
                };

                let access_token = token_data["access_token"].as_str().unwrap_or_default();
                let refresh_token = token_data["refresh_token"].as_str().map(|s| s.to_string());
                let expires_in = token_data["expires_in"].as_i64();

                // 从响应中提取用户邮箱
                let email = token_data["account"]["email_address"]
                    .as_str()
                    .map(|s| s.to_string());

                // 构建凭证
                let now = chrono::Utc::now();
                let credentials = ClaudeOAuthCredentials {
                    access_token: Some(access_token.to_string()),
                    refresh_token,
                    email: email.clone(),
                    expire: expires_in.map(|e| (now + chrono::Duration::seconds(e)).to_rfc3339()),
                    last_refresh: Some(now.to_rfc3339()),
                    cred_type: "claude_oauth".to_string(),
                };

                // 保存凭证到应用数据目录
                let creds_dir = dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("proxycast")
                    .join("credentials")
                    .join("claude_oauth");

                if let Err(e) = std::fs::create_dir_all(&creds_dir) {
                    let html = CLAUDE_OAUTH_ERROR_HTML
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
                let filename = format!("claude_oauth_{}_{}.json", &uuid[..8], timestamp);
                let creds_file_path = creds_dir.join(&filename);

                // 保存凭证
                let creds_json = match serde_json::to_string_pretty(&credentials) {
                    Ok(json) => json,
                    Err(e) => {
                        let html = CLAUDE_OAUTH_ERROR_HTML
                            .replace("ERROR_PLACEHOLDER", &format!("序列化凭证失败: {}", e));
                        if let Some(sender) = tx.lock().await.take() {
                            let _ = sender.send(Err(format!("序列化凭证失败: {}", e)));
                        }
                        return Html(html);
                    }
                };

                if let Err(e) = std::fs::write(&creds_file_path, &creds_json) {
                    let html = CLAUDE_OAUTH_ERROR_HTML
                        .replace("ERROR_PLACEHOLDER", &format!("保存凭证失败: {}", e));
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(Err(format!("保存凭证失败: {}", e)));
                    }
                    return Html(html);
                }

                tracing::info!("[Claude OAuth] 凭证已保存到: {:?}", creds_file_path);

                // 发送成功结果
                let result = ClaudeOAuthResult {
                    credentials,
                    creds_file_path: creds_file_path.to_string_lossy().to_string(),
                };

                if let Some(sender) = tx.lock().await.take() {
                    let _ = sender.send(Ok(result));
                }

                // 返回成功页面
                let html = CLAUDE_OAUTH_SUCCESS_HTML.replace(
                    "EMAIL_PLACEHOLDER",
                    &email.unwrap_or_else(|| "未知邮箱".to_string()),
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
                    tracing::error!("[Claude OAuth] 服务器错误: {}", e);
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

/// 启动 Claude OAuth 登录流程（自动打开浏览器）
pub async fn start_claude_oauth_login() -> Result<ClaudeOAuthResult, Box<dyn Error + Send + Sync>> {
    let (auth_url, wait_future) = start_claude_oauth_server_and_get_url().await?;

    tracing::info!("[Claude OAuth] 打开浏览器进行授权: {}", auth_url);

    // 打开浏览器
    if let Err(e) = open::that(&auth_url) {
        tracing::warn!("[Claude OAuth] 无法打开浏览器: {}. 请手动打开 URL.", e);
    }

    // 等待回调
    wait_future.await
}
