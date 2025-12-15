//! 路由系统模块
//!
//! 支持动态路由注册和命名空间路由解析。
//! 路由格式：
//! - `/{provider-name}/v1/messages` - Provider 命名空间路由
//! - `/{selector}/v1/messages` - 凭证选择器路由（向后兼容）
//! - `/v1/messages` - 默认路由

mod provider_router;
mod route_registry;

pub use provider_router::ProviderRouter;
pub use route_registry::{RegisteredRoute, RouteRegistry, RouteType};
