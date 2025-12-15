//! 路由相关 Tauri 命令

use crate::commands::provider_pool_cmd::ProviderPoolServiceState;
use crate::config;
use crate::database::DbConnection;
use crate::models::route_model::{RouteInfo, RouteListResponse};

/// 获取所有可用的路由端点
#[tauri::command]
pub async fn get_available_routes(
    db: tauri::State<'_, DbConnection>,
    pool_service: tauri::State<'_, ProviderPoolServiceState>,
) -> Result<RouteListResponse, String> {
    // 获取配置中的服务器地址
    let config = config::load_config().unwrap_or_default();
    let base_url = format!("http://{}:{}", config.server.host, config.server.port);

    let routes = pool_service
        .0
        .get_available_routes(db.inner(), &base_url)
        .map_err(|e| e.to_string())?;

    // 添加默认路由
    let mut all_routes = vec![RouteInfo {
        selector: "default".to_string(),
        provider_type: "kiro".to_string(),
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
        default_provider: "kiro".to_string(),
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
    let base_url = format!("http://{}:{}", config.server.host, config.server.port);

    let routes = pool_service
        .0
        .get_available_routes(db.inner(), &base_url)
        .map_err(|e| e.to_string())?;

    // 查找匹配的路由
    let route = routes.iter().find(|r| r.selector == selector).or_else(|| {
        // 如果是默认路由
        if selector == "default" {
            None // 返回 None 让下面的代码生成默认示例
        } else {
            None
        }
    });

    let api_key = &config.server.api_key;

    match route {
        Some(r) => Ok(r.generate_curl_examples(api_key)),
        None => {
            // 生成默认路由的示例
            let mut default_route = RouteInfo::new("default".to_string(), "kiro".to_string());
            default_route.add_endpoint(&base_url, "claude");
            default_route.add_endpoint(&base_url, "openai");
            Ok(default_route.generate_curl_examples(api_key))
        }
    }
}
