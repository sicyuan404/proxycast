//! HTTP 桥接模块
//!
//! 仅在开发模式下启用，允许浏览器 dev server 通过 HTTP 调用 Tauri 命令。
//!
//! 这是一个独立的开发服务器，运行在 3030 端口，与主应用服务器（8999）分离。

#[cfg(debug_assertions)]
pub mod dispatcher;

#[cfg(debug_assertions)]
use axum::{
    extract::State,
    http::HeaderValue,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
#[cfg(debug_assertions)]
use serde::{Deserialize, Serialize};
#[cfg(debug_assertions)]
use std::sync::Arc;
#[cfg(debug_assertions)]
use tokio::sync::RwLock;
#[cfg(debug_assertions)]
use tower_http::cors::CorsLayer;

use crate::server::AppState;

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
pub struct InvokeRequest {
    pub cmd: String,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
pub struct InvokeResponse {
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// 开发桥接服务器配置
#[cfg(debug_assertions)]
pub struct DevBridgeConfig {
    /// 监听地址
    pub host: String,
    /// 监听端口
    pub port: u16,
}

#[cfg(debug_assertions)]
impl Default for DevBridgeConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3030,
        }
    }
}

/// 开发桥接服务器
#[cfg(debug_assertions)]
pub struct DevBridgeServer;

#[cfg(debug_assertions)]
impl DevBridgeServer {
    /// 启动开发桥接服务器
    ///
    /// 这是一个独立的 HTTP 服务器，仅用于开发模式，
    /// 允许浏览器 dev server 通过 HTTP 调用 Tauri 命令。
    ///
    /// 服务器会在后台持续运行，直到应用退出。
    pub async fn start(
        app_state: Arc<RwLock<AppState>>,
        config: Option<DevBridgeConfig>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config = config.unwrap_or_default();

        let app = Router::new()
            .route("/invoke", post(invoke_command))
            .route("/health", post(health_check))
            .layer(
                // CORS 配置 - 允许 localhost:1420 访问
                CorsLayer::new()
                    .allow_origin("http://localhost:1420".parse::<HeaderValue>().unwrap())
                    .allow_methods([axum::http::Method::POST, axum::http::Method::GET])
                    .allow_headers([axum::http::header::CONTENT_TYPE]),
            )
            .with_state(app_state);

        let addr = format!("{}:{}", config.host, config.port);
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[DevBridge] 绑定失败: {} (地址: {})", e, addr);
                return Err(e.into());
            }
        };

        eprintln!("[DevBridge] 正在监听: http://{}", addr);

        // 直接运行服务器（不使用 graceful_shutdown）
        // 服务器将持续运行直到应用退出
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        Ok(())
    }
}

#[cfg(debug_assertions)]
async fn invoke_command(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<InvokeRequest>,
) -> Response {
    // 获取 AppState 的读锁
    let state_ref = state.read().await;
    // 调用命令分发器
    match dispatcher::handle_command(&state_ref, &req.cmd, req.args).await {
        Ok(result) => Json(InvokeResponse {
            result: Some(result),
            error: None,
        })
        .into_response(),
        Err(e) => Json(InvokeResponse {
            result: None,
            error: Some(e.to_string()),
        })
        .into_response(),
    }
}

#[cfg(debug_assertions)]
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "DevBridge",
        "version": "1.0.0"
    }))
}
