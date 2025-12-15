//! Kiro/CodeWhisperer Provider
use crate::converter::openai_to_cw::convert_openai_to_codewhisperer;
use crate::models::openai::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

/// 生成设备指纹 (MAC 地址的 SHA256)
fn get_device_fingerprint() -> String {
    use std::process::Command;

    // 尝试获取 MAC 地址
    let mac = if cfg!(target_os = "macos") {
        Command::new("ifconfig")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| {
                s.lines()
                    .find(|l| l.contains("ether "))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .map(|s| s.to_string())
            })
    } else {
        None
    };

    let mac = mac.unwrap_or_else(|| "00:00:00:00:00:00".to_string());

    // SHA256 hash
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    mac.hash(&mut hasher);
    format!("{:016x}{:016x}", hasher.finish(), hasher.finish())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroCredentials {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub profile_arn: Option<String>,
    pub expires_at: Option<String>,
    pub region: Option<String>,
    pub auth_method: Option<String>,
    pub client_id_hash: Option<String>,
}

impl Default for KiroCredentials {
    fn default() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
            client_id: None,
            client_secret: None,
            profile_arn: None,
            expires_at: None,
            region: Some("us-east-1".to_string()),
            auth_method: Some("social".to_string()),
            client_id_hash: None,
        }
    }
}

pub struct KiroProvider {
    pub credentials: KiroCredentials,
    pub client: Client,
    /// 当前加载的凭证文件路径
    pub creds_path: Option<PathBuf>,
}

impl Default for KiroProvider {
    fn default() -> Self {
        Self {
            credentials: KiroCredentials::default(),
            client: Client::new(),
            creds_path: None,
        }
    }
}

impl KiroProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_creds_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".aws")
            .join("sso")
            .join("cache")
            .join("kiro-auth-token.json")
    }

    pub async fn load_credentials(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = Self::default_creds_path();
        let dir = path.parent().ok_or("Invalid path: no parent directory")?;

        let mut merged = KiroCredentials::default();

        // 读取主凭证文件
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let content = tokio::fs::read_to_string(&path).await?;
            let creds: KiroCredentials = serde_json::from_str(&content)?;
            tracing::info!(
                "[KIRO] Main file loaded: has_access={}, has_refresh={}, has_client_id={}, auth_method={:?}",
                creds.access_token.is_some(),
                creds.refresh_token.is_some(),
                creds.client_id.is_some(),
                creds.auth_method
            );
            merge_credentials(&mut merged, &creds);
        }

        // 如果有 clientIdHash，尝试加载对应的 client_id 和 client_secret
        if let Some(hash) = &merged.client_id_hash {
            let hash_file_path = dir.join(format!("{}.json", hash));
            tracing::info!(
                "[KIRO] 检查 clientIdHash 文件: {}",
                hash_file_path.display()
            );
            if tokio::fs::try_exists(&hash_file_path)
                .await
                .unwrap_or(false)
            {
                if let Ok(content) = tokio::fs::read_to_string(&hash_file_path).await {
                    if let Ok(creds) = serde_json::from_str::<KiroCredentials>(&content) {
                        tracing::info!(
                            "[KIRO] Hash file {:?}: has_client_id={}, has_client_secret={}",
                            hash_file_path.file_name(),
                            creds.client_id.is_some(),
                            creds.client_secret.is_some()
                        );
                        merge_credentials(&mut merged, &creds);
                    } else {
                        tracing::error!(
                            "[KIRO] 无法解析 clientIdHash 文件: {}",
                            hash_file_path.display()
                        );
                    }
                } else {
                    tracing::error!(
                        "[KIRO] 无法读取 clientIdHash 文件: {}",
                        hash_file_path.display()
                    );
                }
            } else {
                tracing::warn!(
                    "[KIRO] clientIdHash {} 指向的文件不存在: {}",
                    hash,
                    hash_file_path.display()
                );
            }
        } else {
            tracing::info!("[KIRO] 没有 clientIdHash 字段");
        }

        // 读取目录中其他 JSON 文件
        if tokio::fs::try_exists(dir).await.unwrap_or(false) {
            let mut entries = tokio::fs::read_dir(dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let file_path = entry.path();
                if file_path.extension().map(|e| e == "json").unwrap_or(false) && file_path != path
                {
                    if let Ok(content) = tokio::fs::read_to_string(&file_path).await {
                        if let Ok(creds) = serde_json::from_str::<KiroCredentials>(&content) {
                            tracing::info!(
                                "[KIRO] Extra file {:?}: has_client_id={}, has_client_secret={}",
                                file_path.file_name(),
                                creds.client_id.is_some(),
                                creds.client_secret.is_some()
                            );
                            merge_credentials(&mut merged, &creds);
                        }
                    }
                }
            }
        }

        tracing::info!(
            "[KIRO] Final merged: has_access={}, has_refresh={}, has_client_id={}, has_client_secret={}, auth_method={:?}",
            merged.access_token.is_some(),
            merged.refresh_token.is_some(),
            merged.client_id.is_some(),
            merged.client_secret.is_some(),
            merged.auth_method
        );

        self.credentials = merged;
        self.creds_path = Some(path);

        // 加载完成后，智能检测并更新认证方式（如果需要）
        let detected_auth_method = self.detect_auth_method();
        if self.credentials.auth_method.as_deref().unwrap_or("social") != detected_auth_method {
            tracing::info!(
                "[KIRO] 加载后检测到需要调整认证方式为: {}",
                detected_auth_method
            );
            self.set_auth_method(&detected_auth_method);
        }

        Ok(())
    }

    /// 从指定路径加载凭证（包括 clientIdHash 文件和同目录的其他 JSON 文件）
    pub async fn load_credentials_from_path(
        &mut self,
        path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = std::path::PathBuf::from(path);
        let dir = path.parent().ok_or("Invalid path: no parent directory")?;

        let mut merged = KiroCredentials::default();

        // 读取主凭证文件
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let content = tokio::fs::read_to_string(&path).await?;
            let creds: KiroCredentials = serde_json::from_str(&content)?;
            tracing::info!(
                "[KIRO] Main file loaded from {:?}: has_access={}, has_refresh={}, has_client_id={}, auth_method={:?}, clientIdHash={:?}",
                path,
                creds.access_token.is_some(),
                creds.refresh_token.is_some(),
                creds.client_id.is_some(),
                creds.auth_method,
                creds.client_id_hash
            );
            merge_credentials(&mut merged, &creds);
        }

        // 如果有 clientIdHash，尝试从 ~/.aws/sso/cache/ 目录加载对应的 client_id 和 client_secret
        if let Some(hash) = &merged.client_id_hash {
            // clientIdHash 文件总是在 ~/.aws/sso/cache/ 目录中
            let aws_sso_cache_dir = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".aws")
                .join("sso")
                .join("cache");
            let hash_file_path = aws_sso_cache_dir.join(format!("{}.json", hash));

            tracing::debug!(
                "[KIRO] 检查 clientIdHash 文件: {}",
                hash_file_path.display()
            );

            if tokio::fs::try_exists(&hash_file_path)
                .await
                .unwrap_or(false)
            {
                if let Ok(content) = tokio::fs::read_to_string(&hash_file_path).await {
                    // 使用 serde_json::Value 来更灵活地解析，因为 hash 文件可能包含额外字段
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&content) {
                        // 直接提取 clientId 和 clientSecret
                        let client_id = json_value
                            .get("clientId")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let client_secret = json_value
                            .get("clientSecret")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        tracing::debug!(
                            "[KIRO] Hash file {:?}: has_client_id={}, has_client_secret={}",
                            hash_file_path.file_name(),
                            client_id.is_some(),
                            client_secret.is_some()
                        );

                        if client_id.is_some() {
                            merged.client_id = client_id;
                        }
                        if client_secret.is_some() {
                            merged.client_secret = client_secret;
                        }
                    } else {
                        tracing::warn!(
                            "[KIRO] 无法解析 clientIdHash 文件 JSON: {}",
                            hash_file_path.display()
                        );
                    }
                } else {
                    tracing::warn!(
                        "[KIRO] 无法读取 clientIdHash 文件: {}",
                        hash_file_path.display()
                    );
                }
            } else {
                tracing::warn!(
                    "[KIRO] clientIdHash {} 指向的文件不存在: {}",
                    hash,
                    hash_file_path.display()
                );
            }
        } else {
            tracing::debug!("[KIRO] 没有 clientIdHash 字段，尝试扫描同目录文件");
        }

        // 如果还没有 client_id/client_secret，读取目录中其他 JSON 文件
        if merged.client_id.is_none() || merged.client_secret.is_none() {
            if tokio::fs::try_exists(dir).await.unwrap_or(false) {
                let mut entries = tokio::fs::read_dir(dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    let file_path = entry.path();
                    if file_path.extension().map(|e| e == "json").unwrap_or(false)
                        && file_path != path
                    {
                        if let Ok(content) = tokio::fs::read_to_string(&file_path).await {
                            if let Ok(creds) = serde_json::from_str::<KiroCredentials>(&content) {
                                tracing::info!(
                                    "[KIRO] Extra file {:?}: has_client_id={}, has_client_secret={}",
                                    file_path.file_name(),
                                    creds.client_id.is_some(),
                                    creds.client_secret.is_some()
                                );
                                merge_credentials(&mut merged, &creds);
                            }
                        }
                    }
                }
            }
        }

        tracing::info!(
            "[KIRO] Final merged from path: has_access={}, has_refresh={}, has_client_id={}, has_client_secret={}, auth_method={:?}",
            merged.access_token.is_some(),
            merged.refresh_token.is_some(),
            merged.client_id.is_some(),
            merged.client_secret.is_some(),
            merged.auth_method
        );

        self.credentials = merged;
        self.creds_path = Some(path);

        // 加载完成后，智能检测并更新认证方式（如果需要）
        let detected_auth_method = self.detect_auth_method();
        if self.credentials.auth_method.as_deref().unwrap_or("social") != detected_auth_method {
            tracing::info!(
                "[KIRO] 从路径加载后检测到需要调整认证方式为: {}",
                detected_auth_method
            );
            self.set_auth_method(&detected_auth_method);
        }

        Ok(())
    }

    pub fn get_base_url(&self) -> String {
        let region = self.credentials.region.as_deref().unwrap_or("us-east-1");
        format!("https://codewhisperer.{region}.amazonaws.com/generateAssistantResponse")
    }

    pub fn get_refresh_url(&self) -> String {
        let region = self.credentials.region.as_deref().unwrap_or("us-east-1");
        let auth_method = self
            .credentials
            .auth_method
            .as_deref()
            .unwrap_or("social")
            .to_lowercase();

        if auth_method == "idc" {
            format!("https://oidc.{region}.amazonaws.com/token")
        } else {
            format!("https://prod.{region}.auth.desktop.kiro.dev/refreshToken")
        }
    }

    /// 构建健康检查使用的端点，与实际API调用保持一致
    pub fn get_health_check_url(&self) -> String {
        // 重用基础URL逻辑，确保健康检查与实际API调用使用相同端点
        self.get_base_url()
    }

    /// 从凭证文件中提取 region 信息的静态方法，供健康检查服务使用
    pub fn extract_region_from_creds(creds_content: &str) -> Result<String, String> {
        let creds: serde_json::Value =
            serde_json::from_str(creds_content).map_err(|e| format!("解析凭证失败: {}", e))?;

        let region = creds["region"].as_str().unwrap_or("us-east-1").to_string();

        Ok(region)
    }

    /// 构建健康检查端点的静态方法，供外部服务使用
    pub fn build_health_check_url(region: &str) -> String {
        format!("https://codewhisperer.{region}.amazonaws.com/generateAssistantResponse")
    }

    /// 检查 Token 是否已过期（基于时间戳）
    pub fn is_token_expired(&self) -> bool {
        if let Some(expires_str) = &self.credentials.expires_at {
            if let Ok(expires_timestamp) = expires_str.parse::<i64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // 提前5分钟判断为过期，避免边界情况
                return now >= (expires_timestamp - 300);
            }
        }

        // 如果没有过期时间信息，保守地认为可能需要刷新
        true
    }

    /// 验证 refresh_token 的基本有效性
    pub fn validate_refresh_token(&self) -> Result<(), String> {
        let refresh_token = self.credentials.refresh_token.as_ref()
            .ok_or("缺少 refresh_token。\n💡 解决方案：\n1. 重新添加 OAuth 凭证\n2. 确保凭证文件包含完整的认证信息")?;

        // 基本格式验证
        if refresh_token.trim().is_empty() {
            return Err("refresh_token 为空。\n💡 解决方案：\n1. 检查凭证文件是否损坏\n2. 重新生成 OAuth 凭证".to_string());
        }

        // 检查是否看起来像有效的 token（简单的长度和格式检查）
        if refresh_token.len() < 10 {
            return Err("refresh_token 格式异常（长度过短）。\n💡 解决方案：\n1. 凭证文件可能已损坏\n2. 重新获取 OAuth 凭证".to_string());
        }

        Ok(())
    }

    /// 检测最佳的认证方式
    /// 优先使用 IdC（如果有完整配置），否则回退到 social ��证
    pub fn detect_auth_method(&self) -> String {
        // 检查当前设置的认证方式
        let current_auth = self.credentials.auth_method.as_deref().unwrap_or("social");

        // 如果当前是 IdC 方式，检查是否有完整的 IdC 配置
        if current_auth.to_lowercase() == "idc" {
            if self.credentials.client_id.is_some() && self.credentials.client_secret.is_some() {
                // IdC 配置完整，继续使用 IdC
                tracing::debug!("[KIRO] IdC 配置完整，使用 IdC 认证");
                "idc".to_string()
            } else {
                // IdC 配置不完整，降级到 social
                tracing::warn!("[KIRO] IdC 配置不完整（缺少 client_id 或 client_secret），自动降级到 social 认证");
                "social".to_string()
            }
        } else {
            // 默认或已设置为 social
            tracing::debug!("[KIRO] 使用 social 认证");
            "social".to_string()
        }
    }

    /// 更新认证方式到凭证中（仅在内存中，需要调用 save_credentials 持久化）
    pub fn set_auth_method(&mut self, method: &str) {
        let old_method = self.credentials.auth_method.as_deref().unwrap_or("social");
        if old_method != method {
            tracing::info!("[KIRO] 认证方式从 {} 切换到 {}", old_method, method);
            self.credentials.auth_method = Some(method.to_string());
        }
    }

    pub async fn refresh_token(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        // 首先验证 refresh_token 的有效性
        self.validate_refresh_token()?;

        tracing::info!("[KIRO] 开始 Token 刷新流程");
        tracing::info!(
            "[KIRO] 当前凭证状态: has_client_id={}, has_client_secret={}, auth_method={:?}",
            self.credentials.client_id.is_some(),
            self.credentials.client_secret.is_some(),
            self.credentials.auth_method
        );

        // 先克隆必要的值，避免借用冲突
        let refresh_token = self
            .credentials
            .refresh_token
            .as_ref()
            .ok_or("No refresh token")?
            .clone();

        // 使用智能检测的认证方式，而不是直接使用配置中的方式
        let detected_auth_method = self.detect_auth_method();
        tracing::info!("[KIRO] 检测到的认证方式: {}", detected_auth_method);

        // 如果检测到的方式与配置中的不同，更新配置
        let current_auth = self.credentials.auth_method.as_deref().unwrap_or("social");
        if current_auth != detected_auth_method {
            tracing::info!(
                "[KIRO] 认证方式从 {} 切换到 {}",
                current_auth,
                detected_auth_method
            );
            self.set_auth_method(&detected_auth_method);
        }

        let auth_method = detected_auth_method.to_lowercase();
        let refresh_url = self.get_refresh_url();

        tracing::debug!(
            "[KIRO] refresh_token: auth_method={}, refresh_url={}",
            auth_method,
            refresh_url
        );
        tracing::debug!(
            "[KIRO] has_client_id={}, has_client_secret={}",
            self.credentials.client_id.is_some(),
            self.credentials.client_secret.is_some()
        );

        let resp = if auth_method == "idc" {
            // IdC 认证使用 JSON 格式（参考 AIClient-2-API 实现）
            let client_id = self
                .credentials
                .client_id
                .as_ref()
                .ok_or("IdC 认证配置错误：缺少 client_id。建议删除后重新添加 OAuth 凭证")?;
            let client_secret = self
                .credentials
                .client_secret
                .as_ref()
                .ok_or("IdC 认证配置错误：缺少 client_secret。建议删除后重新添加 OAuth 凭证")?;

            // 使用 JSON 格式发送请求（与 AIClient-2-API 保持一致）
            let body = serde_json::json!({
                "refreshToken": &refresh_token,
                "clientId": client_id,
                "clientSecret": client_secret,
                "grantType": "refresh_token"
            });

            tracing::debug!("[KIRO] IdC 刷新请求体已构建");

            self.client
                .post(&refresh_url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(&body)
                .send()
                .await?
        } else {
            // Social 认证使用简单的 JSON 格式
            let body = serde_json::json!({ "refreshToken": &refresh_token });
            self.client
                .post(&refresh_url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(&body)
                .send()
                .await?
        };

        tracing::info!("[KIRO] Token 刷新响应状态: {}", resp.status());

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();

            tracing::warn!("[KIRO] Token 刷新失败: {} - {}", status, body_text);

            // 根据具体的HTTP状态码提供更友好的错误信息
            let error_msg = match status.as_u16() {
                401 => {
                    if body_text.contains("Bad credentials") || body_text.contains("invalid") {
                        format!("OAuth 凭证已过期或无效，需要重新认证。\n💡 解决方案：\n1. 删除当前 OAuth 凭证\n2. 重新添加 OAuth 凭证\n3. 确保使用最新的凭证文件\n\n技术详情：{} {}", status, body_text)
                    } else {
                        format!("认证失败，Token 可能已过期。\n💡 解决方案：\n1. 检查 AWS 账户状态\n2. 重新生成 OAuth 凭证\n3. 确保凭证文件格式正确\n\n技术详情：{} {}", status, body_text)
                    }
                }
                403 => format!("权限不足，无法刷新 Token。\n💡 解决方案：\n1. 检查 AWS 账户权限\n2. 确保 OAuth 应用配置正确\n3. 联系管理员检查权限设置\n\n技术详情：{} {}", status, body_text),
                429 => format!("请求过于频繁，已被限流。\n💡 解决方案：\n1. 等待 5-10 分钟后重试\n2. 减少 Token 刷新频率\n3. 检查是否有其他程序在同时使用\n\n技术详情：{} {}", status, body_text),
                500..=599 => format!("服务器错误，AWS OAuth 服务暂时不可用。\n💡 解决方案：\n1. 稍后重试（通常几分钟后恢复）\n2. 检查 AWS 服务状态页面\n3. 如持续失败，联系 AWS 支持\n\n技术详情：{} {}", status, body_text),
                _ => format!("Token 刷新失败。\n💡 解决方案：\n1. 检查网络连接\n2. 确认凭证文件完整性\n3. 尝试重新添加凭证\n\n技术详情：{} {}", status, body_text)
            };

            return Err(error_msg.into());
        }

        let data: serde_json::Value = resp.json().await?;

        // AWS OIDC returns snake_case, social endpoint returns camelCase
        let new_token = data["accessToken"]
            .as_str()
            .or_else(|| data["access_token"].as_str())
            .ok_or("No access token in response")?;

        self.credentials.access_token = Some(new_token.to_string());

        // Handle both camelCase and snake_case response formats
        if let Some(rt) = data["refreshToken"]
            .as_str()
            .or_else(|| data["refresh_token"].as_str())
        {
            self.credentials.refresh_token = Some(rt.to_string());
        }
        if let Some(arn) = data["profileArn"].as_str() {
            self.credentials.profile_arn = Some(arn.to_string());
        }

        // 保存更新后的凭证到文件
        self.save_credentials().await?;

        Ok(new_token.to_string())
    }

    pub async fn save_credentials(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // 使用加载时的路径或默认路径
        let path = self
            .creds_path
            .clone()
            .unwrap_or_else(Self::default_creds_path);

        // 读取现有文件内容
        let mut existing: serde_json::Value = if tokio::fs::try_exists(&path).await.unwrap_or(false)
        {
            let content = tokio::fs::read_to_string(&path).await?;
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        // 更新字段
        if let Some(token) = &self.credentials.access_token {
            existing["accessToken"] = serde_json::json!(token);
        }
        if let Some(token) = &self.credentials.refresh_token {
            existing["refreshToken"] = serde_json::json!(token);
        }
        if let Some(arn) = &self.credentials.profile_arn {
            existing["profileArn"] = serde_json::json!(arn);
        }

        // 写回文件
        let content = serde_json::to_string_pretty(&existing)?;
        tokio::fs::write(&path, content).await?;

        Ok(())
    }

    /// 检查 token 是否即将过期（10 分钟内）
    pub fn is_token_expiring_soon(&self) -> bool {
        if let Some(expires_at) = &self.credentials.expires_at {
            if let Ok(expiry) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                let now = chrono::Utc::now();
                let threshold = now + chrono::Duration::minutes(10);
                return expiry < threshold;
            }
        }
        // 如果没有过期时间，假设不需要刷新
        false
    }

    pub async fn call_api(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<reqwest::Response, Box<dyn Error + Send + Sync>> {
        let token = self
            .credentials
            .access_token
            .as_ref()
            .ok_or("No access token")?;

        let profile_arn = if self.credentials.auth_method.as_deref() == Some("social") {
            self.credentials.profile_arn.clone()
        } else {
            None
        };

        let cw_request = convert_openai_to_codewhisperer(request, profile_arn);
        let url = self.get_base_url();

        // Debug: 记录转换后的请求
        if let Ok(json_str) = serde_json::to_string_pretty(&cw_request) {
            // 保存到文件用于调试
            let uuid_prefix = uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("unknown")
                .to_string();
            let debug_path = dirs::home_dir()
                .unwrap_or_default()
                .join(".proxycast")
                .join("logs")
                .join(format!("cw_request_{uuid_prefix}.json"));
            let _ = tokio::fs::write(&debug_path, &json_str).await;
            tracing::debug!("[CW_REQ] Request saved to {:?}", debug_path);

            // 记录历史消息数量和 tool_results 情况
            let history_len = cw_request
                .conversation_state
                .history
                .as_ref()
                .map(|h| h.len())
                .unwrap_or(0);
            let current_has_tools = cw_request
                .conversation_state
                .current_message
                .user_input_message
                .user_input_message_context
                .as_ref()
                .map(|ctx| ctx.tool_results.as_ref().map(|tr| tr.len()).unwrap_or(0))
                .unwrap_or(0);
            tracing::info!(
                "[CW_REQ] history={} current_tool_results={}",
                history_len,
                current_has_tools
            );
        }

        // 生成设备指纹用于伪装 Kiro IDE
        let device_fp = get_device_fingerprint();
        let kiro_version = "0.1.25";

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("amz-sdk-invocation-id", uuid::Uuid::new_v4().to_string())
            .header("amz-sdk-request", "attempt=1; max=1")
            .header(
                "x-amz-user-agent",
                format!("aws-sdk-js/1.0.7 KiroIDE-{kiro_version}-{device_fp}"),
            )
            .header(
                "user-agent",
                format!(
                    "aws-sdk-js/1.0.7 ua/2.1 os/macos#14.0 lang/js md/nodejs#20.16.0 api/codewhispererstreaming#1.0.7 m/E KiroIDE-{kiro_version}-{device_fp}"
                ),
            )
            .header("x-amzn-kiro-agent-mode", "vibe")
            .json(&cw_request)
            .send()
            .await?;

        Ok(resp)
    }
}

fn merge_credentials(target: &mut KiroCredentials, source: &KiroCredentials) {
    if source.access_token.is_some() {
        target.access_token = source.access_token.clone();
    }
    if source.refresh_token.is_some() {
        target.refresh_token = source.refresh_token.clone();
    }
    if source.client_id.is_some() {
        target.client_id = source.client_id.clone();
    }
    if source.client_secret.is_some() {
        target.client_secret = source.client_secret.clone();
    }
    if source.profile_arn.is_some() {
        target.profile_arn = source.profile_arn.clone();
    }
    if source.expires_at.is_some() {
        target.expires_at = source.expires_at.clone();
    }
    if source.region.is_some() {
        target.region = source.region.clone();
    }
    if source.auth_method.is_some() {
        target.auth_method = source.auth_method.clone();
    }
    if source.client_id_hash.is_some() {
        target.client_id_hash = source.client_id_hash.clone();
    }
}
