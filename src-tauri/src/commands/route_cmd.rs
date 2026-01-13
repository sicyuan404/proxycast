//! 路由相关 Tauri 命令

use crate::commands::network_cmd::get_accessible_url;
use crate::commands::provider_pool_cmd::ProviderPoolServiceState;
use crate::config;
use crate::database::DbConnection;
use crate::models::route_model::{RouteInfo, RouteListResponse};

/// 获取可访问的服务器地址
///
/// 使用 `get_accessible_url` 函数生成可访问的 URL。
/// 对于 `0.0.0.0`，会转换为局域网 IP 或 `127.0.0.1`。
fn get_valid_base_url(config: &config::Config) -> String {
    get_accessible_url(&config.server.host, config.server.port)
}

/// 获取所有可用的路由端点
#[tauri::command]
pub async fn get_available_routes(
    db: tauri::State<'_, DbConnection>,
    pool_service: tauri::State<'_, ProviderPoolServiceState>,
) -> Result<RouteListResponse, String> {
    // 获取配置中的服务器地址和默认 Provider
    let config = config::load_config().unwrap_or_default();
    let base_url = get_valid_base_url(&config);
    let default_provider = config.default_provider.clone();

    let routes = pool_service
        .0
        .get_available_routes(db.inner(), &base_url)
        .map_err(|e| e.to_string())?;

    // 添加默认路由，使用配置中的默认 Provider
    let mut all_routes = vec![RouteInfo {
        selector: "default".to_string(),
        provider_type: default_provider.clone(),
        credential_count: 1,
        endpoints: vec![
            crate::models::route_model::RouteEndpoint {
                path: "/v1/messages".to_string(),
                protocol: "claude".to_string(),
                url: format!("{}/v1/messages", base_url),
            },
            crate::models::route_model::RouteEndpoint {
                path: "/v1/chat/completions".to_string(),
                protocol: "openai".to_string(),
                url: format!("{}/v1/chat/completions", base_url),
            },
        ],
        tags: vec!["默认".to_string()],
        enabled: true,
    }];
    all_routes.extend(routes);

    Ok(RouteListResponse {
        base_url,
        default_provider,
        routes: all_routes,
    })
}

/// 获取指定路由的 curl 示例
#[tauri::command]
pub async fn get_route_curl_examples(
    selector: String,
    db: tauri::State<'_, DbConnection>,
    pool_service: tauri::State<'_, ProviderPoolServiceState>,
) -> Result<Vec<crate::models::route_model::CurlExample>, String> {
    let config = config::load_config().unwrap_or_default();
    let base_url = get_valid_base_url(&config);
    let default_provider = config.default_provider.clone();

    let routes = pool_service
        .0
        .get_available_routes(db.inner(), &base_url)
        .map_err(|e| e.to_string())?;

    // 查找匹配的路由
    let route = routes.iter().find(|r| r.selector == selector);

    // P0 安全修复：curl 示例使用占位符，不暴露真实 API Key
    let api_key = "${PROXYCAST_API_KEY}";

    match route {
        Some(r) => Ok(r.generate_curl_examples(api_key)),
        None => {
            // 生成默认路由的示例，使用配置中的默认 Provider
            let mut default_route = RouteInfo::new("default".to_string(), default_provider);
            default_route.add_endpoint(&base_url, "claude");
            default_route.add_endpoint(&base_url, "openai");
            Ok(default_route.generate_curl_examples(api_key))
        }
    }
}
