//! Provider 路由解析器
//!
//! 解析请求路径，确定目标 Provider 和协议。

use super::route_registry::{RegisteredRoute, RouteRegistry, RouteType};
use crate::models::provider_pool_model::PoolProviderType;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 路由解析结果
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// 匹配的路由
    pub route: RegisteredRoute,
    /// 请求的协议 (openai 或 claude)
    pub protocol: String,
    /// 请求的端点 (messages, chat/completions 等)
    pub endpoint: String,
    /// 路由选择器（从路径中提取）
    pub selector: Option<String>,
}

impl RouteMatch {
    /// 是否是 Claude 协议
    pub fn is_claude_protocol(&self) -> bool {
        self.protocol == "claude" || self.endpoint == "messages"
    }

    /// 是否是 OpenAI 协议
    pub fn is_openai_protocol(&self) -> bool {
        self.protocol == "openai" || self.endpoint == "chat/completions"
    }

    /// 获取 Provider 类型
    pub fn provider_type(&self) -> Option<PoolProviderType> {
        self.route
            .provider_type
            .as_ref()
            .and_then(|s| s.parse().ok())
    }
}

/// Provider 路由器
pub struct ProviderRouter {
    /// 路由注册表
    registry: Arc<RwLock<RouteRegistry>>,
}

impl ProviderRouter {
    /// 创建新的路由器
    pub fn new(registry: Arc<RwLock<RouteRegistry>>) -> Self {
        Self { registry }
    }

    /// 解析请求路径
    ///
    /// 支持的路径格式：
    /// - `/v1/messages` - 默认路由，Claude 协议
    /// - `/v1/chat/completions` - 默认路由，OpenAI 协议
    /// - `/{selector}/v1/messages` - 选择器路由，Claude 协议
    /// - `/{selector}/v1/chat/completions` - 选择器路由，OpenAI 协议
    pub async fn resolve(&self, path: &str) -> Option<RouteMatch> {
        let path = path.trim_start_matches('/');
        let parts: Vec<&str> = path.split('/').collect();

        match parts.as_slice() {
            // /v1/messages
            ["v1", "messages"] => {
                let registry = self.registry.read().await;
                let route = registry
                    .enabled_routes()
                    .into_iter()
                    .find(|r| r.route_type == RouteType::Default)
                    .cloned()
                    .unwrap_or_else(|| RegisteredRoute::default_route("kiro"));

                Some(RouteMatch {
                    route,
                    protocol: "claude".to_string(),
                    endpoint: "messages".to_string(),
                    selector: None,
                })
            }
            // /v1/chat/completions
            ["v1", "chat", "completions"] => {
                let registry = self.registry.read().await;
                let route = registry
                    .enabled_routes()
                    .into_iter()
                    .find(|r| r.route_type == RouteType::Default)
                    .cloned()
                    .unwrap_or_else(|| RegisteredRoute::default_route("kiro"));

                Some(RouteMatch {
                    route,
                    protocol: "openai".to_string(),
                    endpoint: "chat/completions".to_string(),
                    selector: None,
                })
            }
            // /{selector}/v1/messages
            [selector, "v1", "messages"] => {
                let registry = self.registry.read().await;
                let route = registry
                    .find_by_selector(selector)
                    .cloned()
                    .unwrap_or_else(|| {
                        // 创建一个临时的选择器路由
                        RegisteredRoute {
                            path_pattern: format!("/{}/v1/messages", selector),
                            route_type: RouteType::CredentialSelector,
                            provider_type: None,
                            credential_uuid: None,
                            credential_name: Some(selector.to_string()),
                            protocols: vec!["claude".to_string()],
                            enabled: true,
                            priority: 50,
                        }
                    });

                Some(RouteMatch {
                    route,
                    protocol: "claude".to_string(),
                    endpoint: "messages".to_string(),
                    selector: Some(selector.to_string()),
                })
            }
            // /{selector}/v1/chat/completions
            [selector, "v1", "chat", "completions"] => {
                let registry = self.registry.read().await;
                let route = registry
                    .find_by_selector(selector)
                    .cloned()
                    .unwrap_or_else(|| RegisteredRoute {
                        path_pattern: format!("/{}/v1/chat/completions", selector),
                        route_type: RouteType::CredentialSelector,
                        provider_type: None,
                        credential_uuid: None,
                        credential_name: Some(selector.to_string()),
                        protocols: vec!["openai".to_string()],
                        enabled: true,
                        priority: 50,
                    });

                Some(RouteMatch {
                    route,
                    protocol: "openai".to_string(),
                    endpoint: "chat/completions".to_string(),
                    selector: Some(selector.to_string()),
                })
            }
            _ => None,
        }
    }

    /// 注册凭证路由
    pub async fn register_credential(
        &self,
        provider_type: &str,
        credential_uuid: &str,
        credential_name: Option<&str>,
    ) {
        let mut registry = self.registry.write().await;

        // 注册命名空间路由
        let route =
            RegisteredRoute::provider_namespace(provider_type, credential_uuid, credential_name);
        registry.register(route);

        // 注册 UUID 选择器路由
        let selector_route = RegisteredRoute::credential_selector(credential_uuid, provider_type);
        registry.register(selector_route);
    }

    /// 注销凭证路由
    pub async fn unregister_credential(&self, credential_uuid: &str) {
        let mut registry = self.registry.write().await;
        registry.unregister(credential_uuid);
    }

    /// 获取所有注册的路由
    pub async fn list_routes(&self) -> Vec<RegisteredRoute> {
        let registry = self.registry.read().await;
        registry.all_routes().to_vec()
    }

    /// 生成路由 URL
    pub fn generate_url(&self, base_url: &str, route: &RegisteredRoute, protocol: &str) -> String {
        let endpoint = if protocol == "claude" {
            "messages"
        } else {
            "chat/completions"
        };

        if route.route_type == RouteType::Default {
            format!("{}/v1/{}", base_url, endpoint)
        } else {
            let selector = route
                .credential_name
                .as_ref()
                .or(route.credential_uuid.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            format!("{}/{}/v1/{}", base_url, selector, endpoint)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_default_routes() {
        let registry = Arc::new(RwLock::new(RouteRegistry::new()));
        let router = ProviderRouter::new(registry);

        let match1 = router.resolve("/v1/messages").await.unwrap();
        assert_eq!(match1.protocol, "claude");
        assert_eq!(match1.endpoint, "messages");
        assert!(match1.selector.is_none());

        let match2 = router.resolve("/v1/chat/completions").await.unwrap();
        assert_eq!(match2.protocol, "openai");
        assert_eq!(match2.endpoint, "chat/completions");
        assert!(match2.selector.is_none());
    }

    #[tokio::test]
    async fn test_resolve_selector_routes() {
        let registry = Arc::new(RwLock::new(RouteRegistry::new()));
        let router = ProviderRouter::new(registry);

        let match1 = router.resolve("/my-kiro/v1/messages").await.unwrap();
        assert_eq!(match1.protocol, "claude");
        assert_eq!(match1.selector, Some("my-kiro".to_string()));

        let match2 = router
            .resolve("/my-kiro/v1/chat/completions")
            .await
            .unwrap();
        assert_eq!(match2.protocol, "openai");
        assert_eq!(match2.selector, Some("my-kiro".to_string()));
    }

    #[tokio::test]
    async fn test_register_and_resolve() {
        let registry = Arc::new(RwLock::new(RouteRegistry::new()));
        let router = ProviderRouter::new(registry);

        router
            .register_credential("kiro", "uuid-123", Some("my-kiro-account"))
            .await;

        let match1 = router
            .resolve("/my-kiro-account/v1/messages")
            .await
            .unwrap();
        assert_eq!(match1.route.credential_uuid, Some("uuid-123".to_string()));
        assert_eq!(match1.route.provider_type, Some("kiro".to_string()));
    }
}
