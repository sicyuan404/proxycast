//! 模型注册表 Tauri 命令
//!
//! 提供模型注册表相关的前端 API

use crate::models::model_registry::{
    EnhancedModelMetadata, ModelSyncState, ModelTier, UserModelPreference,
};
use crate::services::model_registry_service::ModelRegistryService;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// 模型注册服务状态
pub type ModelRegistryState = Arc<RwLock<Option<ModelRegistryService>>>;

/// 获取所有模型
#[tauri::command]
pub async fn get_model_registry(
    state: State<'_, ModelRegistryState>,
) -> Result<Vec<EnhancedModelMetadata>, String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    Ok(service.get_all_models().await)
}

/// 刷新模型注册表（从 models.dev 获取最新数据）
#[tauri::command]
pub async fn refresh_model_registry(state: State<'_, ModelRegistryState>) -> Result<(), String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    service.refresh_from_repo().await
}

/// 搜索模型
#[tauri::command]
pub async fn search_models(
    state: State<'_, ModelRegistryState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<EnhancedModelMetadata>, String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    Ok(service.search_models(&query, limit.unwrap_or(50)).await)
}

/// 获取用户模型偏好
#[tauri::command]
pub async fn get_model_preferences(
    state: State<'_, ModelRegistryState>,
) -> Result<Vec<UserModelPreference>, String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    service.get_all_preferences().await
}

/// 切换模型收藏状态
#[tauri::command]
pub async fn toggle_model_favorite(
    state: State<'_, ModelRegistryState>,
    model_id: String,
) -> Result<bool, String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    service.toggle_favorite(&model_id).await
}

/// 隐藏模型
#[tauri::command]
pub async fn hide_model(
    state: State<'_, ModelRegistryState>,
    model_id: String,
) -> Result<(), String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    service.hide_model(&model_id).await
}

/// 记录模型使用
#[tauri::command]
pub async fn record_model_usage(
    state: State<'_, ModelRegistryState>,
    model_id: String,
) -> Result<(), String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    service.record_usage(&model_id).await
}

/// 获取模型同步状态
#[tauri::command]
pub async fn get_model_sync_state(
    state: State<'_, ModelRegistryState>,
) -> Result<ModelSyncState, String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    Ok(service.get_sync_state().await)
}

/// 按 Provider 获取模型
#[tauri::command]
pub async fn get_models_for_provider(
    state: State<'_, ModelRegistryState>,
    provider_id: String,
) -> Result<Vec<EnhancedModelMetadata>, String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    Ok(service.get_models_by_provider(&provider_id).await)
}

/// 按服务等级获取模型
#[tauri::command]
pub async fn get_models_by_tier(
    state: State<'_, ModelRegistryState>,
    tier: String,
) -> Result<Vec<EnhancedModelMetadata>, String> {
    let guard = state.read().await;
    let service = guard
        .as_ref()
        .ok_or_else(|| "模型注册服务未初始化".to_string())?;

    let tier: ModelTier = tier
        .parse()
        .map_err(|_| format!("无效的服务等级: {}", tier))?;

    Ok(service.get_models_by_tier(tier).await)
}
