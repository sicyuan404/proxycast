//! OAuth Provider 插件命令
//!
//! 提供 OAuth Provider 插件管理的 Tauri 命令：
//! - list_oauth_plugins: 获取所有已安装的 OAuth Provider 插件
//! - get_oauth_plugin: 获取单个插件信息
//! - enable_oauth_plugin: 启用插件
//! - disable_oauth_plugin: 禁用插件
//! - install_oauth_plugin: 安装插件
//! - uninstall_oauth_plugin: 卸载插件
//! - 插件 SDK 命令

use crate::credential::{
    get_global_registry, init_global_registry, OAuthPluginLoader, PluginPermission,
    PluginSdkContext, PluginSource,
};
use crate::database::DbConnection;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

// ============================================================================
// 状态管理
// ============================================================================

/// OAuth 插件管理器状态
pub struct OAuthPluginManagerState {
    /// 插件加载器
    pub loader: Arc<RwLock<OAuthPluginLoader>>,
    /// 是否已初始化
    pub initialized: Arc<RwLock<bool>>,
}

impl OAuthPluginManagerState {
    /// 创建新状态
    pub fn new(plugins_dir: PathBuf) -> Self {
        Self {
            loader: Arc::new(RwLock::new(OAuthPluginLoader::new(plugins_dir))),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(OAuthPluginLoader::default_plugins_dir())
    }
}

// ============================================================================
// 响应类型
// ============================================================================

/// 认证类型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTypeInfoResponse {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub icon: Option<String>,
}

/// 模型家族信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFamilyResponse {
    pub name: String,
    pub pattern: String,
    pub tier: Option<String>,
    pub description: Option<String>,
}

/// OAuth 插件信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthPluginInfoResponse {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub target_protocol: String,
    pub category: String,
    pub enabled: bool,
    pub install_path: String,
    pub installed_at: String,
    pub last_used_at: Option<String>,
    pub credential_count: u32,
    pub healthy_credential_count: u32,
    pub auth_types: Vec<AuthTypeInfoResponse>,
}

/// 插件安装来源
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginSourceRequest {
    GitHub {
        owner: String,
        repo: String,
        version: Option<String>,
    },
    LocalFile {
        path: String,
    },
    Builtin {
        id: String,
    },
}

impl From<PluginSourceRequest> for PluginSource {
    fn from(req: PluginSourceRequest) -> Self {
        match req {
            PluginSourceRequest::GitHub {
                owner,
                repo,
                version,
            } => PluginSource::GitHub {
                owner,
                repo,
                version,
            },
            PluginSourceRequest::LocalFile { path } => PluginSource::LocalFile {
                path: PathBuf::from(path),
            },
            PluginSourceRequest::Builtin { id } => PluginSource::Builtin { id },
        }
    }
}

/// 安装结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResultResponse {
    pub success: bool,
    pub plugin_id: Option<String>,
    pub error: Option<String>,
}

/// 插件更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUpdateResponse {
    pub plugin_id: String,
    pub current_version: String,
    pub latest_version: String,
    pub changelog: Option<String>,
}

// ============================================================================
// 插件管理命令
// ============================================================================

/// 初始化 OAuth 插件系统
#[tauri::command]
pub async fn init_oauth_plugin_system(
    state: tauri::State<'_, OAuthPluginManagerState>,
) -> Result<(), String> {
    let mut initialized = state.initialized.write().await;
    if *initialized {
        return Ok(());
    }

    let loader = state.loader.read().await;

    // 初始化全局注册表
    let registry = init_global_registry(loader.plugins_dir().to_path_buf());

    // 加载所有插件
    match loader.load_all(&registry).await {
        Ok(loaded) => {
            info!("已加载 {} 个 OAuth Provider 插件", loaded.len());
            *initialized = true;
            Ok(())
        }
        Err(e) => {
            error!("加载 OAuth Provider 插件失败: {}", e);
            Err(e.to_string())
        }
    }
}

/// 获取所有已安装的 OAuth Provider 插件
#[tauri::command]
pub async fn list_oauth_plugins(
    db: tauri::State<'_, DbConnection>,
) -> Result<Vec<OAuthPluginInfoResponse>, String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    let infos = registry.get_plugin_infos();

    // 查询每个插件的凭证数量
    let conn = db.lock().map_err(|e| e.to_string())?;

    let plugins: Vec<OAuthPluginInfoResponse> = infos
        .into_iter()
        .map(|info| {
            // 查询凭证数量
            let credential_count: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM plugin_credentials WHERE plugin_id = ?",
                    params![info.id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            OAuthPluginInfoResponse {
                id: info.id.clone(),
                display_name: info.display_name,
                version: info.version,
                description: info.description,
                target_protocol: info.target_protocol,
                category: format!("{:?}", info.category),
                enabled: info.enabled,
                install_path: registry
                    .plugins_dir()
                    .join(&info.id)
                    .to_string_lossy()
                    .to_string(),
                installed_at: chrono::Utc::now().to_rfc3339(), // TODO: 从数据库获取
                last_used_at: None,
                credential_count,
                healthy_credential_count: info.healthy_credential_count,
                auth_types: info
                    .auth_types
                    .into_iter()
                    .map(|a| AuthTypeInfoResponse {
                        id: a.id,
                        display_name: a.display_name,
                        description: a.description,
                        category: format!("{:?}", a.category),
                        icon: a.icon,
                    })
                    .collect(),
            }
        })
        .collect();

    Ok(plugins)
}

/// 获取单个插件信息
#[tauri::command]
pub async fn get_oauth_plugin(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
) -> Result<Option<OAuthPluginInfoResponse>, String> {
    let plugins = list_oauth_plugins(db).await?;
    Ok(plugins.into_iter().find(|p| p.id == plugin_id))
}

/// 启用插件
#[tauri::command]
pub async fn enable_oauth_plugin(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
) -> Result<(), String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    if registry.enable_plugin(&plugin_id) {
        info!("已启用 OAuth 插件: {}", plugin_id);

        // 更新数据库
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE credential_provider_plugins SET enabled = 1, updated_at = ? WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), plugin_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    } else {
        Err(format!("插件不存在: {}", plugin_id))
    }
}

/// 禁用插件
#[tauri::command]
pub async fn disable_oauth_plugin(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
) -> Result<(), String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    if registry.disable_plugin(&plugin_id) {
        info!("已禁用 OAuth 插件: {}", plugin_id);

        // 更新数据库
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE credential_provider_plugins SET enabled = 0, updated_at = ? WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), plugin_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    } else {
        Err(format!("插件不存在: {}", plugin_id))
    }
}

/// 安装插件
#[tauri::command]
pub async fn install_oauth_plugin(
    _state: tauri::State<'_, OAuthPluginManagerState>,
    source: PluginSourceRequest,
) -> Result<InstallResultResponse, String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    let plugin_source: PluginSource = source.into();

    match registry.install_plugin(plugin_source).await {
        Ok(plugin_id) => {
            info!("已安装 OAuth 插件: {}", plugin_id);
            Ok(InstallResultResponse {
                success: true,
                plugin_id: Some(plugin_id),
                error: None,
            })
        }
        Err(e) => {
            error!("安装 OAuth 插件失败: {}", e);
            Ok(InstallResultResponse {
                success: false,
                plugin_id: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// 卸载插件
#[tauri::command]
pub async fn uninstall_oauth_plugin(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
) -> Result<(), String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    // 使用块作用域确保 MutexGuard 在 await 之前释放
    {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // 删除插件凭证
        conn.execute(
            "DELETE FROM plugin_credentials WHERE plugin_id = ?",
            params![plugin_id],
        )
        .map_err(|e| e.to_string())?;

        // 删除插件存储
        conn.execute(
            "DELETE FROM plugin_storage WHERE plugin_id = ?",
            params![plugin_id],
        )
        .map_err(|e| e.to_string())?;

        // 删除插件记录
        conn.execute(
            "DELETE FROM credential_provider_plugins WHERE id = ?",
            params![plugin_id],
        )
        .map_err(|e| e.to_string())?;
    } // conn 在此处释放

    // 从注册表卸载
    registry
        .uninstall_plugin(&plugin_id)
        .await
        .map_err(|e| e.to_string())?;

    info!("已卸载 OAuth 插件: {}", plugin_id);
    Ok(())
}

/// 检查插件更新
#[tauri::command]
pub async fn check_oauth_plugin_updates() -> Result<Vec<PluginUpdateResponse>, String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    let updates = registry.check_updates().await.map_err(|e| e.to_string())?;

    Ok(updates
        .into_iter()
        .map(|u| PluginUpdateResponse {
            plugin_id: u.plugin_id,
            current_version: u.current_version,
            latest_version: u.latest_version,
            changelog: u.changelog,
        })
        .collect())
}

/// 更新插件
#[tauri::command]
pub async fn update_oauth_plugin(_plugin_id: String) -> Result<(), String> {
    // TODO: 实现插件更新逻辑
    Err("插件更新功能尚未实现".to_string())
}

/// 重新加载所有插件
#[tauri::command]
pub fn reload_oauth_plugins(
    _state: tauri::State<'_, OAuthPluginManagerState>,
) -> Result<(), String> {
    // TODO: 由于 DashMap 生命周期限制，暂时不支持热重载
    // 需要重启应用来重新加载插件
    info!("请重启应用以重新加载 OAuth 插件");
    Err("请重启应用以重新加载 OAuth 插件".to_string())
}

/// 获取插件配置
#[tauri::command]
pub async fn get_oauth_plugin_config(plugin_id: String) -> Result<serde_json::Value, String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    let state = registry
        .get_plugin_state(&plugin_id)
        .ok_or(format!("插件不存在: {}", plugin_id))?;

    Ok(state.config)
}

/// 更新插件配置
#[tauri::command]
pub async fn update_oauth_plugin_config(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    config: serde_json::Value,
) -> Result<(), String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    registry
        .update_plugin_config(&plugin_id, config.clone())
        .await
        .map_err(|e| e.to_string())?;

    // 更新数据库
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE credential_provider_plugins SET config = ?, updated_at = ? WHERE id = ?",
        params![
            config.to_string(),
            chrono::Utc::now().to_rfc3339(),
            plugin_id
        ],
    )
    .map_err(|e| e.to_string())?;

    info!("已更新 OAuth 插件配置: {}", plugin_id);
    Ok(())
}

/// 扫描插件目录
#[tauri::command]
pub async fn scan_oauth_plugin_directory(
    state: tauri::State<'_, OAuthPluginManagerState>,
) -> Result<Vec<String>, String> {
    let loader = state.loader.read().await;

    let paths = loader.scan().await.map_err(|e| e.to_string())?;

    Ok(paths
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

// ============================================================================
// 插件凭证命令
// ============================================================================

/// 凭证信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInfoResponse {
    pub id: String,
    pub plugin_id: String,
    pub auth_type: String,
    pub display_name: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
    pub config: serde_json::Value,
}

/// 获取插件凭证列表
#[tauri::command]
pub async fn plugin_credential_list(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
) -> Result<PluginCredentialListResponse, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, plugin_id, auth_type, display_name, status,
                    config_encrypted, created_at, updated_at, last_used_at
             FROM plugin_credentials WHERE plugin_id = ?",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![plugin_id], |row| {
            Ok(CredentialInfoResponse {
                id: row.get(0)?,
                plugin_id: row.get(1)?,
                auth_type: row.get(2)?,
                display_name: row.get(3)?,
                status: row.get(4)?,
                config: serde_json::json!({}), // 不返回加密配置
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                last_used_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let credentials: Vec<CredentialInfoResponse> = rows.filter_map(|r| r.ok()).collect();

    Ok(PluginCredentialListResponse { credentials })
}

/// 凭证列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCredentialListResponse {
    pub credentials: Vec<CredentialInfoResponse>,
}

/// 获取单个凭证
#[tauri::command]
pub async fn plugin_credential_get(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    credential_id: String,
) -> Result<PluginCredentialGetResponse, String> {
    let result = plugin_credential_list(db, plugin_id).await?;
    let credential = result
        .credentials
        .into_iter()
        .find(|c| c.id == credential_id);
    Ok(PluginCredentialGetResponse { credential })
}

/// 单个凭证响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCredentialGetResponse {
    pub credential: Option<CredentialInfoResponse>,
}

/// 创建凭证
#[tauri::command]
pub async fn plugin_credential_create(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    auth_type: String,
    config: serde_json::Value,
) -> Result<String, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let credential_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // TODO: 加密配置
    let config_encrypted = config.to_string();

    conn.execute(
        "INSERT INTO plugin_credentials
         (id, plugin_id, auth_type, status, config_encrypted, created_at, updated_at)
         VALUES (?, ?, ?, 'active', ?, ?, ?)",
        params![
            credential_id,
            plugin_id,
            auth_type,
            config_encrypted,
            now,
            now
        ],
    )
    .map_err(|e| e.to_string())?;

    info!("已创建插件凭证: {} (插件: {})", credential_id, plugin_id);
    Ok(credential_id)
}

/// 更新凭证
#[tauri::command]
pub async fn plugin_credential_update(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    credential_id: String,
    config: serde_json::Value,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let config_encrypted = config.to_string();

    let affected = conn
        .execute(
            "UPDATE plugin_credentials SET config_encrypted = ?, updated_at = ?
             WHERE id = ? AND plugin_id = ?",
            params![config_encrypted, now, credential_id, plugin_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("凭证不存在: {}", credential_id));
    }

    info!("已更新插件凭证: {}", credential_id);
    Ok(())
}

/// 删除凭证
#[tauri::command]
pub async fn plugin_credential_delete(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    credential_id: String,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let affected = conn
        .execute(
            "DELETE FROM plugin_credentials WHERE id = ? AND plugin_id = ?",
            params![credential_id, plugin_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("凭证不存在: {}", credential_id));
    }

    info!("已删除插件凭证: {}", credential_id);
    Ok(())
}

/// 验证凭证
#[tauri::command]
pub async fn plugin_credential_validate(
    plugin_id: String,
    credential_id: String,
) -> Result<serde_json::Value, String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    let plugin = registry
        .get(&plugin_id)
        .ok_or(format!("插件不存在: {}", plugin_id))?;

    let result = plugin
        .validate_credential(&credential_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "valid": result.valid,
        "message": result.message
    }))
}

/// 刷新凭证
#[tauri::command]
pub async fn plugin_credential_refresh(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    credential_id: String,
) -> Result<(), String> {
    let registry = get_global_registry().ok_or("OAuth 插件系统未初始化")?;

    let plugin = registry
        .get(&plugin_id)
        .ok_or(format!("插件不存在: {}", plugin_id))?;

    plugin
        .refresh_token(&credential_id)
        .await
        .map_err(|e| e.to_string())?;

    // 更新最后使用时间
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE plugin_credentials SET last_used_at = ? WHERE id = ?",
        params![chrono::Utc::now().to_rfc3339(), credential_id],
    )
    .map_err(|e| e.to_string())?;

    info!("已刷新插件凭证: {}", credential_id);
    Ok(())
}

// ============================================================================
// 插件 SDK 命令
// ============================================================================

/// 插件数据库查询
#[tauri::command]
pub async fn plugin_database_query(
    plugin_id: String,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    // 创建 SDK 上下文
    let context = PluginSdkContext::new(plugin_id.clone(), vec![PluginPermission::DatabaseRead]);

    // 执行查询
    let result = context
        .database_query(&sql, params)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(result).unwrap())
}

/// 插件数据库执行
#[tauri::command]
pub async fn plugin_database_execute(
    plugin_id: String,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let context = PluginSdkContext::new(plugin_id.clone(), vec![PluginPermission::DatabaseWrite]);

    let affected = context
        .database_execute(&sql, params)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "affected": affected }))
}

/// 插件 HTTP 请求
#[tauri::command]
pub async fn plugin_http_request(
    plugin_id: String,
    url: String,
    method: String,
    headers: std::collections::HashMap<String, String>,
    body: Option<String>,
    timeout_ms: u64,
) -> Result<serde_json::Value, String> {
    use crate::credential::HttpRequestOptions;

    let context = PluginSdkContext::new(plugin_id.clone(), vec![PluginPermission::HttpRequest]);

    let options = HttpRequestOptions {
        method,
        headers,
        body,
        timeout_ms,
    };

    let response = context
        .http_request(&url, options)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(response).unwrap())
}

/// 插件加密
#[tauri::command]
pub async fn plugin_crypto_encrypt(
    plugin_id: String,
    data: String,
) -> Result<serde_json::Value, String> {
    let context = PluginSdkContext::new(plugin_id.clone(), vec![PluginPermission::CryptoEncrypt]);

    let encrypted = context
        .crypto_encrypt(&data)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "encrypted": encrypted }))
}

/// 插件解密
#[tauri::command]
pub async fn plugin_crypto_decrypt(
    plugin_id: String,
    data: String,
) -> Result<serde_json::Value, String> {
    let context = PluginSdkContext::new(plugin_id.clone(), vec![PluginPermission::CryptoDecrypt]);

    let decrypted = context
        .crypto_decrypt(&data)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "decrypted": decrypted }))
}

/// 插件通知
#[tauri::command]
pub async fn plugin_notification(
    plugin_id: String,
    level: String,
    message: String,
) -> Result<(), String> {
    let context = PluginSdkContext::new(plugin_id.clone(), vec![PluginPermission::Notification]);

    match level.as_str() {
        "success" => context
            .notification_success(&message)
            .map_err(|e| e.to_string())?,
        "error" => context
            .notification_error(&message)
            .map_err(|e| e.to_string())?,
        "info" => context
            .notification_info(&message)
            .map_err(|e| e.to_string())?,
        _ => context
            .notification_info(&message)
            .map_err(|e| e.to_string())?,
    }

    Ok(())
}

/// 插件存储获取
#[tauri::command]
pub async fn plugin_storage_get(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    key: String,
) -> Result<serde_json::Value, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM plugin_storage WHERE plugin_id = ? AND key = ?",
            params![plugin_id, key],
            |row| row.get(0),
        )
        .ok();

    Ok(serde_json::json!({ "value": value }))
}

/// 插件存储设置
#[tauri::command]
pub async fn plugin_storage_set(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    key: String,
    value: String,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO plugin_storage (plugin_id, key, value, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
        params![plugin_id, key, value, now, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 插件存储删除
#[tauri::command]
pub async fn plugin_storage_delete(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    key: String,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "DELETE FROM plugin_storage WHERE plugin_id = ? AND key = ?",
        params![plugin_id, key],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 插件存储键列表
#[tauri::command]
pub async fn plugin_storage_keys(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
) -> Result<serde_json::Value, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT key FROM plugin_storage WHERE plugin_id = ?")
        .map_err(|e| e.to_string())?;

    let keys: Vec<String> = stmt
        .query_map(params![plugin_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({ "keys": keys }))
}

/// 插件配置获取
#[tauri::command]
pub async fn plugin_config_get(plugin_id: String) -> Result<serde_json::Value, String> {
    get_oauth_plugin_config(plugin_id)
        .await
        .map(|c| serde_json::json!({ "config": c }))
}

/// 插件配置设置
#[tauri::command]
pub async fn plugin_config_set(
    db: tauri::State<'_, DbConnection>,
    plugin_id: String,
    config: serde_json::Value,
) -> Result<(), String> {
    update_oauth_plugin_config(db, plugin_id, config).await
}

/// 读取插件 UI 文件
/// 用于前端动态加载插件的 React 组件
#[tauri::command]
pub async fn read_plugin_ui_file(path: String) -> Result<String, String> {
    use std::fs;

    // 展开 ~ 为用户主目录
    let expanded_path = if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&path[2..])
        } else {
            std::path::PathBuf::from(&path)
        }
    } else {
        std::path::PathBuf::from(&path)
    };

    // 读取文件内容
    fs::read_to_string(&expanded_path)
        .map_err(|e| format!("读取插件 UI 文件失败: {} (路径: {:?})", e, expanded_path))
}
