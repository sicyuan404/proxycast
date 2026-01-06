//! 路由配置相关 Tauri 命令

use crate::ProviderType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRuleDto {
    pub pattern: String,
    pub target_provider: ProviderType,
    pub priority: i32,
    pub enabled: bool,
}

/// 路由配置状态
pub struct RouterConfigState {
    pub rules: Arc<RwLock<Vec<RoutingRuleDto>>>,
    pub exclusions: Arc<RwLock<HashMap<ProviderType, Vec<String>>>>,
}

impl Default for RouterConfigState {
    fn default() -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            exclusions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// 获取所有路由规则
#[tauri::command]
pub async fn get_routing_rules(
    state: tauri::State<'_, RouterConfigState>,
) -> Result<Vec<RoutingRuleDto>, String> {
    let rules = state.rules.read().await;
    Ok(rules.clone())
}

/// 添加路由规则
#[tauri::command]
pub async fn add_routing_rule(
    state: tauri::State<'_, RouterConfigState>,
    rule: RoutingRuleDto,
) -> Result<(), String> {
    let mut rules = state.rules.write().await;
    // Check for duplicate pattern
    if rules.iter().any(|r| r.pattern == rule.pattern) {
        return Err("该模式已存在".to_string());
    }
    rules.push(rule);
    // Sort by priority
    rules.sort_by(|a, b| a.priority.cmp(&b.priority));
    Ok(())
}

/// 移除路由规则
#[tauri::command]
pub async fn remove_routing_rule(
    state: tauri::State<'_, RouterConfigState>,
    pattern: String,
) -> Result<(), String> {
    let mut rules = state.rules.write().await;
    rules.retain(|r| r.pattern != pattern);
    Ok(())
}

/// 更新路由规则
#[tauri::command]
pub async fn update_routing_rule(
    state: tauri::State<'_, RouterConfigState>,
    pattern: String,
    rule: RoutingRuleDto,
) -> Result<(), String> {
    let mut rules = state.rules.write().await;
    if let Some(existing) = rules.iter_mut().find(|r| r.pattern == pattern) {
        *existing = rule;
        // Re-sort by priority
        rules.sort_by(|a, b| a.priority.cmp(&b.priority));
        Ok(())
    } else {
        Err("规则不存在".to_string())
    }
}

/// 获取所有排除列表
#[tauri::command]
pub async fn get_exclusions(
    state: tauri::State<'_, RouterConfigState>,
) -> Result<HashMap<ProviderType, Vec<String>>, String> {
    let exclusions = state.exclusions.read().await;
    Ok(exclusions.clone())
}

/// 添加排除模式
#[tauri::command]
pub async fn add_exclusion(
    state: tauri::State<'_, RouterConfigState>,
    provider: ProviderType,
    pattern: String,
) -> Result<(), String> {
    let mut exclusions = state.exclusions.write().await;
    let patterns = exclusions.entry(provider).or_default();
    if !patterns.contains(&pattern) {
        patterns.push(pattern);
    }
    Ok(())
}

/// 移除排除模式
#[tauri::command]
pub async fn remove_exclusion(
    state: tauri::State<'_, RouterConfigState>,
    provider: ProviderType,
    pattern: String,
) -> Result<(), String> {
    let mut exclusions = state.exclusions.write().await;
    if let Some(patterns) = exclusions.get_mut(&provider) {
        patterns.retain(|p| p != &pattern);
    }
    Ok(())
}

/// 设置默认 Provider（路由器专用）
#[tauri::command]
pub async fn set_router_default_provider(_provider: ProviderType) -> Result<(), String> {
    // This would integrate with the main config
    // For now, just acknowledge the request
    Ok(())
}

/// 清空所有路由配置
#[tauri::command]
pub async fn clear_all_routing_config(
    state: tauri::State<'_, RouterConfigState>,
) -> Result<(), String> {
    {
        let mut rules = state.rules.write().await;
        rules.clear();
    }
    {
        let mut exclusions = state.exclusions.write().await;
        exclusions.clear();
    }
    Ok(())
}
