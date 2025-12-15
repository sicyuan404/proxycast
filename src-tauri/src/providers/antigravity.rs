//! Antigravity Provider - Google 内部 Gemini 3 Pro 接口
//!
//! 支持 Gemini 3 Pro 等高级模型，通过 Google 内部 API 访问。

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;
use uuid::Uuid;

// Constants
const ANTIGRAVITY_BASE_URL_DAILY: &str = "https://daily-cloudcode-pa.sandbox.googleapis.com";
const ANTIGRAVITY_BASE_URL_AUTOPUSH: &str = "https://autopush-cloudcode-pa.sandbox.googleapis.com";
const ANTIGRAVITY_API_VERSION: &str = "v1internal";
const CREDENTIALS_DIR: &str = ".antigravity";
const CREDENTIALS_FILE: &str = "oauth_creds.json";

// OAuth credentials - 与 Antigravity CLI 相同
const OAUTH_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const OAUTH_CLIENT_SECRET: &str = "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";

// Token 刷新提前量（秒）
const REFRESH_SKEW: i64 = 3000;

/// Antigravity 支持的模型列表
pub const ANTIGRAVITY_MODELS: &[&str] = &[
    "gemini-3-pro-preview",
    "gemini-3-pro-image-preview",
    "gemini-2.5-computer-use-preview-10-2025",
    "gemini-claude-sonnet-4-5",
    "gemini-claude-sonnet-4-5-thinking",
];

/// 模型别名映射（用户友好名称 -> 内部名称）
fn alias_to_model_name(model: &str) -> &str {
    match model {
        "gemini-2.5-computer-use-preview-10-2025" => "rev19-uic3-1p",
        "gemini-3-pro-image-preview" => "gemini-3-pro-image",
        "gemini-3-pro-preview" => "gemini-3-pro-high",
        "gemini-claude-sonnet-4-5" => "claude-sonnet-4-5",
        "gemini-claude-sonnet-4-5-thinking" => "claude-sonnet-4-5-thinking",
        _ => model,
    }
}

/// 内部模型名称 -> 用户友好名称
#[allow(dead_code)]
fn model_name_to_alias(model: &str) -> &str {
    match model {
        "rev19-uic3-1p" => "gemini-2.5-computer-use-preview-10-2025",
        "gemini-3-pro-image" => "gemini-3-pro-image-preview",
        "gemini-3-pro-high" => "gemini-3-pro-preview",
        "claude-sonnet-4-5" => "gemini-claude-sonnet-4-5",
        "claude-sonnet-4-5-thinking" => "gemini-claude-sonnet-4-5-thinking",
        _ => model,
    }
}

/// 生成随机请求 ID
fn generate_request_id() -> String {
    format!("agent-{}", Uuid::new_v4())
}

/// 生成随机会话 ID
fn generate_session_id() -> String {
    // 使用 UUID 的一部分作为随机数
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    let n: u64 = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]) % 9_000_000_000_000_000_000;
    format!("-{}", n)
}

/// 生成随机项目 ID
fn generate_project_id() -> String {
    let adjectives = ["useful", "bright", "swift", "calm", "bold"];
    let nouns = ["fuze", "wave", "spark", "flow", "core"];
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    let adj = adjectives[(bytes[0] as usize) % adjectives.len()];
    let noun = nouns[(bytes[1] as usize) % nouns.len()];
    let random_part: String = uuid.to_string()[..5].to_lowercase();
    format!("{}-{}-{}", adj, noun, random_part)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityCredentials {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub expiry_date: Option<i64>,
    pub scope: Option<String>,
}

impl Default for AntigravityCredentials {
    fn default() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
            token_type: Some("Bearer".to_string()),
            expiry_date: None,
            scope: None,
        }
    }
}

/// Antigravity Provider
pub struct AntigravityProvider {
    pub credentials: AntigravityCredentials,
    pub project_id: Option<String>,
    pub client: Client,
    pub base_urls: Vec<String>,
    pub available_models: Vec<String>,
}

impl Default for AntigravityProvider {
    fn default() -> Self {
        Self {
            credentials: AntigravityCredentials::default(),
            project_id: None,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_urls: vec![
                ANTIGRAVITY_BASE_URL_DAILY.to_string(),
                ANTIGRAVITY_BASE_URL_AUTOPUSH.to_string(),
            ],
            available_models: ANTIGRAVITY_MODELS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl AntigravityProvider {
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
            let creds: AntigravityCredentials = serde_json::from_str(&content)?;
            self.credentials = creds;
        }

        Ok(())
    }

    pub async fn load_credentials_from_path(
        &mut self,
        path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let content = tokio::fs::read_to_string(path).await?;
        let creds: AntigravityCredentials = serde_json::from_str(&content)?;
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

    pub fn is_token_valid(&self) -> bool {
        if self.credentials.access_token.is_none() {
            return false;
        }
        if let Some(expiry) = self.credentials.expiry_date {
            let now = chrono::Utc::now().timestamp_millis();
            // Token valid if more than 5 minutes until expiry
            return expiry > now + 300_000;
        }
        true
    }

    pub fn is_token_expiring_soon(&self) -> bool {
        if let Some(expiry) = self.credentials.expiry_date {
            let now = chrono::Utc::now().timestamp_millis();
            let refresh_skew_ms = REFRESH_SKEW * 1000;
            return expiry <= now + refresh_skew_ms;
        }
        true
    }

    pub async fn refresh_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let refresh_token = self
            .credentials
            .refresh_token
            .as_ref()
            .ok_or("No refresh token available")?;

        let params = [
            ("client_id", OAUTH_CLIENT_ID),
            ("client_secret", OAUTH_CLIENT_SECRET),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];

        let resp = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Token refresh failed: {status} - {body}").into());
        }

        let data: serde_json::Value = resp.json().await?;

        let new_token = data["access_token"]
            .as_str()
            .ok_or("No access token in response")?;

        self.credentials.access_token = Some(new_token.to_string());

        if let Some(expires_in) = data["expires_in"].as_i64() {
            self.credentials.expiry_date =
                Some(chrono::Utc::now().timestamp_millis() + expires_in * 1000);
        }

        // 如果返回了新的 refresh_token，也更新它
        if let Some(new_refresh) = data["refresh_token"].as_str() {
            self.credentials.refresh_token = Some(new_refresh.to_string());
        }

        // Save refreshed credentials
        self.save_credentials().await?;

        Ok(new_token.to_string())
    }

    /// 调用 Antigravity API
    async fn call_api_internal(
        &self,
        base_url: &str,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let token = self
            .credentials
            .access_token
            .as_ref()
            .ok_or("No access token")?;

        let url = format!("{}/{ANTIGRAVITY_API_VERSION}:{method}", base_url);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("User-Agent", "antigravity/1.11.5 windows/amd64")
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

    /// 调用 API，支持多环境降级
    pub async fn call_api(
        &self,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;

        for base_url in &self.base_urls {
            match self.call_api_internal(base_url, method, body).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    tracing::warn!("[Antigravity] Failed on {}: {}", base_url, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "All Antigravity base URLs failed".into()))
    }

    /// 发现项目 ID
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
            // 生成一个随机项目 ID 作为后备
            let fallback = generate_project_id();
            self.project_id = Some(fallback.clone());
            return Ok(fallback);
        }

        self.project_id = Some(project_id.clone());
        Ok(project_id)
    }

    /// 获取可用模型列表
    pub async fn fetch_available_models(
        &mut self,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let body = serde_json::json!({});

        match self.call_api("fetchAvailableModels", &body).await {
            Ok(resp) => {
                if let Some(models) = resp["models"].as_object() {
                    self.available_models = models
                        .keys()
                        .filter_map(|name| {
                            let alias = model_name_to_alias(name);
                            if alias.is_empty() {
                                None
                            } else {
                                Some(alias.to_string())
                            }
                        })
                        .collect();
                }
            }
            Err(e) => {
                tracing::warn!(
                    "[Antigravity] Failed to fetch models: {}, using defaults",
                    e
                );
            }
        }

        Ok(self.available_models.clone())
    }

    /// 生成内容（非流式）
    pub async fn generate_content(
        &self,
        model: &str,
        request_body: &serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let project_id = self.project_id.clone().unwrap_or_else(generate_project_id);
        let actual_model = alias_to_model_name(model);

        let payload = self.build_antigravity_request(actual_model, &project_id, request_body);

        let resp = self.call_api("generateContent", &payload).await?;

        // 转换为 Gemini 格式响应
        Ok(self.to_gemini_response(&resp))
    }

    /// 构建 Antigravity 请求
    fn build_antigravity_request(
        &self,
        model: &str,
        project_id: &str,
        request_body: &serde_json::Value,
    ) -> serde_json::Value {
        let mut payload = request_body.clone();

        // 设置基本字段
        payload["model"] = serde_json::json!(model);
        payload["userAgent"] = serde_json::json!("antigravity");
        payload["project"] = serde_json::json!(project_id);
        payload["requestId"] = serde_json::json!(generate_request_id());

        // 确保 request 对象存在
        if payload.get("request").is_none() {
            payload["request"] = serde_json::json!({});
        }

        // 设置会话 ID
        payload["request"]["sessionId"] = serde_json::json!(generate_session_id());

        // 删除安全设置
        if let Some(request) = payload.get_mut("request") {
            if let Some(obj) = request.as_object_mut() {
                obj.remove("safetySettings");
            }
        }

        payload
    }

    /// 转换为 Gemini 格式响应
    fn to_gemini_response(&self, antigravity_resp: &serde_json::Value) -> serde_json::Value {
        let mut response = serde_json::json!({});

        if let Some(candidates) = antigravity_resp.get("candidates") {
            response["candidates"] = candidates.clone();
        }

        if let Some(usage) = antigravity_resp.get("usageMetadata") {
            response["usageMetadata"] = usage.clone();
        }

        if let Some(feedback) = antigravity_resp.get("promptFeedback") {
            response["promptFeedback"] = feedback.clone();
        }

        response
    }

    /// 检查模型是否支持
    pub fn supports_model(&self, model: &str) -> bool {
        self.available_models.iter().any(|m| m == model)
    }
}
