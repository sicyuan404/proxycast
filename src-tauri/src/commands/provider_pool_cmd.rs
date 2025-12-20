//! Provider Pool Tauri 命令

use crate::credential::CredentialSyncService;
use crate::database::dao::provider_pool::ProviderPoolDao;
use crate::database::DbConnection;
use crate::models::provider_pool_model::{
    AddCredentialRequest, CredentialData, CredentialDisplay, HealthCheckResult, OAuthStatus,
    PoolProviderType, ProviderCredential, ProviderPoolOverview, UpdateCredentialRequest,
};
use crate::services::provider_pool_service::ProviderPoolService;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Emitter, State};
use uuid::Uuid;

pub struct ProviderPoolServiceState(pub Arc<ProviderPoolService>);

/// 凭证同步服务状态封装
pub struct CredentialSyncServiceState(pub Option<Arc<CredentialSyncService>>);

/// 展开路径中的 ~ 为用户主目录
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// 获取应用凭证存储目录
fn get_credentials_dir() -> Result<PathBuf, String> {
    let app_data_dir = dirs::data_dir()
        .ok_or_else(|| "无法获取应用数据目录".to_string())?
        .join("proxycast")
        .join("credentials");

    // 确保目录存在
    if !app_data_dir.exists() {
        fs::create_dir_all(&app_data_dir).map_err(|e| format!("创建凭证存储目录失败: {}", e))?;
    }

    Ok(app_data_dir)
}

/// 复制并重命名 OAuth 凭证文件
///
/// 对于 Kiro 凭证，会自动合并 clientIdHash 文件中的 client_id/client_secret，
/// 使副本文件完全独立，支持多账号场景。
fn copy_and_rename_credential_file(
    source_path: &str,
    provider_type: &str,
) -> Result<String, String> {
    let expanded_source = expand_tilde(source_path);
    let source = Path::new(&expanded_source);

    // 验证源文件存在
    if !source.exists() {
        return Err(format!("凭证文件不存在: {}", expanded_source));
    }

    // 生成新的文件名：{provider_type}_{uuid}_{timestamp}.json
    let uuid = Uuid::new_v4().to_string();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let new_filename = format!(
        "{}_{}_{}_{}.json",
        provider_type,
        &uuid[..8], // 使用 UUID 前8位
        timestamp,
        provider_type
    );

    // 获取目标目录
    let credentials_dir = get_credentials_dir()?;
    let target_path = credentials_dir.join(&new_filename);

    // 对于 Kiro 凭证，需要合并 clientIdHash 文件中的 client_id/client_secret
    if provider_type == "kiro" {
        let content =
            fs::read_to_string(&source).map_err(|e| format!("读取凭证文件失败: {}", e))?;
        let mut creds: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("解析凭证文件失败: {}", e))?;

        // 检测 refreshToken 是否被截断（仅记录警告，不阻止添加）
        // 正常的 refreshToken 长度应该在 500+ 字符，如果小于 100 字符则可能被截断
        // 注意：即使 refreshToken 被截断，也允许添加凭证，在刷新时才会提示错误
        if let Some(refresh_token) = creds.get("refreshToken").and_then(|v| v.as_str()) {
            let token_len = refresh_token.len();

            // 检测常见的截断模式
            let is_truncated =
                token_len < 100 || refresh_token.ends_with("...") || refresh_token.contains("...");

            if is_truncated {
                tracing::warn!(
                    "[KIRO] 检测到 refreshToken 可能被截断！长度: {}, 内容: {}... (仍允许添加，刷新时会提示)",
                    token_len,
                    &refresh_token[..std::cmp::min(50, token_len)]
                );
                // 不再阻止添加，只记录警告
                // 在刷新 Token 时会检测并提示用户
            } else {
                tracing::info!("[KIRO] refreshToken 长度检查通过: {} 字符", token_len);
            }
        } else {
            tracing::warn!("[KIRO] 凭证文件中没有 refreshToken 字段");
        }

        let aws_sso_cache_dir = dirs::home_dir()
            .ok_or_else(|| "无法获取用户主目录".to_string())?
            .join(".aws")
            .join("sso")
            .join("cache");

        // 尝试从 clientIdHash 文件或扫描目录获取 client_id/client_secret
        let mut found_credentials = false;

        // 方式1：如果有 clientIdHash，读取对应文件
        if let Some(hash) = creds.get("clientIdHash").and_then(|v| v.as_str()) {
            let hash_file_path = aws_sso_cache_dir.join(format!("{}.json", hash));

            if hash_file_path.exists() {
                if let Ok(hash_content) = fs::read_to_string(&hash_file_path) {
                    if let Ok(hash_json) = serde_json::from_str::<serde_json::Value>(&hash_content)
                    {
                        if let Some(client_id) = hash_json.get("clientId") {
                            creds["clientId"] = client_id.clone();
                        }
                        if let Some(client_secret) = hash_json.get("clientSecret") {
                            creds["clientSecret"] = client_secret.clone();
                        }
                        if creds.get("clientId").is_some() && creds.get("clientSecret").is_some() {
                            found_credentials = true;
                            tracing::info!(
                                "[KIRO] 已从 clientIdHash 文件合并 client_id/client_secret 到副本"
                            );
                        }
                    }
                }
            }
        }

        // 方式2：如果没有 clientIdHash 或未找到，扫描目录中的其他 JSON 文件
        if !found_credentials && aws_sso_cache_dir.exists() {
            tracing::info!(
                "[KIRO] 没有 clientIdHash 或未找到，扫描目录查找 client_id/client_secret"
            );
            if let Ok(entries) = fs::read_dir(&aws_sso_cache_dir) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    // 跳过主凭证文件和备份文件
                    if file_path.extension().map(|e| e == "json").unwrap_or(false) {
                        let file_name =
                            file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if file_name.starts_with("kiro-auth-token") {
                            continue;
                        }
                        if let Ok(file_content) = fs::read_to_string(&file_path) {
                            if let Ok(file_json) =
                                serde_json::from_str::<serde_json::Value>(&file_content)
                            {
                                let has_client_id =
                                    file_json.get("clientId").and_then(|v| v.as_str()).is_some();
                                let has_client_secret = file_json
                                    .get("clientSecret")
                                    .and_then(|v| v.as_str())
                                    .is_some();
                                if has_client_id && has_client_secret {
                                    creds["clientId"] = file_json["clientId"].clone();
                                    creds["clientSecret"] = file_json["clientSecret"].clone();
                                    found_credentials = true;
                                    tracing::info!(
                                        "[KIRO] 已从 {} 合并 client_id/client_secret 到副本",
                                        file_name
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if !found_credentials {
            // 检查认证方式
            let auth_method = creds
                .get("authMethod")
                .and_then(|v| v.as_str())
                .unwrap_or("social");

            if auth_method.to_lowercase() == "idc" {
                // IdC 认证必须有 clientId/clientSecret
                tracing::error!(
                    "[KIRO] IdC 认证方式缺少 clientId/clientSecret，无法创建有效的凭证副本"
                );
                return Err(format!(
                    "IdC 认证凭证不完整：缺少 clientId/clientSecret。\n\n💡 解决方案：\n1. 确保 ~/.aws/sso/cache/ 目录下有对应的 clientIdHash 文件\n2. 如果使用 AWS IAM Identity Center，请确保已完成完整的 SSO 登录流程\n3. 或者尝试使用 Social 认证方式的凭证"
                ));
            } else {
                tracing::warn!("[KIRO] 未找到 client_id/client_secret，将使用 social 认证方式");
            }
        }

        // 写入合并后的凭证到副本文件
        let merged_content =
            serde_json::to_string_pretty(&creds).map_err(|e| format!("序列化凭证失败: {}", e))?;
        fs::write(&target_path, merged_content).map_err(|e| format!("写入凭证文件失败: {}", e))?;
    } else {
        // 其他类型直接复制
        fs::copy(&source, &target_path).map_err(|e| format!("复制凭证文件失败: {}", e))?;
    }

    // 返回新的文件路径
    Ok(target_path.to_string_lossy().to_string())
}

/// 删除凭证文件（如果在应用存储目录中）
fn cleanup_credential_file(file_path: &str) -> Result<(), String> {
    let path = Path::new(file_path);

    // 只删除在应用凭证存储目录中的文件
    if let Ok(credentials_dir) = get_credentials_dir() {
        if let Ok(canonical_path) = path.canonicalize() {
            if let Ok(canonical_dir) = credentials_dir.canonicalize() {
                if canonical_path.starts_with(canonical_dir) {
                    if let Err(e) = fs::remove_file(&canonical_path) {
                        // 只记录警告，不中断删除过程
                        println!("Warning: Failed to delete credential file: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

/// 获取凭证池概览
#[tauri::command]
pub fn get_provider_pool_overview(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
) -> Result<Vec<ProviderPoolOverview>, String> {
    pool_service.0.get_overview(&db)
}

/// 获取指定类型的凭证列表
#[tauri::command]
pub fn get_provider_pool_credentials(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    provider_type: String,
) -> Result<Vec<CredentialDisplay>, String> {
    pool_service.0.get_by_type(&db, &provider_type)
}

/// 添加凭证
///
/// 添加凭证到数据库，并同步到 YAML 配置文件
/// Requirements: 1.1, 1.2
#[tauri::command]
pub fn add_provider_pool_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    sync_service: State<'_, CredentialSyncServiceState>,
    request: AddCredentialRequest,
) -> Result<ProviderCredential, String> {
    // 添加到数据库
    let credential = pool_service.0.add_credential(
        &db,
        &request.provider_type,
        request.credential,
        request.name,
        request.check_health,
        request.check_model_name,
    )?;

    // 同步到 YAML 配置（如果同步服务可用）
    if let Some(ref sync) = sync_service.0 {
        if let Err(e) = sync.add_credential(&credential) {
            // 记录警告但不中断操作
            tracing::warn!("同步凭证到 YAML 失败: {}", e);
        }
    }

    Ok(credential)
}

/// 更新凭证
/// 更新凭证
///
/// 更新数据库中的凭证，并同步到 YAML 配置文件
/// Requirements: 1.1, 1.2
#[tauri::command]
pub fn update_provider_pool_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    sync_service: State<'_, CredentialSyncServiceState>,
    uuid: String,
    request: UpdateCredentialRequest,
) -> Result<ProviderCredential, String> {
    tracing::info!(
        "[UPDATE_CREDENTIAL] 收到更新请求: uuid={}, name={:?}, check_model_name={:?}, not_supported_models={:?}",
        uuid,
        request.name,
        request.check_model_name,
        request.not_supported_models
    );
    // 如果需要重新上传文件，先处理文件上传
    let credential = if let Some(new_file_path) = request.new_creds_file_path {
        // 获取当前凭证以确定类型
        let conn = db.lock().map_err(|e| e.to_string())?;
        let current_credential = ProviderPoolDao::get_by_uuid(&conn, &uuid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("凭证不存在: {}", uuid))?;

        // 根据凭证类型复制新文件
        let new_stored_path = match &current_credential.credential {
            CredentialData::KiroOAuth { creds_file_path } => {
                // 清理旧文件
                cleanup_credential_file(creds_file_path)?;
                copy_and_rename_credential_file(&new_file_path, "kiro")?
            }
            CredentialData::GeminiOAuth {
                creds_file_path, ..
            } => {
                // 清理旧文件
                cleanup_credential_file(creds_file_path)?;
                copy_and_rename_credential_file(&new_file_path, "gemini")?
            }
            CredentialData::QwenOAuth { creds_file_path } => {
                // 清理旧文件
                cleanup_credential_file(creds_file_path)?;
                copy_and_rename_credential_file(&new_file_path, "qwen")?
            }
            CredentialData::AntigravityOAuth {
                creds_file_path, ..
            } => {
                // 清理旧文件
                cleanup_credential_file(creds_file_path)?;
                copy_and_rename_credential_file(&new_file_path, "antigravity")?
            }
            _ => {
                return Err("只有 OAuth 凭证支持重新上传文件".to_string());
            }
        };

        // 更新凭证数据
        let mut updated_cred = current_credential;

        // 更新凭证数据中的文件路径
        match &mut updated_cred.credential {
            CredentialData::KiroOAuth { creds_file_path } => {
                *creds_file_path = new_stored_path;
            }
            CredentialData::GeminiOAuth {
                creds_file_path,
                project_id,
            } => {
                *creds_file_path = new_stored_path;
                if let Some(new_pid) = request.new_project_id {
                    *project_id = Some(new_pid);
                }
            }
            CredentialData::QwenOAuth { creds_file_path } => {
                *creds_file_path = new_stored_path;
            }
            CredentialData::AntigravityOAuth {
                creds_file_path,
                project_id,
            } => {
                *creds_file_path = new_stored_path;
                if let Some(new_pid) = request.new_project_id {
                    *project_id = Some(new_pid);
                }
            }
            _ => {}
        }

        // 应用其他更新
        // 处理 name：空字符串表示清除，None 表示不修改
        if let Some(name) = request.name {
            updated_cred.name = if name.is_empty() { None } else { Some(name) };
        }
        if let Some(is_disabled) = request.is_disabled {
            updated_cred.is_disabled = is_disabled;
        }
        if let Some(check_health) = request.check_health {
            updated_cred.check_health = check_health;
        }
        // 处理 check_model_name：空字符串表示清除，None 表示不修改
        if let Some(check_model_name) = request.check_model_name {
            updated_cred.check_model_name = if check_model_name.is_empty() {
                None
            } else {
                Some(check_model_name)
            };
        }
        if let Some(not_supported_models) = request.not_supported_models {
            updated_cred.not_supported_models = not_supported_models;
        }

        updated_cred.updated_at = Utc::now();

        // 保存到数据库
        ProviderPoolDao::update(&conn, &updated_cred).map_err(|e| e.to_string())?;

        updated_cred
    } else if request.new_base_url.is_some() || request.new_api_key.is_some() {
        // 更新 API Key 凭证的 api_key 和/或 base_url
        let conn = db.lock().map_err(|e| e.to_string())?;
        let mut current_credential = ProviderPoolDao::get_by_uuid(&conn, &uuid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("凭证不存在: {}", uuid))?;

        // 更新 api_key 和 base_url
        match &mut current_credential.credential {
            CredentialData::OpenAIKey { api_key, base_url } => {
                if let Some(new_key) = request.new_api_key {
                    if !new_key.is_empty() {
                        *api_key = new_key;
                    }
                }
                if let Some(new_url) = request.new_base_url {
                    *base_url = if new_url.is_empty() {
                        None
                    } else {
                        Some(new_url)
                    };
                }
            }
            CredentialData::ClaudeKey { api_key, base_url } => {
                if let Some(new_key) = request.new_api_key {
                    if !new_key.is_empty() {
                        *api_key = new_key;
                    }
                }
                if let Some(new_url) = request.new_base_url {
                    *base_url = if new_url.is_empty() {
                        None
                    } else {
                        Some(new_url)
                    };
                }
            }
            _ => {
                return Err("只有 API Key 凭证支持修改 API Key 和 Base URL".to_string());
            }
        }

        // 应用其他更新
        // 处理 name：空字符串表示清除，None 表示不修改
        if let Some(name) = request.name {
            current_credential.name = if name.is_empty() { None } else { Some(name) };
        }
        if let Some(is_disabled) = request.is_disabled {
            current_credential.is_disabled = is_disabled;
        }
        if let Some(check_health) = request.check_health {
            current_credential.check_health = check_health;
        }
        // 处理 check_model_name：空字符串表示清除，None 表示不修改
        if let Some(check_model_name) = request.check_model_name {
            current_credential.check_model_name = if check_model_name.is_empty() {
                None
            } else {
                Some(check_model_name)
            };
        }
        if let Some(not_supported_models) = request.not_supported_models {
            current_credential.not_supported_models = not_supported_models;
        }

        current_credential.updated_at = Utc::now();

        // 保存到数据库
        ProviderPoolDao::update(&conn, &current_credential).map_err(|e| e.to_string())?;

        current_credential
    } else {
        // 常规更新，不涉及文件
        pool_service.0.update_credential(
            &db,
            &uuid,
            request.name,
            request.is_disabled,
            request.check_health,
            request.check_model_name,
            request.not_supported_models,
        )?
    };

    // 同步到 YAML 配置（如果同步服务可用）
    if let Some(ref sync) = sync_service.0 {
        if let Err(e) = sync.update_credential(&credential) {
            // 记录警告但不中断操作
            tracing::warn!("同步凭证更新到 YAML 失败: {}", e);
        }
    }

    Ok(credential)
}

/// 删除凭证
/// 删除凭证
///
/// 从数据库删除凭证，并同步到 YAML 配置文件
/// Requirements: 1.1, 1.2
#[tauri::command]
pub fn delete_provider_pool_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    sync_service: State<'_, CredentialSyncServiceState>,
    uuid: String,
    provider_type: Option<String>,
) -> Result<bool, String> {
    // 从数据库删除
    let result = pool_service.0.delete_credential(&db, &uuid)?;

    // 同步到 YAML 配置（如果同步服务可用且提供了 provider_type）
    if let Some(ref sync) = sync_service.0 {
        if let Some(pt) = provider_type {
            if let Ok(pool_type) = pt.parse::<PoolProviderType>() {
                if let Err(e) = sync.remove_credential(pool_type, &uuid) {
                    // 记录警告但不中断操作
                    tracing::warn!("从 YAML 删除凭证失败: {}", e);
                }
            }
        }
    }

    Ok(result)
}

/// 切换凭证启用/禁用状态
#[tauri::command]
pub fn toggle_provider_pool_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    uuid: String,
    is_disabled: bool,
) -> Result<ProviderCredential, String> {
    pool_service
        .0
        .update_credential(&db, &uuid, None, Some(is_disabled), None, None, None)
}

/// 重置凭证计数器
#[tauri::command]
pub fn reset_provider_pool_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    uuid: String,
) -> Result<(), String> {
    pool_service.0.reset_counters(&db, &uuid)
}

/// 重置指定类型的所有凭证健康状态
#[tauri::command]
pub fn reset_provider_pool_health(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    provider_type: String,
) -> Result<usize, String> {
    pool_service.0.reset_health_by_type(&db, &provider_type)
}

/// 执行单个凭证的健康检查
#[tauri::command]
pub async fn check_provider_pool_credential_health(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    uuid: String,
) -> Result<HealthCheckResult, String> {
    tracing::info!("[DEBUG] 开始健康检查 for uuid: {}", uuid);
    let result = pool_service.0.check_credential_health(&db, &uuid).await;
    match &result {
        Ok(health) => tracing::info!(
            "[DEBUG] 健康检查完成: success={}, message={:?}",
            health.success,
            health.message
        ),
        Err(err) => tracing::error!("[DEBUG] 健康检查失败: {}", err),
    }
    result
}

/// 执行指定类型的所有凭证健康检查
#[tauri::command]
pub async fn check_provider_pool_type_health(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    provider_type: String,
) -> Result<Vec<HealthCheckResult>, String> {
    pool_service.0.check_type_health(&db, &provider_type).await
}

/// 添加 Kiro OAuth 凭证（通过文件路径）
#[tauri::command]
pub fn add_kiro_oauth_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "kiro")?;

    pool_service.0.add_credential(
        &db,
        "kiro",
        CredentialData::KiroOAuth {
            creds_file_path: stored_file_path,
        },
        name,
        Some(true),
        None,
    )
}

/// 添加 Gemini OAuth 凭证（通过文件路径）
#[tauri::command]
pub fn add_gemini_oauth_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    project_id: Option<String>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "gemini")?;

    pool_service.0.add_credential(
        &db,
        "gemini",
        CredentialData::GeminiOAuth {
            creds_file_path: stored_file_path,
            project_id,
        },
        name,
        Some(true),
        None,
    )
}

/// 添加 Qwen OAuth 凭证（通过文件路径）
#[tauri::command]
pub fn add_qwen_oauth_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "qwen")?;

    pool_service.0.add_credential(
        &db,
        "qwen",
        CredentialData::QwenOAuth {
            creds_file_path: stored_file_path,
        },
        name,
        Some(true),
        None,
    )
}

/// 添加 Antigravity OAuth 凭证（通过文件路径）
#[tauri::command]
pub fn add_antigravity_oauth_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    project_id: Option<String>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "antigravity")?;

    pool_service.0.add_credential(
        &db,
        "antigravity",
        CredentialData::AntigravityOAuth {
            creds_file_path: stored_file_path,
            project_id,
        },
        name,
        Some(true),
        None,
    )
}

/// 添加 OpenAI API Key 凭证
#[tauri::command]
pub fn add_openai_key_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    api_key: String,
    base_url: Option<String>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    pool_service.0.add_credential(
        &db,
        "openai",
        CredentialData::OpenAIKey { api_key, base_url },
        name,
        Some(true),
        None,
    )
}

/// 添加 Claude API Key 凭证
#[tauri::command]
pub fn add_claude_key_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    api_key: String,
    base_url: Option<String>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    pool_service.0.add_credential(
        &db,
        "claude",
        CredentialData::ClaudeKey { api_key, base_url },
        name,
        Some(true),
        None,
    )
}

/// 添加 Codex OAuth 凭证（通过文件路径）
#[tauri::command]
pub fn add_codex_oauth_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "codex")?;

    pool_service.0.add_credential(
        &db,
        "codex",
        CredentialData::CodexOAuth {
            creds_file_path: stored_file_path,
        },
        name,
        Some(true),
        None,
    )
}

/// 添加 Claude OAuth 凭证（通过文件路径）
#[tauri::command]
pub fn add_claude_oauth_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "claude_oauth")?;

    pool_service.0.add_credential(
        &db,
        "claude_oauth",
        CredentialData::ClaudeOAuth {
            creds_file_path: stored_file_path,
        },
        name,
        Some(true),
        None,
    )
}

/// 添加 iFlow OAuth 凭证（通过文件路径）
#[tauri::command]
pub fn add_iflow_oauth_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "iflow")?;

    pool_service.0.add_credential(
        &db,
        "iflow",
        CredentialData::IFlowOAuth {
            creds_file_path: stored_file_path,
        },
        name,
        Some(true),
        None,
    )
}

/// 添加 iFlow Cookie 凭证（通过文件路径）
#[tauri::command]
pub fn add_iflow_cookie_credential(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    creds_file_path: String,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    // 复制并重命名文件到应用存储目录
    let stored_file_path = copy_and_rename_credential_file(&creds_file_path, "iflow_cookie")?;

    pool_service.0.add_credential(
        &db,
        "iflow",
        CredentialData::IFlowCookie {
            creds_file_path: stored_file_path,
        },
        name,
        Some(true),
        None,
    )
}

/// 刷新凭证的 OAuth Token
#[tauri::command]
pub async fn refresh_pool_credential_token(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    uuid: String,
) -> Result<String, String> {
    tracing::info!("[DEBUG] 开始刷新 Token for uuid: {}", uuid);
    let result = pool_service.0.refresh_credential_token(&db, &uuid).await;
    match &result {
        Ok(msg) => tracing::info!("[DEBUG] Token 刷新成功: {}", msg),
        Err(err) => tracing::error!("[DEBUG] Token 刷新失败: {}", err),
    }
    result
}

/// 获取凭证的 OAuth 状态
#[tauri::command]
pub fn get_pool_credential_oauth_status(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    uuid: String,
) -> Result<OAuthStatus, String> {
    pool_service.0.get_credential_oauth_status(&db, &uuid)
}

/// 调试 Kiro 凭证加载（从默认路径）
#[tauri::command]
pub async fn debug_kiro_credentials() -> Result<String, String> {
    use crate::providers::kiro::KiroProvider;

    let mut provider = KiroProvider::new();

    let mut result = String::new();
    result.push_str("🔍 开始 Kiro 凭证调试 (默认路径)...\n\n");

    match provider.load_credentials().await {
        Ok(_) => {
            result.push_str("✅ 凭证加载成功!\n");
            result.push_str(&format!(
                "📄 认证方式: {:?}\n",
                provider.credentials.auth_method
            ));
            result.push_str(&format!(
                "🔑 有 client_id: {}\n",
                provider.credentials.client_id.is_some()
            ));
            result.push_str(&format!(
                "🔒 有 client_secret: {}\n",
                provider.credentials.client_secret.is_some()
            ));
            result.push_str(&format!(
                "🏷️  有 clientIdHash: {}\n",
                provider.credentials.client_id_hash.is_some()
            ));

            if let Some(hash) = &provider.credentials.client_id_hash {
                result.push_str(&format!("🔗 clientIdHash: {}\n", hash));
            }

            let detected_method = provider.detect_auth_method();
            result.push_str(&format!("🎯 检测到的认证方式: {}\n", detected_method));

            let refresh_url = provider.get_refresh_url();
            result.push_str(&format!("🌐 刷新端点: {}\n", refresh_url));

            if let Some(client_id) = &provider.credentials.client_id {
                result.push_str(&format!(
                    "🆔 client_id 前缀: {}...\n",
                    &client_id[..std::cmp::min(20, client_id.len())]
                ));
            }

            result.push_str("\n🚀 尝试刷新 token...\n");
            match provider.refresh_token().await {
                Ok(token) => {
                    result.push_str(&format!("✅ Token 刷新成功! Token 长度: {}\n", token.len()));
                    result.push_str(&format!(
                        "🎫 Token 前缀: {}...\n",
                        &token[..std::cmp::min(50, token.len())]
                    ));
                }
                Err(e) => {
                    result.push_str(&format!("❌ Token 刷新失败: {}\n", e));
                }
            }
        }
        Err(e) => {
            result.push_str(&format!("❌ 凭证加载失败: {}\n", e));
        }
    }

    Ok(result)
}

/// 测试用户上传的凭证文件
#[tauri::command]
pub async fn test_user_credentials() -> Result<String, String> {
    use crate::providers::kiro::KiroProvider;

    let mut result = String::new();
    result.push_str("🧪 测试用户上传的凭证文件...\n\n");

    // 测试用户上传的凭证文件路径
    let user_creds_path = dirs::home_dir()
        .ok_or("无法获取用户主目录".to_string())?
        .join(
            "Library/Application Support/proxycast/credentials/kiro_d8da9d58_1765757992_kiro.json",
        );

    result.push_str(&format!("📂 用户凭证路径: {}\n", user_creds_path.display()));

    // 检查文件是否存在
    if !user_creds_path.exists() {
        result.push_str("❌ 用户凭证文件不存在!\n");
        result.push_str("💡 请确保文件路径正确，或重新上传凭证文件\n");
        return Ok(result);
    }

    result.push_str("✅ 用户凭证文件存在\n\n");

    // 读取并解析用户凭证文件
    match std::fs::read_to_string(&user_creds_path) {
        Ok(content) => {
            result.push_str("✅ 成功读取凭证文件\n");
            result.push_str(&format!("📄 文件大小: {} 字节\n", content.len()));

            // 尝试解析 JSON
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => {
                    result.push_str("✅ JSON 格式有效\n");

                    // 检查关键字段
                    let has_access_token =
                        json.get("accessToken").and_then(|v| v.as_str()).is_some();
                    let has_refresh_token =
                        json.get("refreshToken").and_then(|v| v.as_str()).is_some();
                    let auth_method = json.get("authMethod").and_then(|v| v.as_str());
                    let client_id_hash = json.get("clientIdHash").and_then(|v| v.as_str());
                    let region = json.get("region").and_then(|v| v.as_str());

                    result.push_str(&format!("🔑 有 accessToken: {}\n", has_access_token));
                    result.push_str(&format!("🔄 有 refreshToken: {}\n", has_refresh_token));
                    result.push_str(&format!("📄 authMethod: {:?}\n", auth_method));
                    result.push_str(&format!("🏷️ clientIdHash: {:?}\n", client_id_hash));
                    result.push_str(&format!("🌍 region: {:?}\n", region));

                    if let Some(hash) = client_id_hash {
                        // 检查 clientIdHash 对应的文件
                        let hash_file_path = dirs::home_dir()
                            .unwrap()
                            .join(".aws/sso/cache")
                            .join(format!("{}.json", hash));

                        result.push_str(&format!(
                            "\n🔗 检查 clientIdHash 文件: {}\n",
                            hash_file_path.display()
                        ));

                        if hash_file_path.exists() {
                            result.push_str("✅ clientIdHash 文件存在\n");

                            match std::fs::read_to_string(&hash_file_path) {
                                Ok(hash_content) => {
                                    match serde_json::from_str::<serde_json::Value>(&hash_content) {
                                        Ok(hash_json) => {
                                            let has_client_id = hash_json
                                                .get("clientId")
                                                .and_then(|v| v.as_str())
                                                .is_some();
                                            let has_client_secret = hash_json
                                                .get("clientSecret")
                                                .and_then(|v| v.as_str())
                                                .is_some();

                                            result.push_str(&format!(
                                                "🆔 hash 文件有 clientId: {}\n",
                                                has_client_id
                                            ));
                                            result.push_str(&format!(
                                                "🔒 hash 文件有 clientSecret: {}\n",
                                                has_client_secret
                                            ));

                                            if has_client_id && has_client_secret {
                                                result.push_str("✅ IdC 认证配置完整!\n");
                                            } else {
                                                result.push_str(
                                                    "⚠️ IdC 认证配置不完整，将使用 social 认证\n",
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            result.push_str(&format!(
                                                "❌ 无法解析 hash 文件 JSON: {}\n",
                                                e
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    result.push_str(&format!("❌ 无法读取 hash 文件: {}\n", e));
                                }
                            }
                        } else {
                            result.push_str("❌ clientIdHash 文件不存在\n");
                        }
                    }

                    // 现在使用我们的 KiroProvider 来测试加载
                    result.push_str("\n🔧 使用 KiroProvider 测试加载...\n");

                    let mut provider = KiroProvider::new();
                    // 设置凭证路径到用户文件
                    provider.creds_path = Some(user_creds_path.clone());

                    match provider
                        .load_credentials_from_path(&user_creds_path.to_string_lossy())
                        .await
                    {
                        Ok(_) => {
                            result.push_str("✅ KiroProvider 加载成功!\n");
                            result.push_str(&format!(
                                "📄 最终认证方式: {:?}\n",
                                provider.credentials.auth_method
                            ));
                            result.push_str(&format!(
                                "🔑 最终有 client_id: {}\n",
                                provider.credentials.client_id.is_some()
                            ));
                            result.push_str(&format!(
                                "🔒 最终有 client_secret: {}\n",
                                provider.credentials.client_secret.is_some()
                            ));

                            let detected_method = provider.detect_auth_method();
                            result.push_str(&format!("🎯 检测到的认证方式: {}\n", detected_method));

                            let refresh_url = provider.get_refresh_url();
                            result.push_str(&format!("🌐 刷新端点: {}\n", refresh_url));

                            result.push_str("\n🚀 尝试刷新 token...\n");
                            match provider.refresh_token().await {
                                Ok(token) => {
                                    result.push_str(&format!(
                                        "✅ Token 刷新成功! Token 长度: {}\n",
                                        token.len()
                                    ));
                                    result.push_str(&format!(
                                        "🎫 Token 前缀: {}...\n",
                                        &token[..std::cmp::min(50, token.len())]
                                    ));
                                }
                                Err(e) => {
                                    result.push_str(&format!("❌ Token 刷新失败: {}\n", e));
                                }
                            }
                        }
                        Err(e) => {
                            result.push_str(&format!("❌ KiroProvider 加载失败: {}\n", e));
                        }
                    }
                }
                Err(e) => {
                    result.push_str(&format!("❌ JSON 格式无效: {}\n", e));
                }
            }
        }
        Err(e) => {
            result.push_str(&format!("❌ 无法读取凭证文件: {}\n", e));
        }
    }

    Ok(result)
}

/// 迁移 Private 配置到凭证池
///
/// 从 providers 配置中读取单个凭证配置，迁移到凭证池中并标记为 Private 来源
/// Requirements: 6.4
#[tauri::command]
pub fn migrate_private_config_to_pool(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    config: crate::config::Config,
) -> Result<MigrationResultResponse, String> {
    let result = pool_service.0.migrate_private_config(&db, &config)?;
    Ok(MigrationResultResponse {
        migrated_count: result.migrated_count,
        skipped_count: result.skipped_count,
        errors: result.errors,
    })
}

/// 迁移结果响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MigrationResultResponse {
    /// 成功迁移的凭证数量
    pub migrated_count: usize,
    /// 跳过的凭证数量（已存在）
    pub skipped_count: usize,
    /// 错误信息列表
    pub errors: Vec<String>,
}

/// 获取 Antigravity OAuth 授权 URL 并等待回调（不自动打开浏览器）
///
/// 启动服务器后通过事件发送授权 URL，然后等待回调
/// 成功后返回凭证
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AntigravityAuthUrlResponse {
    pub auth_url: String,
}

#[tauri::command]
pub async fn get_antigravity_auth_url_and_wait(
    app: tauri::AppHandle,
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
    skip_project_id_fetch: Option<bool>,
) -> Result<ProviderCredential, String> {
    use crate::providers::antigravity;

    tracing::info!("[Antigravity OAuth] 启动服务器并获取授权 URL");

    // 启动服务器并获取授权 URL
    let (auth_url, wait_future) =
        antigravity::start_oauth_server_and_get_url(skip_project_id_fetch.unwrap_or(false))
            .await
            .map_err(|e| format!("启动 OAuth 服务器失败: {}", e))?;

    tracing::info!("[Antigravity OAuth] 授权 URL: {}", auth_url);

    // 通过事件发送授权 URL 给前端
    let _ = app.emit(
        "antigravity-auth-url",
        AntigravityAuthUrlResponse {
            auth_url: auth_url.clone(),
        },
    );

    // 等待回调
    let result = wait_future.await.map_err(|e| e.to_string())?;

    tracing::info!(
        "[Antigravity OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 从凭证中获取 project_id
    let project_id = result.credentials.projectId.clone();

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "antigravity",
        CredentialData::AntigravityOAuth {
            creds_file_path: result.creds_file_path,
            project_id,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!(
        "[Antigravity OAuth] 凭证已添加到凭证池: {}",
        credential.uuid
    );

    Ok(credential)
}

/// 启动 Antigravity OAuth 登录流程
///
/// 打开浏览器让用户登录 Google 账号，获取 Antigravity 凭证
#[tauri::command]
pub async fn start_antigravity_oauth_login(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
    skip_project_id_fetch: Option<bool>,
) -> Result<ProviderCredential, String> {
    use crate::providers::antigravity;

    tracing::info!("[Antigravity OAuth] 开始 OAuth 登录流程");

    // 启动 OAuth 登录
    let result = antigravity::start_oauth_login(skip_project_id_fetch.unwrap_or(false))
        .await
        .map_err(|e| format!("Antigravity OAuth 登录失败: {}", e))?;

    tracing::info!(
        "[Antigravity OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 从凭证中获取 project_id
    let project_id = result.credentials.projectId.clone();

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "antigravity",
        CredentialData::AntigravityOAuth {
            creds_file_path: result.creds_file_path,
            project_id,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!(
        "[Antigravity OAuth] 凭证已添加到凭证池: {}",
        credential.uuid
    );

    Ok(credential)
}

/// Codex OAuth 授权 URL 响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodexAuthUrlResponse {
    pub auth_url: String,
}

/// 获取 Codex OAuth 授权 URL 并等待回调（不自动打开浏览器）
///
/// 启动服务器后通过事件发送授权 URL，然后等待回调
/// 成功后返回凭证
#[tauri::command]
pub async fn get_codex_auth_url_and_wait(
    app: tauri::AppHandle,
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::codex;

    tracing::info!("[Codex OAuth] 启动服务器并获取授权 URL");

    // 启动服务器并获取授权 URL
    let (auth_url, wait_future) = codex::start_codex_oauth_server_and_get_url()
        .await
        .map_err(|e| format!("启动 OAuth 服务器失败: {}", e))?;

    tracing::info!("[Codex OAuth] 授权 URL: {}", auth_url);

    // 通过事件发送授权 URL 给前端
    let _ = app.emit(
        "codex-auth-url",
        CodexAuthUrlResponse {
            auth_url: auth_url.clone(),
        },
    );

    // 等待回调
    let result = wait_future.await.map_err(|e| e.to_string())?;

    tracing::info!(
        "[Codex OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "codex",
        CredentialData::CodexOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Codex OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// 启动 Codex OAuth 登录流程
///
/// 打开浏览器让用户登录 OpenAI 账号，获取 Codex 凭证
#[tauri::command]
pub async fn start_codex_oauth_login(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::codex;

    tracing::info!("[Codex OAuth] 开始 OAuth 登录流程");

    // 启动 OAuth 登录
    let result = codex::start_codex_oauth_login()
        .await
        .map_err(|e| format!("Codex OAuth 登录失败: {}", e))?;

    tracing::info!(
        "[Codex OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "codex",
        CredentialData::CodexOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Codex OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// Claude OAuth 授权 URL 响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClaudeOAuthAuthUrlResponse {
    pub auth_url: String,
}

/// 获取 Claude OAuth 授权 URL 并等待回调（不自动打开浏览器）
///
/// 启动服务器后通过事件发送授权 URL，然后等待回调
/// 成功后返回凭证
#[tauri::command]
pub async fn get_claude_oauth_auth_url_and_wait(
    app: tauri::AppHandle,
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::claude_oauth;

    tracing::info!("[Claude OAuth] 启动服务器并获取授权 URL");

    // 启动服务器并获取授权 URL
    let (auth_url, wait_future) = claude_oauth::start_claude_oauth_server_and_get_url()
        .await
        .map_err(|e| format!("启动 OAuth 服务器失败: {}", e))?;

    tracing::info!("[Claude OAuth] 授权 URL: {}", auth_url);

    // 通过事件发送授权 URL 给前端
    let _ = app.emit(
        "claude-oauth-auth-url",
        ClaudeOAuthAuthUrlResponse {
            auth_url: auth_url.clone(),
        },
    );

    // 等待回调
    let result = wait_future.await.map_err(|e| e.to_string())?;

    tracing::info!(
        "[Claude OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "claude_oauth",
        CredentialData::ClaudeOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Claude OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// 启动 Claude OAuth 登录流程
///
/// 打开浏览器让用户登录 Claude 账号，获取凭证
#[tauri::command]
pub async fn start_claude_oauth_login(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::claude_oauth;

    tracing::info!("[Claude OAuth] 开始 OAuth 登录流程");

    // 启动 OAuth 登录
    let result = claude_oauth::start_claude_oauth_login()
        .await
        .map_err(|e| format!("Claude OAuth 登录失败: {}", e))?;

    tracing::info!(
        "[Claude OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "claude_oauth",
        CredentialData::ClaudeOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Claude OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// Qwen Device Code 响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QwenDeviceCodeResponse {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: i64,
}

/// 获取 Qwen Device Code 并等待用户授权
///
/// 启动 Device Code Flow 后通过事件发送设备码信息，然后轮询等待授权
/// 成功后返回凭证
#[tauri::command]
pub async fn get_qwen_device_code_and_wait(
    app: tauri::AppHandle,
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::qwen;

    tracing::info!("[Qwen] 启动 Device Code Flow");

    // 启动 Device Code Flow 并获取设备码信息
    let (device_response, wait_future) = qwen::start_qwen_device_code_and_get_info()
        .await
        .map_err(|e| format!("启动 Device Code Flow 失败: {}", e))?;

    tracing::info!(
        "[Qwen] Device Code: user_code={}, verification_uri={}",
        device_response.user_code,
        device_response.verification_uri
    );

    // 通过事件发送设备码信息给前端
    let _ = app.emit(
        "qwen-device-code",
        QwenDeviceCodeResponse {
            user_code: device_response.user_code.clone(),
            verification_uri: device_response.verification_uri.clone(),
            verification_uri_complete: device_response.verification_uri_complete.clone(),
            expires_in: device_response.expires_in,
        },
    );

    // 等待用户授权
    let result = wait_future.await.map_err(|e| e.to_string())?;

    tracing::info!("[Qwen] 登录成功，凭证保存到: {}", result.creds_file_path);

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "qwen",
        CredentialData::QwenOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Qwen] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// 启动 Qwen Device Code Flow 登录流程
///
/// 自动打开浏览器让用户完成授权
#[tauri::command]
pub async fn start_qwen_device_code_login(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::qwen;

    tracing::info!("[Qwen] 开始 Device Code Flow 登录流程");

    // 启动 Device Code Flow 登录
    let result = qwen::start_qwen_device_code_login()
        .await
        .map_err(|e| format!("Qwen Device Code Flow 登录失败: {}", e))?;

    tracing::info!("[Qwen] 登录成功，凭证保存到: {}", result.creds_file_path);

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "qwen",
        CredentialData::QwenOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Qwen] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// iFlow OAuth 授权 URL 响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IFlowAuthUrlResponse {
    pub auth_url: String,
}

/// 获取 iFlow OAuth 授权 URL 并等待回调（不自动打开浏览器）
///
/// 启动服务器后通过事件发送授权 URL，然后等待回调
/// 成功后返回凭证
#[tauri::command]
pub async fn get_iflow_auth_url_and_wait(
    app: tauri::AppHandle,
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::iflow;

    tracing::info!("[iFlow OAuth] 启动服务器并获取授权 URL");

    // 启动服务器并获取授权 URL
    let (auth_url, wait_future) = iflow::start_iflow_oauth_server_and_get_url()
        .await
        .map_err(|e| format!("启动 OAuth 服务器失败: {}", e))?;

    tracing::info!("[iFlow OAuth] 授权 URL: {}", auth_url);

    // 通过事件发送授权 URL 给前端
    let _ = app.emit(
        "iflow-auth-url",
        IFlowAuthUrlResponse {
            auth_url: auth_url.clone(),
        },
    );

    // 等待回调
    let result = wait_future.await.map_err(|e| e.to_string())?;

    tracing::info!(
        "[iFlow OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "iflow",
        CredentialData::IFlowOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[iFlow OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// 启动 iFlow OAuth 登录流程
///
/// 打开浏览器让用户登录 iFlow 账号，获取凭证
#[tauri::command]
pub async fn start_iflow_oauth_login(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::iflow;

    tracing::info!("[iFlow OAuth] 开始 OAuth 登录流程");

    // 启动 OAuth 登录
    let result = iflow::start_iflow_oauth_login()
        .await
        .map_err(|e| format!("iFlow OAuth 登录失败: {}", e))?;

    tracing::info!(
        "[iFlow OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "iflow",
        CredentialData::IFlowOAuth {
            creds_file_path: result.creds_file_path,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[iFlow OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// 获取 Kiro 凭证的 Machine ID 指纹信息
///
/// 返回凭证的唯一设备指纹，用于在 UI 中展示
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KiroFingerprintInfo {
    /// Machine ID（SHA256 哈希，64 字符）
    pub machine_id: String,
    /// Machine ID 的短格式（前 16 字符）
    pub machine_id_short: String,
    /// 指纹来源（profileArn / clientId / system）
    pub source: String,
    /// 认证方式
    pub auth_method: String,
}

#[tauri::command]
pub async fn get_kiro_credential_fingerprint(
    db: State<'_, DbConnection>,
    uuid: String,
) -> Result<KiroFingerprintInfo, String> {
    use crate::database::dao::provider_pool::ProviderPoolDao;
    use crate::providers::kiro::{generate_machine_id_from_credentials, KiroProvider};

    // 获取凭证文件路径（在锁释放前完成）
    let creds_file_path = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let credential = ProviderPoolDao::get_by_uuid(&conn, &uuid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("凭证不存在: {}", uuid))?;

        // 检查是否为 Kiro 凭证
        match &credential.credential {
            CredentialData::KiroOAuth { creds_file_path } => creds_file_path.clone(),
            _ => return Err("只有 Kiro 凭证支持获取指纹信息".to_string()),
        }
    }; // conn 在这里释放

    // 加载凭证文件（异步操作，锁已释放）
    let mut provider = KiroProvider::new();
    provider
        .load_credentials_from_path(&creds_file_path)
        .await
        .map_err(|e| format!("加载凭证失败: {}", e))?;

    // 确定指纹来源
    let (source, profile_arn, client_id) = if provider.credentials.profile_arn.is_some() {
        (
            "profileArn".to_string(),
            provider.credentials.profile_arn.as_deref(),
            None,
        )
    } else if provider.credentials.client_id.is_some() {
        (
            "clientId".to_string(),
            None,
            provider.credentials.client_id.as_deref(),
        )
    } else {
        ("system".to_string(), None, None)
    };

    // 生成 Machine ID
    let machine_id = generate_machine_id_from_credentials(profile_arn, client_id);
    let machine_id_short = machine_id[..16].to_string();

    // 获取认证方式
    let auth_method = provider
        .credentials
        .auth_method
        .clone()
        .unwrap_or_else(|| "social".to_string());

    Ok(KiroFingerprintInfo {
        machine_id,
        machine_id_short,
        source,
        auth_method,
    })
}

/// Gemini OAuth 授权 URL 响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeminiAuthUrlResponse {
    pub auth_url: String,
    pub session_id: String,
}

use once_cell::sync::Lazy;
/// Gemini OAuth 会话存储（用于存储 code_verifier）
use std::collections::HashMap;
use tokio::sync::RwLock;

static GEMINI_OAUTH_SESSIONS: Lazy<
    RwLock<HashMap<String, crate::providers::gemini::GeminiOAuthSession>>,
> = Lazy::new(|| RwLock::new(HashMap::new()));

/// 获取 Gemini OAuth 授权 URL（不等待回调）
///
/// 生成授权 URL 和 session_id，通过事件发送给前端
/// 用户需要手动复制授权码回来，然后调用 exchange_gemini_code
#[tauri::command]
pub async fn get_gemini_auth_url_and_wait(
    app: tauri::AppHandle,
    _db: State<'_, DbConnection>,
    _pool_service: State<'_, ProviderPoolServiceState>,
    _name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::gemini;

    tracing::info!("[Gemini OAuth] 生成授权 URL");

    // 生成授权 URL 和会话信息
    let (auth_url, session) = gemini::generate_gemini_auth_url_with_session();
    let session_id = session.session_id.clone();

    tracing::info!("[Gemini OAuth] 授权 URL: {}", auth_url);
    tracing::info!("[Gemini OAuth] Session ID: {}", session_id);

    // 存储会话信息（用于后续交换 token）
    {
        let mut sessions = GEMINI_OAUTH_SESSIONS.write().await;
        sessions.insert(session_id.clone(), session);

        // 清理过期的会话（超过 10 分钟）
        let now = chrono::Utc::now().timestamp();
        sessions.retain(|_, s| now - s.created_at < 600);
    }

    // 通过事件发送授权 URL 给前端
    let _ = app.emit(
        "gemini-auth-url",
        GeminiAuthUrlResponse {
            auth_url: auth_url.clone(),
            session_id: session_id.clone(),
        },
    );

    // 返回错误，让前端知道需要用户手动输入授权码
    // 这不是真正的错误，只是流程需要用户交互
    Err(format!("AUTH_URL:{}", auth_url))
}

/// 用 Gemini 授权码交换 Token 并添加凭证
#[tauri::command]
pub async fn exchange_gemini_code(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    code: String,
    session_id: Option<String>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::gemini;

    tracing::info!("[Gemini OAuth] 开始交换授权码");

    // 获取 code_verifier
    let code_verifier = if let Some(ref sid) = session_id {
        let sessions = GEMINI_OAUTH_SESSIONS.read().await;
        sessions
            .get(sid)
            .map(|s| s.code_verifier.clone())
            .ok_or_else(|| "会话已过期，请重新获取授权 URL".to_string())?
    } else {
        // 如果没有 session_id，尝试使用最近的会话
        let sessions = GEMINI_OAUTH_SESSIONS.read().await;
        sessions
            .values()
            .max_by_key(|s| s.created_at)
            .map(|s| s.code_verifier.clone())
            .ok_or_else(|| "没有可用的会话，请先获取授权 URL".to_string())?
    };

    // 交换 token 并创建凭证
    let result = gemini::exchange_gemini_code_and_create_credentials(&code, &code_verifier)
        .await
        .map_err(|e| format!("交换授权码失败: {}", e))?;

    tracing::info!(
        "[Gemini OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 清理使用过的会话
    if let Some(ref sid) = session_id {
        let mut sessions = GEMINI_OAUTH_SESSIONS.write().await;
        sessions.remove(sid);
    }

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "gemini",
        CredentialData::GeminiOAuth {
            creds_file_path: result.creds_file_path,
            project_id: None, // 项目 ID 会在健康检查时自动获取
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Gemini OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}

/// 启动 Gemini OAuth 登录流程
///
/// 打开浏览器让用户登录 Google 账号，获取 Gemini 凭证
#[tauri::command]
pub async fn start_gemini_oauth_login(
    db: State<'_, DbConnection>,
    pool_service: State<'_, ProviderPoolServiceState>,
    name: Option<String>,
) -> Result<ProviderCredential, String> {
    use crate::providers::gemini;

    tracing::info!("[Gemini OAuth] 开始 OAuth 登录流程");

    // 启动 OAuth 登录
    let result = gemini::start_gemini_oauth_login()
        .await
        .map_err(|e| format!("Gemini OAuth 登录失败: {}", e))?;

    tracing::info!(
        "[Gemini OAuth] 登录成功，凭证保存到: {}",
        result.creds_file_path
    );

    // 添加到凭证池
    let credential = pool_service.0.add_credential(
        &db,
        "gemini",
        CredentialData::GeminiOAuth {
            creds_file_path: result.creds_file_path,
            project_id: None,
        },
        name,
        Some(true),
        None,
    )?;

    tracing::info!("[Gemini OAuth] 凭证已添加到凭证池: {}", credential.uuid);

    Ok(credential)
}
