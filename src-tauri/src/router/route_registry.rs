//! 路由注册表
//!
//! 管理所有注册的路由，支持动态添加和查询。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 路由类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteType {
    /// Provider 命名空间路由 (如 /claude-kiro-oauth/v1/messages)
    ProviderNamespace,
    /// 凭证选择器路由 (如 /{uuid}/v1/messages)
    CredentialSelector,
    /// 默认路由 (如 /v1/messages)
    Default,
}

/// 注册的路由信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredRoute {
    /// 路由路径模式
    pub path_pattern: String,
    /// 路由类型
    pub route_type: RouteType,
    /// Provider 类型（如果适用）
    pub provider_type: Option<String>,
    /// 凭证 UUID（如果适用）
    pub credential_uuid: Option<String>,
    /// 凭证名称（如果适用）
    pub credential_name: Option<String>,
    /// 支持的协议
    pub protocols: Vec<String>,
    /// 是否启用
    pub enabled: bool,
    /// 优先级（数字越小优先级越高）
    pub priority: u32,
}

impl RegisteredRoute {
    /// 创建 Provider 命名空间路由
    pub fn provider_namespace(
        provider_type: &str,
        credential_uuid: &str,
        credential_name: Option<&str>,
    ) -> Self {
        let path_pattern = format!(
            "/{}/v1/{{endpoint}}",
            Self::generate_route_name(provider_type, credential_name)
        );
        Self {
            path_pattern,
            route_type: RouteType::ProviderNamespace,
            provider_type: Some(provider_type.to_string()),
            credential_uuid: Some(credential_uuid.to_string()),
            credential_name: credential_name.map(|s| s.to_string()),
            protocols: vec!["openai".to_string(), "claude".to_string()],
            enabled: true,
            priority: 10,
        }
    }

    /// 创建凭证选择器路由
    pub fn credential_selector(credential_uuid: &str, provider_type: &str) -> Self {
        Self {
            path_pattern: format!("/{}/v1/{{endpoint}}", credential_uuid),
            route_type: RouteType::CredentialSelector,
            provider_type: Some(provider_type.to_string()),
            credential_uuid: Some(credential_uuid.to_string()),
            credential_name: None,
            protocols: vec!["openai".to_string(), "claude".to_string()],
            enabled: true,
            priority: 20,
        }
    }

    /// 创建默认路由
    pub fn default_route(provider_type: &str) -> Self {
        Self {
            path_pattern: "/v1/{endpoint}".to_string(),
            route_type: RouteType::Default,
            provider_type: Some(provider_type.to_string()),
            credential_uuid: None,
            credential_name: None,
            protocols: vec!["openai".to_string(), "claude".to_string()],
            enabled: true,
            priority: 100,
        }
    }

    /// 生成路由名称
    fn generate_route_name(provider_type: &str, credential_name: Option<&str>) -> String {
        if let Some(name) = credential_name {
            // 将名称转换为 URL 友好格式
            name.to_lowercase()
                .replace(' ', "-")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect()
        } else {
            provider_type.to_lowercase()
        }
    }

    /// 获取路由的显示名称
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.credential_name {
            name.clone()
        } else if let Some(provider) = &self.provider_type {
            provider.clone()
        } else {
            "default".to_string()
        }
    }
}

/// 路由注册表
#[derive(Debug, Default)]
pub struct RouteRegistry {
    /// 所有注册的路由
    routes: Vec<RegisteredRoute>,
    /// 路由名称到索引的映射
    name_index: HashMap<String, usize>,
    /// UUID 到索引的映射
    uuid_index: HashMap<String, usize>,
}

impl RouteRegistry {
    /// 创建新的路由注册表
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册路由
    pub fn register(&mut self, route: RegisteredRoute) {
        let index = self.routes.len();

        // 更新索引
        if let Some(name) = &route.credential_name {
            self.name_index.insert(name.to_lowercase(), index);
        }
        if let Some(uuid) = &route.credential_uuid {
            self.uuid_index.insert(uuid.clone(), index);
        }

        self.routes.push(route);

        // 按优先级排序
        self.sort_by_priority();
    }

    /// 注销路由
    pub fn unregister(&mut self, credential_uuid: &str) {
        if let Some(&index) = self.uuid_index.get(credential_uuid) {
            if index < self.routes.len() {
                let route = &self.routes[index];
                if let Some(name) = &route.credential_name {
                    self.name_index.remove(&name.to_lowercase());
                }
                self.uuid_index.remove(credential_uuid);
                self.routes.remove(index);

                // 重建索引
                self.rebuild_indices();
            }
        }
    }

    /// 按名称查找路由
    pub fn find_by_name(&self, name: &str) -> Option<&RegisteredRoute> {
        self.name_index
            .get(&name.to_lowercase())
            .and_then(|&idx| self.routes.get(idx))
    }

    /// 按 UUID 查找路由
    pub fn find_by_uuid(&self, uuid: &str) -> Option<&RegisteredRoute> {
        self.uuid_index
            .get(uuid)
            .and_then(|&idx| self.routes.get(idx))
    }

    /// 按选择器查找路由（名称或 UUID）
    pub fn find_by_selector(&self, selector: &str) -> Option<&RegisteredRoute> {
        self.find_by_name(selector)
            .or_else(|| self.find_by_uuid(selector))
    }

    /// 获取所有路由
    pub fn all_routes(&self) -> &[RegisteredRoute] {
        &self.routes
    }

    /// 获取启用的路由
    pub fn enabled_routes(&self) -> Vec<&RegisteredRoute> {
        self.routes.iter().filter(|r| r.enabled).collect()
    }

    /// 按 Provider 类型获取路由
    pub fn routes_by_provider(&self, provider_type: &str) -> Vec<&RegisteredRoute> {
        self.routes
            .iter()
            .filter(|r| r.provider_type.as_deref() == Some(provider_type))
            .collect()
    }

    /// 清空所有路由
    pub fn clear(&mut self) {
        self.routes.clear();
        self.name_index.clear();
        self.uuid_index.clear();
    }

    /// 按优先级排序
    fn sort_by_priority(&mut self) {
        self.routes.sort_by_key(|r| r.priority);
        self.rebuild_indices();
    }

    /// 重建索引
    fn rebuild_indices(&mut self) {
        self.name_index.clear();
        self.uuid_index.clear();

        for (index, route) in self.routes.iter().enumerate() {
            if let Some(name) = &route.credential_name {
                self.name_index.insert(name.to_lowercase(), index);
            }
            if let Some(uuid) = &route.credential_uuid {
                self.uuid_index.insert(uuid.clone(), index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_registration() {
        let mut registry = RouteRegistry::new();

        let route =
            RegisteredRoute::provider_namespace("kiro", "uuid-123", Some("my-kiro-account"));
        registry.register(route);

        assert!(registry.find_by_name("my-kiro-account").is_some());
        assert!(registry.find_by_uuid("uuid-123").is_some());
        assert!(registry.find_by_selector("my-kiro-account").is_some());
    }

    #[test]
    fn test_route_unregistration() {
        let mut registry = RouteRegistry::new();

        let route =
            RegisteredRoute::provider_namespace("kiro", "uuid-123", Some("my-kiro-account"));
        registry.register(route);

        registry.unregister("uuid-123");

        assert!(registry.find_by_name("my-kiro-account").is_none());
        assert!(registry.find_by_uuid("uuid-123").is_none());
    }
}
