//! Provider Pool 管理服务
//!
//! 提供凭证池的选择、健康检测、负载均衡等功能。

use crate::database::dao::provider_pool::ProviderPoolDao;
use crate::database::DbConnection;
use crate::models::provider_pool_model::{
    get_default_check_model, get_oauth_creds_path, CredentialData, CredentialDisplay,
    HealthCheckResult, OAuthStatus, PoolProviderType, PoolStats, ProviderCredential,
    ProviderPoolOverview,
};
use crate::models::route_model::RouteInfo;
use crate::providers::kiro::KiroProvider;
use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// 凭证池管理服务
pub struct ProviderPoolService {
    /// HTTP 客户端（用于健康检测）
    client: Client,
    /// 轮询索引（按 provider_type 和可选的 model 分组）
    round_robin_index: std::sync::RwLock<HashMap<String, AtomicUsize>>,
    /// 最大错误次数（超过后标记为不健康）
    max_error_count: u32,
    /// 健康检查超时时间
    health_check_timeout: Duration,
}

impl Default for ProviderPoolService {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderPoolService {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            round_robin_index: std::sync::RwLock::new(HashMap::new()),
            max_error_count: 3,
            health_check_timeout: Duration::from_secs(30),
        }
    }

    /// 获取所有凭证概览
    pub fn get_overview(&self, db: &DbConnection) -> Result<Vec<ProviderPoolOverview>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let grouped = ProviderPoolDao::get_grouped(&conn).map_err(|e| e.to_string())?;

        let mut overview = Vec::new();
        for (provider_type, mut credentials) in grouped {
            // 为每个凭证加载 token 缓存
            for cred in &mut credentials {
                cred.cached_token = ProviderPoolDao::get_token_cache(&conn, &cred.uuid)
                    .ok()
                    .flatten();
            }

            let stats = PoolStats::from_credentials(&credentials);
            let displays: Vec<CredentialDisplay> = credentials.iter().map(|c| c.into()).collect();

            overview.push(ProviderPoolOverview {
                provider_type: provider_type.to_string(),
                stats,
                credentials: displays,
            });
        }

        // 按 provider_type 排序
        overview.sort_by(|a, b| a.provider_type.cmp(&b.provider_type));
        Ok(overview)
    }

    /// 获取指定类型的凭证列表
    pub fn get_by_type(
        &self,
        db: &DbConnection,
        provider_type: &str,
    ) -> Result<Vec<CredentialDisplay>, String> {
        let pt: PoolProviderType = provider_type.parse().map_err(|e: String| e)?;
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut credentials =
            ProviderPoolDao::get_by_type(&conn, &pt).map_err(|e| e.to_string())?;

        // 为每个凭证加载 token 缓存
        for cred in &mut credentials {
            cred.cached_token = ProviderPoolDao::get_token_cache(&conn, &cred.uuid)
                .ok()
                .flatten();
        }

        Ok(credentials.iter().map(|c| c.into()).collect())
    }

    /// 添加凭证
    pub fn add_credential(
        &self,
        db: &DbConnection,
        provider_type: &str,
        credential: CredentialData,
        name: Option<String>,
        check_health: Option<bool>,
        check_model_name: Option<String>,
    ) -> Result<ProviderCredential, String> {
        let pt: PoolProviderType = provider_type.parse().map_err(|e: String| e)?;

        let mut cred = ProviderCredential::new(pt, credential);
        cred.name = name;
        cred.check_health = check_health.unwrap_or(true);
        cred.check_model_name = check_model_name;

        let conn = db.lock().map_err(|e| e.to_string())?;
        ProviderPoolDao::insert(&conn, &cred).map_err(|e| e.to_string())?;

        Ok(cred)
    }

    /// 更新凭证
    pub fn update_credential(
        &self,
        db: &DbConnection,
        uuid: &str,
        name: Option<String>,
        is_disabled: Option<bool>,
        check_health: Option<bool>,
        check_model_name: Option<String>,
        not_supported_models: Option<Vec<String>>,
    ) -> Result<ProviderCredential, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut cred = ProviderPoolDao::get_by_uuid(&conn, uuid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Credential not found: {}", uuid))?;

        if let Some(n) = name {
            cred.name = Some(n);
        }
        if let Some(d) = is_disabled {
            cred.is_disabled = d;
        }
        if let Some(c) = check_health {
            cred.check_health = c;
        }
        if let Some(m) = check_model_name {
            cred.check_model_name = Some(m);
        }
        if let Some(models) = not_supported_models {
            cred.not_supported_models = models;
        }
        cred.updated_at = Utc::now();

        ProviderPoolDao::update(&conn, &cred).map_err(|e| e.to_string())?;
        Ok(cred)
    }

    /// 删除凭证
    pub fn delete_credential(&self, db: &DbConnection, uuid: &str) -> Result<bool, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ProviderPoolDao::delete(&conn, uuid).map_err(|e| e.to_string())
    }

    /// 选择一个可用的凭证（轮询负载均衡）
    pub fn select_credential(
        &self,
        db: &DbConnection,
        provider_type: &str,
        model: Option<&str>,
    ) -> Result<Option<ProviderCredential>, String> {
        let pt: PoolProviderType = provider_type.parse().map_err(|e: String| e)?;
        let conn = db.lock().map_err(|e| e.to_string())?;
        let credentials = ProviderPoolDao::get_by_type(&conn, &pt).map_err(|e| e.to_string())?;
        drop(conn);

        // 过滤可用的凭证
        let mut available: Vec<_> = credentials
            .into_iter()
            .filter(|c| c.is_available())
            .collect();

        // 如果指定了模型，进一步过滤支持该模型的凭证
        if let Some(m) = model {
            available.retain(|c| c.supports_model(m));
        }

        if available.is_empty() {
            return Ok(None);
        }

        // 轮询选择
        let index_key = match model {
            Some(m) => format!("{}:{}", provider_type, m),
            None => provider_type.to_string(),
        };

        let index = {
            let indices = self.round_robin_index.read().unwrap();
            indices
                .get(&index_key)
                .map(|i| i.load(Ordering::SeqCst))
                .unwrap_or(0)
        };

        let selected_index = index % available.len();
        let selected = available.remove(selected_index);

        // 更新轮询索引
        {
            let mut indices = self.round_robin_index.write().unwrap();
            let counter = indices
                .entry(index_key)
                .or_insert_with(|| AtomicUsize::new(0));
            counter.store((index + 1) % usize::MAX, Ordering::SeqCst);
        }

        Ok(Some(selected))
    }

    /// 记录凭证使用
    pub fn record_usage(&self, db: &DbConnection, uuid: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let cred = ProviderPoolDao::get_by_uuid(&conn, uuid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Credential not found: {}", uuid))?;

        ProviderPoolDao::update_usage(&conn, uuid, cred.usage_count + 1, Utc::now())
            .map_err(|e| e.to_string())
    }

    /// 标记凭证为健康
    pub fn mark_healthy(
        &self,
        db: &DbConnection,
        uuid: &str,
        check_model: Option<&str>,
    ) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ProviderPoolDao::update_health_status(
            &conn,
            uuid,
            true,
            0,
            None,
            None,
            Some(Utc::now()),
            check_model,
        )
        .map_err(|e| e.to_string())
    }

    /// 标记凭证为不健康
    pub fn mark_unhealthy(
        &self,
        db: &DbConnection,
        uuid: &str,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let cred = ProviderPoolDao::get_by_uuid(&conn, uuid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Credential not found: {}", uuid))?;

        let new_error_count = cred.error_count + 1;
        let is_healthy = new_error_count < self.max_error_count;

        ProviderPoolDao::update_health_status(
            &conn,
            uuid,
            is_healthy,
            new_error_count,
            Some(Utc::now()),
            error_message,
            None,
            None,
        )
        .map_err(|e| e.to_string())
    }

    /// 重置凭证计数器
    pub fn reset_counters(&self, db: &DbConnection, uuid: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ProviderPoolDao::reset_counters(&conn, uuid).map_err(|e| e.to_string())
    }

    /// 重置指定类型的所有凭证健康状态
    pub fn reset_health_by_type(
        &self,
        db: &DbConnection,
        provider_type: &str,
    ) -> Result<usize, String> {
        let pt: PoolProviderType = provider_type.parse().map_err(|e: String| e)?;
        let conn = db.lock().map_err(|e| e.to_string())?;
        ProviderPoolDao::reset_health_by_type(&conn, &pt).map_err(|e| e.to_string())
    }

    /// 执行单个凭证的健康检查
    pub async fn check_credential_health(
        &self,
        db: &DbConnection,
        uuid: &str,
    ) -> Result<HealthCheckResult, String> {
        let cred = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            ProviderPoolDao::get_by_uuid(&conn, uuid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Credential not found: {}", uuid))?
        };

        let check_model = cred
            .check_model_name
            .clone()
            .unwrap_or_else(|| get_default_check_model(cred.provider_type).to_string());

        let start = std::time::Instant::now();
        let result = self
            .perform_health_check(&cred.credential, &check_model)
            .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(_) => {
                self.mark_healthy(db, uuid, Some(&check_model))?;
                Ok(HealthCheckResult {
                    uuid: uuid.to_string(),
                    success: true,
                    model: Some(check_model),
                    message: Some("Health check passed".to_string()),
                    duration_ms,
                })
            }
            Err(e) => {
                self.mark_unhealthy(db, uuid, Some(&e))?;
                Ok(HealthCheckResult {
                    uuid: uuid.to_string(),
                    success: false,
                    model: Some(check_model),
                    message: Some(e),
                    duration_ms,
                })
            }
        }
    }

    /// 执行指定类型的所有凭证健康检查
    pub async fn check_type_health(
        &self,
        db: &DbConnection,
        provider_type: &str,
    ) -> Result<Vec<HealthCheckResult>, String> {
        let pt: PoolProviderType = provider_type.parse().map_err(|e: String| e)?;
        let credentials = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            ProviderPoolDao::get_by_type(&conn, &pt).map_err(|e| e.to_string())?
        };

        let mut results = Vec::new();
        for cred in credentials {
            if cred.is_disabled || !cred.check_health {
                continue;
            }

            let result = self.check_credential_health(db, &cred.uuid).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// 执行实际的健康检查请求
    async fn perform_health_check(
        &self,
        credential: &CredentialData,
        model: &str,
    ) -> Result<(), String> {
        // 根据凭证类型构建测试请求
        match credential {
            CredentialData::KiroOAuth { creds_file_path } => {
                self.check_kiro_health(creds_file_path, model).await
            }
            CredentialData::GeminiOAuth {
                creds_file_path,
                project_id,
            } => {
                self.check_gemini_health(creds_file_path, project_id.as_deref(), model)
                    .await
            }
            CredentialData::QwenOAuth { creds_file_path } => {
                self.check_qwen_health(creds_file_path, model).await
            }
            CredentialData::AntigravityOAuth {
                creds_file_path,
                project_id,
            } => {
                self.check_antigravity_health(creds_file_path, project_id.as_deref(), model)
                    .await
            }
            CredentialData::OpenAIKey { api_key, base_url } => {
                self.check_openai_health(api_key, base_url.as_deref(), model)
                    .await
            }
            CredentialData::ClaudeKey { api_key, base_url } => {
                self.check_claude_health(api_key, base_url.as_deref(), model)
                    .await
            }
        }
    }

    /// 将技术错误转换为用户友好的错误信息
    fn format_user_friendly_error(&self, error: &str, provider_type: &str) -> String {
        if error.contains("No client_id") {
            format!("OAuth 配置不完整：缺少必要的认证参数。\n💡 解决方案：\n1. 检查 {} OAuth 凭证配置是否完整\n2. 如问题持续，建议删除后重新添加此凭证\n3. 或者切换到其他可用的凭证", provider_type)
        } else if error.contains("请求失败") || error.contains("error sending request") {
            format!("网络连接失败，无法访问 {} 服务。\n💡 解决方案：\n1. 检查网络连接是否正常\n2. 确认防火墙或代理设置\n3. 稍后重试，如问题持续请联系网络管理员", provider_type)
        } else if error.contains("HTTP 401") || error.contains("HTTP 403") {
            format!("{} 认证失败，凭证可能已过期或无效。\n💡 解决方案：\n1. 点击\"刷新\"按钮尝试更新 Token\n2. 如刷新失败，请删除后重新添加此凭证\n3. 检查账户权限是否正常", provider_type)
        } else if error.contains("HTTP 429") {
            format!("{} 请求频率过高，已被限流。\n💡 解决方案：\n1. 稍等几分钟后再次尝试\n2. 考虑添加更多凭证分散负载", provider_type)
        } else if error.contains("HTTP 500")
            || error.contains("HTTP 502")
            || error.contains("HTTP 503")
        {
            format!("{} 服务暂时不可用。\n💡 解决方案：\n1. 这通常是服务提供方的临时问题\n2. 请稍后重试\n3. 如问题持续，可尝试其他凭证", provider_type)
        } else if error.contains("读取凭证文件失败") || error.contains("解析凭证失败")
        {
            format!("凭证文件损坏或不可读。\n💡 解决方案：\n1. 凭证文件可能已损坏\n2. 建议删除此凭证后重新添加\n3. 确保文件权限正确且格式为有效的 JSON")
        } else {
            // 对于其他未识别的错误，提供通用建议
            format!("操作失败：{}\n💡 建议：\n1. 检查网络连接和凭证状态\n2. 尝试刷新 Token 或重新添加凭证\n3. 如问题持续，请联系技术支持", error)
        }
    }

    // Kiro OAuth 健康检查
    async fn check_kiro_health(&self, creds_path: &str, model: &str) -> Result<(), String> {
        tracing::debug!("[KIRO HEALTH] 开始健康检查，凭证路径: {}", creds_path);

        // 使用 KiroProvider 加载凭证（包括 clientIdHash 文件）
        let mut provider = KiroProvider::new();
        provider
            .load_credentials_from_path(creds_path)
            .await
            .map_err(|e| {
                self.format_user_friendly_error(&format!("加载凭证失败: {}", e), "Kiro")
            })?;

        let access_token = provider
            .credentials
            .access_token
            .as_ref()
            .ok_or_else(|| "凭证中缺少 access_token".to_string())?;

        let health_check_url = provider.get_health_check_url();

        // 获取 modelId 映射
        let model_id = match model {
            "claude-opus-4-5" | "claude-opus-4-5-20251101" => "claude-opus-4.5",
            "claude-haiku-4-5" => "claude-haiku-4.5",
            "claude-sonnet-4-5" | "claude-sonnet-4-5-20250929" => "CLAUDE_SONNET_4_5_20250929_V1_0",
            "claude-sonnet-4-20250514" => "CLAUDE_SONNET_4_20250514_V1_0",
            "claude-3-7-sonnet-20250219" => "CLAUDE_3_7_SONNET_20250219_V1_0",
            _ => "claude-haiku-4.5", // 默认使用 haiku
        };

        tracing::debug!("[KIRO HEALTH] 健康检查 URL: {}", health_check_url);
        tracing::debug!("[KIRO HEALTH] 使用模型: {} -> {}", model, model_id);

        // 构建与实际 API 调用相同格式的测试请求（参考 AIClient-2-API 实现）
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let mut request_body = serde_json::json!({
            "conversationState": {
                "chatTriggerType": "MANUAL",
                "conversationId": conversation_id,
                "currentMessage": {
                    "userInputMessage": {
                        "content": "Say OK",
                        "modelId": model_id,
                        "origin": "AI_EDITOR"
                    }
                }
            }
        });

        // 如果是 social 认证方式，需要添加 profileArn
        if provider.credentials.auth_method.as_deref() == Some("social") {
            if let Some(profile_arn) = &provider.credentials.profile_arn {
                request_body["profileArn"] = serde_json::json!(profile_arn);
            }
        }

        tracing::debug!("[KIRO HEALTH] 请求体已构建");

        let response = self
            .client
            .post(&health_check_url)
            .bearer_auth(access_token)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("x-amz-user-agent", "aws-sdk-js/1.0.7 KiroIDE-0.1.25")
            .header("user-agent", "aws-sdk-js/1.0.7 ua/2.1 os/macos#14.0 lang/js md/nodejs#20.16.0 api/codewhispererstreaming#1.0.7 m/E KiroIDE-0.1.25")
            .header("amz-sdk-invocation-id", uuid::Uuid::new_v4().to_string())
            .header("amz-sdk-request", "attempt=1; max=1")
            .header("x-amzn-kiro-agent-mode", "vibe")
            .json(&request_body)
            .timeout(self.health_check_timeout)
            .send()
            .await
            .map_err(|e| self.format_user_friendly_error(&format!("请求失败: {}", e), "Kiro"))?;

        let status = response.status();
        tracing::info!("[KIRO HEALTH] 响应状态: {}", status);

        if status.is_success() {
            tracing::info!("[KIRO HEALTH] 健康检查成功");
            Ok(())
        } else {
            let body_text = response.text().await.unwrap_or_default();
            tracing::warn!("[KIRO HEALTH] 健康检查失败: {} - {}", status, body_text);
            let error_msg = format!("HTTP {}: {}", status, body_text);
            Err(self.format_user_friendly_error(&error_msg, "Kiro"))
        }
    }

    // Gemini OAuth 健康检查
    async fn check_gemini_health(
        &self,
        creds_path: &str,
        _project_id: Option<&str>,
        model: &str,
    ) -> Result<(), String> {
        let creds_content =
            std::fs::read_to_string(creds_path).map_err(|e| format!("读取凭证文件失败: {}", e))?;
        let creds: serde_json::Value =
            serde_json::from_str(&creds_content).map_err(|e| format!("解析凭证失败: {}", e))?;

        let access_token = creds["access_token"]
            .as_str()
            .ok_or_else(|| "凭证中缺少 access_token".to_string())?;

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            model
        );

        let request_body = serde_json::json!({
            "contents": [{
                "parts": [{"text": "Say OK"}]
            }],
            "generationConfig": {
                "maxOutputTokens": 10
            }
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .json(&request_body)
            .timeout(self.health_check_timeout)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()))
        }
    }

    // Qwen OAuth 健康检查
    async fn check_qwen_health(&self, creds_path: &str, model: &str) -> Result<(), String> {
        let creds_content =
            std::fs::read_to_string(creds_path).map_err(|e| format!("读取凭证文件失败: {}", e))?;
        let creds: serde_json::Value =
            serde_json::from_str(&creds_content).map_err(|e| format!("解析凭证失败: {}", e))?;

        let access_token = creds["access_token"]
            .as_str()
            .ok_or_else(|| "凭证中缺少 access_token".to_string())?;

        let request_body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "Say OK"}],
            "max_tokens": 10
        });

        let response = self
            .client
            .post("https://chat.qwen.ai/api/v1/chat/completions")
            .bearer_auth(access_token)
            .json(&request_body)
            .timeout(self.health_check_timeout)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()))
        }
    }

    // Antigravity OAuth 健康检查
    async fn check_antigravity_health(
        &self,
        creds_path: &str,
        _project_id: Option<&str>,
        _model: &str,
    ) -> Result<(), String> {
        let creds_content =
            std::fs::read_to_string(creds_path).map_err(|e| format!("读取凭证文件失败: {}", e))?;
        let creds: serde_json::Value =
            serde_json::from_str(&creds_content).map_err(|e| format!("解析凭证失败: {}", e))?;

        let access_token = creds["access_token"]
            .as_str()
            .ok_or_else(|| "凭证中缺少 access_token".to_string())?;

        // 使用 fetchAvailableModels 作为健康检查
        let url =
            "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:fetchAvailableModels";

        let response = self
            .client
            .post(url)
            .bearer_auth(access_token)
            .header("User-Agent", "antigravity/1.11.5 windows/amd64")
            .json(&serde_json::json!({}))
            .timeout(self.health_check_timeout)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()))
        }
    }

    // OpenAI API 健康检查
    async fn check_openai_health(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        model: &str,
    ) -> Result<(), String> {
        let url = format!(
            "{}/chat/completions",
            base_url.unwrap_or("https://api.openai.com/v1")
        );

        let request_body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "Say OK"}],
            "max_tokens": 10
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .json(&request_body)
            .timeout(self.health_check_timeout)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()))
        }
    }

    // Claude API 健康检查
    async fn check_claude_health(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        model: &str,
    ) -> Result<(), String> {
        let url = format!(
            "{}/messages",
            base_url.unwrap_or("https://api.anthropic.com/v1")
        );

        let request_body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "Say OK"}],
            "max_tokens": 10
        });

        let response = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request_body)
            .timeout(self.health_check_timeout)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()))
        }
    }

    /// 根据名称获取凭证
    pub fn get_by_name(
        &self,
        db: &DbConnection,
        name: &str,
    ) -> Result<Option<ProviderCredential>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ProviderPoolDao::get_by_name(&conn, name).map_err(|e| e.to_string())
    }

    /// 根据 UUID 获取凭证
    pub fn get_by_uuid(
        &self,
        db: &DbConnection,
        uuid: &str,
    ) -> Result<Option<ProviderCredential>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ProviderPoolDao::get_by_uuid(&conn, uuid).map_err(|e| e.to_string())
    }

    /// 获取所有可用的路由端点
    pub fn get_available_routes(
        &self,
        db: &DbConnection,
        base_url: &str,
    ) -> Result<Vec<RouteInfo>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let grouped = ProviderPoolDao::get_grouped(&conn).map_err(|e| e.to_string())?;
        drop(conn);

        let mut routes = Vec::new();

        // 为每种 Provider 类型创建路由
        for (provider_type, credentials) in &grouped {
            let available: Vec<_> = credentials.iter().filter(|c| c.is_available()).collect();
            if available.is_empty() {
                continue;
            }

            // Provider 类型路由 (轮询)
            let mut route = RouteInfo::new(provider_type.to_string(), provider_type.to_string());
            route.credential_count = available.len();
            route.add_endpoint(base_url, "claude");
            route.add_endpoint(base_url, "openai");
            route.tags.push("轮询".to_string());
            routes.push(route);
        }

        // 为每个命名凭证创建路由
        for (_provider_type, credentials) in &grouped {
            for cred in credentials {
                if let Some(name) = &cred.name {
                    if cred.is_available() {
                        let mut route =
                            RouteInfo::new(name.clone(), cred.provider_type.to_string());
                        route.credential_count = 1;
                        route.enabled = !cred.is_disabled;
                        route.add_endpoint(base_url, "claude");
                        route.add_endpoint(base_url, "openai");
                        route.tags.push("指定凭证".to_string());
                        routes.push(route);
                    }
                }
            }
        }

        Ok(routes)
    }

    /// 获取 OAuth 凭证状态
    pub fn get_oauth_status(
        &self,
        creds_path: &str,
        provider_type: &str,
    ) -> Result<OAuthStatus, String> {
        let content =
            std::fs::read_to_string(creds_path).map_err(|e| format!("读取凭证文件失败: {}", e))?;
        let creds: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("解析凭证文件失败: {}", e))?;

        let has_access_token = creds
            .get("accessToken")
            .or_else(|| creds.get("access_token"))
            .map(|v| v.as_str().is_some())
            .unwrap_or(false);

        let has_refresh_token = creds
            .get("refreshToken")
            .or_else(|| creds.get("refresh_token"))
            .map(|v| v.as_str().is_some())
            .unwrap_or(false);

        // 检查 token 是否有效（根据 expiry_date 判断）
        let (is_token_valid, expiry_info) = match provider_type {
            "kiro" => {
                let expires_at = creds
                    .get("expiresAt")
                    .or_else(|| creds.get("expires_at"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                // Kiro 没有标准的过期时间字段，假设有 access_token 就有效
                (has_access_token, expires_at)
            }
            "gemini" | "qwen" => {
                let expiry = creds.get("expiry_date").and_then(|v| v.as_i64());
                if let Some(exp) = expiry {
                    let now = chrono::Utc::now().timestamp();
                    let is_valid = exp > now;
                    let expiry_str = chrono::DateTime::from_timestamp(exp, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_else(|| exp.to_string());
                    (is_valid, Some(expiry_str))
                } else {
                    (has_access_token, None)
                }
            }
            _ => (has_access_token, None),
        };

        Ok(OAuthStatus {
            has_access_token,
            has_refresh_token,
            is_token_valid,
            expiry_info,
            creds_path: creds_path.to_string(),
        })
    }

    /// 刷新 OAuth Token (Kiro)
    pub async fn refresh_kiro_token(&self, creds_path: &str) -> Result<String, String> {
        let mut provider = crate::providers::kiro::KiroProvider::new();
        provider
            .load_credentials_from_path(creds_path)
            .await
            .map_err(|e| {
                self.format_user_friendly_error(&format!("加载凭证失败: {}", e), "Kiro")
            })?;
        provider.refresh_token().await.map_err(|e| {
            self.format_user_friendly_error(&format!("刷新 Token 失败: {}", e), "Kiro")
        })
    }

    /// 刷新 OAuth Token (Gemini)
    pub async fn refresh_gemini_token(&self, creds_path: &str) -> Result<String, String> {
        let mut provider = crate::providers::gemini::GeminiProvider::new();
        provider
            .load_credentials_from_path(creds_path)
            .await
            .map_err(|e| format!("加载凭证失败: {}", e))?;
        provider
            .refresh_token()
            .await
            .map_err(|e| format!("刷新 Token 失败: {}", e))
    }

    /// 刷新 OAuth Token (Qwen)
    pub async fn refresh_qwen_token(&self, creds_path: &str) -> Result<String, String> {
        let mut provider = crate::providers::qwen::QwenProvider::new();
        provider
            .load_credentials_from_path(creds_path)
            .await
            .map_err(|e| format!("加载凭证失败: {}", e))?;
        provider
            .refresh_token()
            .await
            .map_err(|e| format!("刷新 Token 失败: {}", e))
    }

    /// 刷新 OAuth Token (Antigravity)
    pub async fn refresh_antigravity_token(&self, creds_path: &str) -> Result<String, String> {
        let mut provider = crate::providers::antigravity::AntigravityProvider::new();
        provider
            .load_credentials_from_path(creds_path)
            .await
            .map_err(|e| format!("加载凭证失败: {}", e))?;
        provider
            .refresh_token()
            .await
            .map_err(|e| format!("刷新 Token 失败: {}", e))
    }

    /// 刷新凭证池中指定凭证的 OAuth Token
    pub async fn refresh_credential_token(
        &self,
        db: &DbConnection,
        uuid: &str,
    ) -> Result<String, String> {
        let cred = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            ProviderPoolDao::get_by_uuid(&conn, uuid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Credential not found: {}", uuid))?
        };

        match &cred.credential {
            CredentialData::KiroOAuth { creds_file_path } => {
                self.refresh_kiro_token(creds_file_path).await
            }
            CredentialData::GeminiOAuth {
                creds_file_path, ..
            } => self.refresh_gemini_token(creds_file_path).await,
            CredentialData::QwenOAuth { creds_file_path } => {
                self.refresh_qwen_token(creds_file_path).await
            }
            CredentialData::AntigravityOAuth {
                creds_file_path, ..
            } => self.refresh_antigravity_token(creds_file_path).await,
            _ => Err("此凭证类型不支持 Token 刷新".to_string()),
        }
    }

    /// 获取凭证池中指定凭证的 OAuth 状态
    pub fn get_credential_oauth_status(
        &self,
        db: &DbConnection,
        uuid: &str,
    ) -> Result<OAuthStatus, String> {
        let cred = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            ProviderPoolDao::get_by_uuid(&conn, uuid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Credential not found: {}", uuid))?
        };

        let creds_path = get_oauth_creds_path(&cred.credential)
            .ok_or_else(|| "此凭证类型不是 OAuth 凭证".to_string())?;

        self.get_oauth_status(&creds_path, &cred.provider_type.to_string())
    }
}
