//! API Key Provider 管理服务
//!
//! 提供 API Key Provider 的 CRUD 操作、加密存储和轮询负载均衡功能。
//!
//! **Feature: provider-ui-refactor**
//! **Validates: Requirements 7.3, 9.1, 9.2, 9.3**

use crate::database::dao::api_key_provider::{
    ApiKeyEntry, ApiKeyProvider, ApiKeyProviderDao, ApiProviderType, ProviderGroup,
    ProviderWithKeys,
};
use crate::database::system_providers::{get_system_providers, to_api_key_provider};
use crate::database::DbConnection;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

// ============================================================================
// 加密服务
// ============================================================================

/// 简单的 API Key 加密服务
/// 使用 XOR 加密 + Base64 编码
/// 注意：这是一个简单的混淆方案，不是强加密
struct EncryptionService {
    /// 加密密钥（从机器 ID 派生）
    key: Vec<u8>,
}

impl EncryptionService {
    /// 创建新的加密服务
    fn new() -> Self {
        // 使用机器特定信息生成密钥
        let machine_id = Self::get_machine_id();
        let mut hasher = Sha256::new();
        hasher.update(machine_id.as_bytes());
        hasher.update(b"proxycast-api-key-encryption-salt");
        let key = hasher.finalize().to_vec();

        Self { key }
    }

    /// 获取机器 ID
    fn get_machine_id() -> String {
        // 尝试获取机器 ID，失败则使用默认值
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            return id.trim().to_string();
        }
        if let Ok(id) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
            return id.trim().to_string();
        }
        // macOS: 使用 IOPlatformUUID
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("ioreg")
                .args(["-rd1", "-c", "IOPlatformExpertDevice"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("IOPlatformUUID") {
                        if let Some(uuid) = line.split('"').nth(3) {
                            return uuid.to_string();
                        }
                    }
                }
            }
        }
        // 默认值
        "proxycast-default-machine-id".to_string()
    }

    /// 加密 API Key
    fn encrypt(&self, plaintext: &str) -> String {
        let encrypted: Vec<u8> = plaintext
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ self.key[i % self.key.len()])
            .collect();
        BASE64.encode(encrypted)
    }

    /// 解密 API Key
    fn decrypt(&self, ciphertext: &str) -> Result<String, String> {
        let encrypted = BASE64
            .decode(ciphertext)
            .map_err(|e| format!("Base64 解码失败: {}", e))?;
        let decrypted: Vec<u8> = encrypted
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ self.key[i % self.key.len()])
            .collect();
        String::from_utf8(decrypted).map_err(|e| format!("UTF-8 解码失败: {}", e))
    }

    /// 检查是否为加密后的值（非明文）
    fn is_encrypted(&self, value: &str) -> bool {
        // 加密后的值是 Base64 编码的，通常不包含常见的 API Key 前缀
        !value.starts_with("sk-")
            && !value.starts_with("pk-")
            && !value.starts_with("api-")
            && BASE64.decode(value).is_ok()
    }
}

// ============================================================================
// API Key Provider 服务
// ============================================================================

/// API Key Provider 管理服务
pub struct ApiKeyProviderService {
    /// 加密服务
    encryption: EncryptionService,
    /// 轮询索引（按 provider_id 分组）
    round_robin_index: RwLock<HashMap<String, AtomicUsize>>,
}

impl Default for ApiKeyProviderService {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiKeyProviderService {
    /// 创建新的服务实例
    pub fn new() -> Self {
        Self {
            encryption: EncryptionService::new(),
            round_robin_index: RwLock::new(HashMap::new()),
        }
    }

    // ==================== Provider 操作 ====================

    /// 初始化系统 Provider
    /// 检查数据库中是否存在系统 Provider，如果不存在则插入
    /// **Validates: Requirements 9.3**
    pub fn initialize_system_providers(&self, db: &DbConnection) -> Result<usize, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let system_providers = get_system_providers();
        let mut inserted_count = 0;

        for def in &system_providers {
            // 检查是否已存在
            let existing =
                ApiKeyProviderDao::get_provider_by_id(&conn, def.id).map_err(|e| e.to_string())?;

            if existing.is_none() {
                // 插入新的系统 Provider
                let provider = to_api_key_provider(def);
                ApiKeyProviderDao::insert_provider(&conn, &provider).map_err(|e| e.to_string())?;
                inserted_count += 1;
            }
        }

        if inserted_count > 0 {
            tracing::info!("初始化了 {} 个系统 Provider", inserted_count);
        }

        Ok(inserted_count)
    }

    /// 获取所有 Provider（包含 API Keys）
    /// 首次调用时会自动初始化系统 Provider
    pub fn get_all_providers(&self, db: &DbConnection) -> Result<Vec<ProviderWithKeys>, String> {
        // 首先确保系统 Provider 已初始化
        self.initialize_system_providers(db)?;

        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut providers =
            ApiKeyProviderDao::get_all_providers_with_keys(&conn).map_err(|e| e.to_string())?;

        // 解密 API Keys（用于前端显示掩码）
        for provider in &mut providers {
            for _key in &mut provider.api_keys {
                // 保持加密状态，前端会显示掩码
            }
        }

        Ok(providers)
    }

    /// 获取单个 Provider（包含 API Keys）
    pub fn get_provider(
        &self,
        db: &DbConnection,
        id: &str,
    ) -> Result<Option<ProviderWithKeys>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let provider =
            ApiKeyProviderDao::get_provider_by_id(&conn, id).map_err(|e| e.to_string())?;

        match provider {
            Some(p) => {
                let api_keys = ApiKeyProviderDao::get_api_keys_by_provider(&conn, id)
                    .map_err(|e| e.to_string())?;
                Ok(Some(ProviderWithKeys {
                    provider: p,
                    api_keys,
                }))
            }
            None => Ok(None),
        }
    }

    /// 添加自定义 Provider
    pub fn add_custom_provider(
        &self,
        db: &DbConnection,
        name: String,
        provider_type: ApiProviderType,
        api_host: String,
        api_version: Option<String>,
        project: Option<String>,
        location: Option<String>,
        region: Option<String>,
    ) -> Result<ApiKeyProvider, String> {
        let now = Utc::now();
        let id = format!("custom-{}", uuid::Uuid::new_v4());

        let provider = ApiKeyProvider {
            id: id.clone(),
            name,
            provider_type,
            api_host,
            is_system: false,
            group: ProviderGroup::Custom,
            enabled: true,
            sort_order: 9999, // 自定义 Provider 排在最后
            api_version,
            project,
            location,
            region,
            created_at: now,
            updated_at: now,
        };

        let conn = db.lock().map_err(|e| e.to_string())?;
        ApiKeyProviderDao::insert_provider(&conn, &provider).map_err(|e| e.to_string())?;

        Ok(provider)
    }

    /// 更新 Provider 配置
    pub fn update_provider(
        &self,
        db: &DbConnection,
        id: &str,
        name: Option<String>,
        api_host: Option<String>,
        enabled: Option<bool>,
        sort_order: Option<i32>,
        api_version: Option<String>,
        project: Option<String>,
        location: Option<String>,
        region: Option<String>,
    ) -> Result<ApiKeyProvider, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut provider = ApiKeyProviderDao::get_provider_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Provider not found: {}", id))?;

        // 更新字段
        if let Some(n) = name {
            provider.name = n;
        }
        if let Some(h) = api_host {
            provider.api_host = h;
        }
        if let Some(e) = enabled {
            provider.enabled = e;
        }
        if let Some(s) = sort_order {
            provider.sort_order = s;
        }
        if let Some(v) = api_version {
            provider.api_version = if v.is_empty() { None } else { Some(v) };
        }
        if let Some(p) = project {
            provider.project = if p.is_empty() { None } else { Some(p) };
        }
        if let Some(l) = location {
            provider.location = if l.is_empty() { None } else { Some(l) };
        }
        if let Some(r) = region {
            provider.region = if r.is_empty() { None } else { Some(r) };
        }
        provider.updated_at = Utc::now();

        ApiKeyProviderDao::update_provider(&conn, &provider).map_err(|e| e.to_string())?;

        Ok(provider)
    }

    /// 删除自定义 Provider
    /// 系统 Provider 不允许删除
    pub fn delete_custom_provider(&self, db: &DbConnection, id: &str) -> Result<bool, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // 检查是否为系统 Provider
        let provider = ApiKeyProviderDao::get_provider_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Provider not found: {}", id))?;

        if provider.is_system {
            return Err("不允许删除系统 Provider".to_string());
        }

        ApiKeyProviderDao::delete_provider(&conn, id).map_err(|e| e.to_string())
    }

    // ==================== API Key 操作 ====================

    /// 添加 API Key
    ///
    /// 当添加第一个 API Key 时，会自动启用 Provider
    pub fn add_api_key(
        &self,
        db: &DbConnection,
        provider_id: &str,
        api_key: &str,
        alias: Option<String>,
    ) -> Result<ApiKeyEntry, String> {
        // 验证 Provider 存在
        let conn = db.lock().map_err(|e| e.to_string())?;
        let provider = ApiKeyProviderDao::get_provider_by_id(&conn, provider_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Provider not found: {}", provider_id))?;

        // 检查是否是第一个 API Key，如果是则自动启用 Provider
        let existing_keys = ApiKeyProviderDao::get_api_keys_by_provider(&conn, provider_id)
            .map_err(|e| e.to_string())?;
        let should_enable_provider = existing_keys.is_empty() && !provider.enabled;

        // 加密 API Key
        let encrypted_key = self.encryption.encrypt(api_key);

        let now = Utc::now();
        let key = ApiKeyEntry {
            id: uuid::Uuid::new_v4().to_string(),
            provider_id: provider_id.to_string(),
            api_key_encrypted: encrypted_key,
            alias,
            enabled: true,
            usage_count: 0,
            error_count: 0,
            last_used_at: None,
            created_at: now,
        };

        ApiKeyProviderDao::insert_api_key(&conn, &key).map_err(|e| e.to_string())?;

        // 如果是第一个 API Key，自动启用 Provider
        if should_enable_provider {
            let mut updated_provider = provider;
            updated_provider.enabled = true;
            updated_provider.updated_at = now;
            ApiKeyProviderDao::update_provider(&conn, &updated_provider)
                .map_err(|e| e.to_string())?;
            tracing::info!(
                "[ApiKeyProviderService] 自动启用 Provider: {} (添加了第一个 API Key)",
                provider_id
            );
        }

        Ok(key)
    }

    /// 删除 API Key
    pub fn delete_api_key(&self, db: &DbConnection, key_id: &str) -> Result<bool, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ApiKeyProviderDao::delete_api_key(&conn, key_id).map_err(|e| e.to_string())
    }

    /// 切换 API Key 启用状态
    pub fn toggle_api_key(
        &self,
        db: &DbConnection,
        key_id: &str,
        enabled: bool,
    ) -> Result<ApiKeyEntry, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut key = ApiKeyProviderDao::get_api_key_by_id(&conn, key_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("API Key not found: {}", key_id))?;

        key.enabled = enabled;
        ApiKeyProviderDao::update_api_key(&conn, &key).map_err(|e| e.to_string())?;

        Ok(key)
    }

    /// 更新 API Key 别名
    pub fn update_api_key_alias(
        &self,
        db: &DbConnection,
        key_id: &str,
        alias: Option<String>,
    ) -> Result<ApiKeyEntry, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut key = ApiKeyProviderDao::get_api_key_by_id(&conn, key_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("API Key not found: {}", key_id))?;

        key.alias = alias;
        ApiKeyProviderDao::update_api_key(&conn, &key).map_err(|e| e.to_string())?;

        Ok(key)
    }

    // ==================== 轮询负载均衡 ====================

    /// 获取下一个可用的 API Key（轮询负载均衡）
    /// **Validates: Requirements 7.3**
    pub fn get_next_api_key(
        &self,
        db: &DbConnection,
        provider_id: &str,
    ) -> Result<Option<String>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // 获取所有启用的 API Keys
        let keys = ApiKeyProviderDao::get_enabled_api_keys_by_provider(&conn, provider_id)
            .map_err(|e| e.to_string())?;

        if keys.is_empty() {
            return Ok(None);
        }

        // 获取或创建轮询索引
        let index = {
            let mut indices = self.round_robin_index.write().map_err(|e| e.to_string())?;
            indices
                .entry(provider_id.to_string())
                .or_insert_with(|| AtomicUsize::new(0))
                .fetch_add(1, Ordering::SeqCst)
        };

        // 选择 API Key
        let selected_key = &keys[index % keys.len()];

        // 解密并返回
        let decrypted = self.encryption.decrypt(&selected_key.api_key_encrypted)?;
        Ok(Some(decrypted))
    }

    /// 获取下一个可用的 API Key 条目（包含 ID，用于记录使用）
    pub fn get_next_api_key_entry(
        &self,
        db: &DbConnection,
        provider_id: &str,
    ) -> Result<Option<(String, String)>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // 获取所有启用的 API Keys
        let keys = ApiKeyProviderDao::get_enabled_api_keys_by_provider(&conn, provider_id)
            .map_err(|e| e.to_string())?;

        if keys.is_empty() {
            return Ok(None);
        }

        // 获取或创建轮询索引
        let index = {
            let mut indices = self.round_robin_index.write().map_err(|e| e.to_string())?;
            indices
                .entry(provider_id.to_string())
                .or_insert_with(|| AtomicUsize::new(0))
                .fetch_add(1, Ordering::SeqCst)
        };

        // 选择 API Key
        let selected_key = &keys[index % keys.len()];

        // 解密并返回
        let decrypted = self.encryption.decrypt(&selected_key.api_key_encrypted)?;
        Ok(Some((selected_key.id.clone(), decrypted)))
    }

    /// 记录 API Key 使用
    pub fn record_usage(&self, db: &DbConnection, key_id: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let key = ApiKeyProviderDao::get_api_key_by_id(&conn, key_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("API Key not found: {}", key_id))?;

        ApiKeyProviderDao::update_api_key_usage(&conn, key_id, key.usage_count + 1, Utc::now())
            .map_err(|e| e.to_string())
    }

    /// 按 Provider 类型获取下一个可用的 API Key（轮询负载均衡）
    /// 这个方法会查找所有该类型的 Provider（包括自定义 Provider）
    pub fn get_next_api_key_by_type(
        &self,
        db: &DbConnection,
        provider_type: ApiProviderType,
    ) -> Result<Option<(String, String, ApiKeyProvider)>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // 获取所有启用的 API Keys（按类型）
        let keys = ApiKeyProviderDao::get_enabled_api_keys_by_type(&conn, provider_type)
            .map_err(|e| e.to_string())?;

        if keys.is_empty() {
            return Ok(None);
        }

        // 使用类型名称作为轮询索引的 key
        let type_key = format!("type:{}", provider_type);
        let index = {
            let mut indices = self.round_robin_index.write().map_err(|e| e.to_string())?;
            indices
                .entry(type_key)
                .or_insert_with(|| AtomicUsize::new(0))
                .fetch_add(1, Ordering::SeqCst)
        };

        // 选择 API Key
        let (selected_key, provider) = &keys[index % keys.len()];

        // 解密并返回
        let decrypted = self.encryption.decrypt(&selected_key.api_key_encrypted)?;
        Ok(Some((selected_key.id.clone(), decrypted, provider.clone())))
    }

    /// 记录 API Key 错误
    pub fn record_error(&self, db: &DbConnection, key_id: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ApiKeyProviderDao::increment_api_key_error(&conn, key_id).map_err(|e| e.to_string())
    }

    // ==================== 加密相关 ====================

    /// 检查 API Key 是否已加密
    pub fn is_encrypted(&self, value: &str) -> bool {
        self.encryption.is_encrypted(value)
    }

    /// 解密 API Key（用于 API 调用）
    pub fn decrypt_api_key(&self, encrypted: &str) -> Result<String, String> {
        self.encryption.decrypt(encrypted)
    }

    /// 加密 API Key（用于存储）
    pub fn encrypt_api_key(&self, plaintext: &str) -> String {
        self.encryption.encrypt(plaintext)
    }

    // ==================== UI 状态 ====================

    /// 获取 UI 状态
    pub fn get_ui_state(&self, db: &DbConnection, key: &str) -> Result<Option<String>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ApiKeyProviderDao::get_ui_state(&conn, key).map_err(|e| e.to_string())
    }

    /// 设置 UI 状态
    pub fn set_ui_state(&self, db: &DbConnection, key: &str, value: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ApiKeyProviderDao::set_ui_state(&conn, key, value).map_err(|e| e.to_string())
    }

    /// 批量更新 Provider 排序顺序
    /// **Validates: Requirements 8.4**
    pub fn update_provider_sort_orders(
        &self,
        db: &DbConnection,
        sort_orders: Vec<(String, i32)>,
    ) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ApiKeyProviderDao::update_provider_sort_orders(&conn, &sort_orders)
            .map_err(|e| e.to_string())
    }

    // ==================== 导入导出 ====================

    /// 导出配置
    pub fn export_config(
        &self,
        db: &DbConnection,
        include_keys: bool,
    ) -> Result<serde_json::Value, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let providers =
            ApiKeyProviderDao::get_all_providers_with_keys(&conn).map_err(|e| e.to_string())?;

        let export_data = if include_keys {
            // 包含 API Keys（但不包含实际的 key 值）
            let providers_json: Vec<serde_json::Value> = providers
                .iter()
                .map(|p| {
                    let keys: Vec<serde_json::Value> = p
                        .api_keys
                        .iter()
                        .map(|k| {
                            serde_json::json!({
                                "id": k.id,
                                "alias": k.alias,
                                "enabled": k.enabled,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "provider": p.provider,
                        "api_keys": keys,
                    })
                })
                .collect();
            serde_json::json!({
                "version": "1.0",
                "exported_at": Utc::now().to_rfc3339(),
                "providers": providers_json,
            })
        } else {
            // 不包含 API Keys
            let providers_json: Vec<serde_json::Value> = providers
                .iter()
                .map(|p| serde_json::json!(p.provider))
                .collect();
            serde_json::json!({
                "version": "1.0",
                "exported_at": Utc::now().to_rfc3339(),
                "providers": providers_json,
            })
        };

        Ok(export_data)
    }

    /// 导入配置
    pub fn import_config(
        &self,
        db: &DbConnection,
        config_json: &str,
    ) -> Result<ImportResult, String> {
        let config: serde_json::Value =
            serde_json::from_str(config_json).map_err(|e| format!("JSON 解析失败: {}", e))?;

        let providers = config["providers"]
            .as_array()
            .ok_or_else(|| "配置格式错误: 缺少 providers 数组".to_string())?;

        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut imported_providers = 0;
        let mut skipped_providers = 0;
        let mut errors = Vec::new();

        for provider_json in providers {
            let provider_data = if provider_json.get("provider").is_some() {
                &provider_json["provider"]
            } else {
                provider_json
            };

            let id = provider_data["id"]
                .as_str()
                .ok_or_else(|| "Provider 缺少 id".to_string())?;

            // 检查是否已存在
            if ApiKeyProviderDao::get_provider_by_id(&conn, id)
                .map_err(|e| e.to_string())?
                .is_some()
            {
                skipped_providers += 1;
                continue;
            }

            // 解析 Provider
            let provider: ApiKeyProvider = serde_json::from_value(provider_data.clone())
                .map_err(|e| format!("Provider 解析失败: {}", e))?;

            // 插入 Provider
            if let Err(e) = ApiKeyProviderDao::insert_provider(&conn, &provider) {
                errors.push(format!("导入 Provider {} 失败: {}", id, e));
                continue;
            }

            imported_providers += 1;
        }

        Ok(ImportResult {
            success: errors.is_empty(),
            imported_providers,
            imported_api_keys: 0, // API Keys 不在导入中包含实际值
            skipped_providers,
            errors,
        })
    }

    // ==================== 智能降级 ====================

    /// 根据 PoolProviderType 获取降级凭证
    ///
    /// 用于智能降级场景：当 Provider Pool 无可用凭证时，自动从 API Key Provider 查找
    ///
    /// 降级策略：
    /// 1. 首先通过类型映射查找 (PoolProviderType → ApiProviderType)
    /// 2. 如果类型映射失败，尝试通过 provider_id 直接查找 (支持 60+ Provider)
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `pool_type`: Provider Pool 中的 Provider 类型
    /// - `provider_id_hint`: 可选的 provider_id 提示，如 "deepseek", "dashscope"
    ///
    /// # 返回
    /// - `Ok(Some(credential))`: 找到可用的降级凭证
    /// - `Ok(None)`: 没有找到可用的降级凭证
    /// - `Err(e)`: 查询过程中发生错误
    pub fn get_fallback_credential(
        &self,
        db: &DbConnection,
        pool_type: &PoolProviderType,
        provider_id_hint: Option<&str>,
    ) -> Result<Option<ProviderCredential>, String> {
        eprintln!(
            "[get_fallback_credential] 开始查找: pool_type={:?}, provider_id_hint={:?}",
            pool_type, provider_id_hint
        );

        // 策略 1: 优先通过 provider_id 直接查找 (支持 deepseek, moonshot 等 60+ Provider)
        // 这些 Provider 在 API Key Provider 中有独立配置，应该优先使用
        if let Some(provider_id) = provider_id_hint {
            eprintln!(
                "[get_fallback_credential] 尝试按 provider_id '{}' 查找",
                provider_id
            );
            if let Some(cred) = self.find_by_provider_id(db, provider_id)? {
                eprintln!(
                    "[get_fallback_credential] 通过 provider_id '{}' 找到凭证: {:?}",
                    provider_id, cred.name
                );
                return Ok(Some(cred));
            }
            eprintln!(
                "[get_fallback_credential] provider_id '{}' 未找到凭证",
                provider_id
            );
        }

        // 策略 2: 通过类型映射查找（降级方案）
        if let Some(api_type) = self.map_pool_type_to_api_type(pool_type) {
            eprintln!(
                "[get_fallback_credential] 尝试类型映射: {:?} -> {:?}",
                pool_type, api_type
            );
            if let Some(cred) = self.find_by_api_type(db, pool_type, &api_type)? {
                eprintln!(
                    "[get_fallback_credential] 通过类型映射找到凭证: {:?}",
                    cred.name
                );
                return Ok(Some(cred));
            }
        }

        eprintln!(
            "[get_fallback_credential] 未找到 {:?} 的降级凭证 (provider_id_hint: {:?})",
            pool_type, provider_id_hint
        );
        Ok(None)
    }

    /// PoolProviderType → ApiProviderType 映射
    ///
    /// 仅映射有明确对应关系的类型
    fn map_pool_type_to_api_type(&self, pool_type: &PoolProviderType) -> Option<ApiProviderType> {
        match pool_type {
            // API Key 类型 - 直接映射
            PoolProviderType::Claude => Some(ApiProviderType::Anthropic),
            PoolProviderType::OpenAI => Some(ApiProviderType::Openai),
            PoolProviderType::GeminiApiKey => Some(ApiProviderType::Gemini),
            PoolProviderType::Vertex => Some(ApiProviderType::Vertexai),

            // OAuth 类型 - 可降级到 API Key
            PoolProviderType::Gemini => Some(ApiProviderType::Gemini), // Gemini OAuth → Gemini API Key
            PoolProviderType::Qwen => Some(ApiProviderType::Openai), // Qwen OAuth → Dashscope (OpenAI 兼容)

            // API Key Provider 类型 - 直接映射
            PoolProviderType::Anthropic => Some(ApiProviderType::Anthropic),
            PoolProviderType::AzureOpenai => Some(ApiProviderType::AzureOpenai),
            PoolProviderType::AwsBedrock => Some(ApiProviderType::AwsBedrock),
            PoolProviderType::Ollama => Some(ApiProviderType::Ollama),

            // OAuth-only，无降级
            PoolProviderType::Kiro => None,
            PoolProviderType::Codex => None,
            PoolProviderType::ClaudeOAuth => None,
            PoolProviderType::Antigravity => None,
            PoolProviderType::IFlow => None,
        }
    }

    /// 通过 ApiProviderType 查找凭证
    fn find_by_api_type(
        &self,
        db: &DbConnection,
        pool_type: &PoolProviderType,
        api_type: &ApiProviderType,
    ) -> Result<Option<ProviderCredential>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // 查找该类型的启用的 Provider（按 sort_order 排序）
        let providers = ApiKeyProviderDao::get_all_providers(&conn).map_err(|e| e.to_string())?;

        let matching_providers: Vec<_> = providers
            .into_iter()
            .filter(|p| p.enabled && p.provider_type == *api_type)
            .collect();

        if matching_providers.is_empty() {
            return Ok(None);
        }

        // 尝试从每个匹配的 Provider 获取可用的 API Key
        for provider in matching_providers {
            let keys = ApiKeyProviderDao::get_enabled_api_keys_by_provider(&conn, &provider.id)
                .map_err(|e| e.to_string())?;

            if keys.is_empty() {
                continue;
            }

            // 轮询选择 API Key
            let index = {
                let mut indices = self.round_robin_index.write().map_err(|e| e.to_string())?;
                indices
                    .entry(provider.id.clone())
                    .or_insert_with(|| AtomicUsize::new(0))
                    .fetch_add(1, Ordering::SeqCst)
            };

            let selected_key = &keys[index % keys.len()];

            // 解密 API Key
            let api_key = self.encryption.decrypt(&selected_key.api_key_encrypted)?;

            // 转换为 ProviderCredential
            let credential = self.convert_to_provider_credential(
                pool_type,
                api_type,
                &provider,
                &selected_key.id,
                &api_key,
            )?;

            tracing::info!(
                "[智能降级] 成功找到凭证: {:?} -> {} (key: {})",
                pool_type,
                provider.name,
                selected_key.alias.as_deref().unwrap_or(&selected_key.id)
            );

            return Ok(Some(credential));
        }

        Ok(None)
    }

    /// 通过 provider_id 直接查找凭证 (支持 60+ Provider)
    ///
    /// 例如: "deepseek", "dashscope", "openrouter"
    fn find_by_provider_id(
        &self,
        db: &DbConnection,
        provider_id: &str,
    ) -> Result<Option<ProviderCredential>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // 直接按 provider_id 查找
        let provider =
            ApiKeyProviderDao::get_provider_by_id(&conn, provider_id).map_err(|e| e.to_string())?;

        let provider = match provider {
            Some(p) if p.enabled => {
                eprintln!(
                    "[find_by_provider_id] 找到已启用的 provider: id={}, name={}, api_host={}",
                    p.id, p.name, p.api_host
                );
                p
            }
            Some(_p) => {
                eprintln!(
                    "[find_by_provider_id] provider '{}' 存在但未启用",
                    provider_id
                );
                return Ok(None);
            }
            None => {
                eprintln!("[find_by_provider_id] provider '{}' 不存在", provider_id);
                return Ok(None);
            }
        };

        // 获取启用的 API Key
        let keys = ApiKeyProviderDao::get_enabled_api_keys_by_provider(&conn, &provider.id)
            .map_err(|e| e.to_string())?;

        if keys.is_empty() {
            eprintln!(
                "[find_by_provider_id] provider '{}' 没有启用的 API Key",
                provider_id
            );
            return Ok(None);
        }

        eprintln!(
            "[find_by_provider_id] provider '{}' 有 {} 个启用的 API Key",
            provider_id,
            keys.len()
        );

        // 轮询选择 API Key
        let index = {
            let mut indices = self.round_robin_index.write().map_err(|e| e.to_string())?;
            indices
                .entry(provider.id.clone())
                .or_insert_with(|| AtomicUsize::new(0))
                .fetch_add(1, Ordering::SeqCst)
        };

        let selected_key = &keys[index % keys.len()];

        // 解密 API Key
        let api_key = self.encryption.decrypt(&selected_key.api_key_encrypted)?;

        // 转换为 OpenAI 兼容的 ProviderCredential
        // 大多数 60+ Provider 都使用 OpenAI 兼容协议
        let credential =
            self.convert_to_openai_compatible_credential(&provider, &selected_key.id, &api_key)?;

        tracing::info!(
            "[智能降级] 成功通过 provider_id 找到凭证: {} (key: {})",
            provider.name,
            selected_key.alias.as_deref().unwrap_or(&selected_key.id)
        );

        Ok(Some(credential))
    }

    /// 转换为 ProviderCredential
    fn convert_to_provider_credential(
        &self,
        pool_type: &PoolProviderType,
        api_type: &ApiProviderType,
        provider: &ApiKeyProvider,
        key_id: &str,
        api_key: &str,
    ) -> Result<ProviderCredential, String> {
        let credential_data = match api_type {
            ApiProviderType::Anthropic => CredentialData::ClaudeKey {
                api_key: api_key.to_string(),
                base_url: Some(provider.api_host.clone()),
            },
            ApiProviderType::Gemini => CredentialData::GeminiApiKey {
                api_key: api_key.to_string(),
                base_url: Some(provider.api_host.clone()),
                excluded_models: Vec::new(),
            },
            ApiProviderType::Vertexai => CredentialData::VertexKey {
                api_key: api_key.to_string(),
                base_url: Some(provider.api_host.clone()),
                model_aliases: std::collections::HashMap::new(),
            },
            // 其他类型（包括 Openai, OpenaiResponse 等）都用 OpenAI Key 格式
            _ => CredentialData::OpenAIKey {
                api_key: api_key.to_string(),
                base_url: Some(provider.api_host.clone()),
            },
        };

        let now = chrono::Utc::now();
        Ok(ProviderCredential {
            uuid: format!("fallback-{}", key_id),
            provider_type: *pool_type,
            credential: credential_data,
            name: Some(format!("[降级] {}", provider.name)),
            is_healthy: true,
            is_disabled: false,
            check_health: false, // 降级凭证不参与健康检查
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
            source: CredentialSource::Imported, // 标记为导入来源
            proxy_url: None,
        })
    }

    /// 转换为 OpenAI 兼容的 ProviderCredential
    ///
    /// 用于 DeepSeek、Moonshot、智谱 等 60+ Provider
    fn convert_to_openai_compatible_credential(
        &self,
        provider: &ApiKeyProvider,
        key_id: &str,
        api_key: &str,
    ) -> Result<ProviderCredential, String> {
        let credential_data = CredentialData::OpenAIKey {
            api_key: api_key.to_string(),
            base_url: Some(provider.api_host.clone()), // 关键：使用 Provider 的 api_host
        };

        let now = chrono::Utc::now();
        Ok(ProviderCredential {
            uuid: format!("fallback-{}", key_id),
            provider_type: PoolProviderType::OpenAI, // 统一使用 OpenAI 类型
            credential: credential_data,
            name: Some(format!("[降级] {}", provider.name)),
            is_healthy: true,
            is_disabled: false,
            check_health: false, // 降级凭证不参与健康检查
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
            source: CredentialSource::Imported, // 标记为导入来源
            proxy_url: None,
        })
    }
}

/// 导入结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub success: bool,
    pub imported_providers: usize,
    pub imported_api_keys: usize,
    pub skipped_providers: usize,
    pub errors: Vec<String>,
}

use serde::{Deserialize, Serialize};

use crate::models::provider_pool_model::{
    CredentialData, CredentialSource, PoolProviderType, ProviderCredential,
};
