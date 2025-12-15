//! HTTP API 服务器
use crate::config::Config;
use crate::converter::anthropic_to_openai::convert_anthropic_to_openai;
use crate::converter::openai_to_antigravity::{
    convert_antigravity_to_openai_response, convert_openai_to_antigravity,
};
use crate::database::DbConnection;
use crate::logger::LogStore;
use crate::models::anthropic::*;
use crate::models::openai::*;
use crate::models::route_model::{RouteInfo, RouteListResponse};
use crate::providers::antigravity::AntigravityProvider;
use crate::providers::claude_custom::ClaudeCustomProvider;
use crate::providers::gemini::GeminiProvider;
use crate::providers::kiro::KiroProvider;
use crate::providers::openai_custom::OpenAICustomProvider;
use crate::providers::qwen::QwenProvider;
use crate::services::provider_pool_service::ProviderPoolService;
use crate::services::token_cache_service::TokenCacheService;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};

/// 安全截断字符串到指定字符数，避免 UTF-8 边界问题
fn safe_truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        chars[..max_chars].iter().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub requests: u64,
    pub uptime_secs: u64,
}

pub struct ServerState {
    pub config: Config,
    pub running: bool,
    pub requests: u64,
    pub start_time: Option<std::time::Instant>,
    pub kiro_provider: KiroProvider,
    pub gemini_provider: GeminiProvider,
    pub qwen_provider: QwenProvider,
    pub openai_custom_provider: OpenAICustomProvider,
    pub claude_custom_provider: ClaudeCustomProvider,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ServerState {
    pub fn new(config: Config) -> Self {
        let kiro = KiroProvider::new();
        let gemini = GeminiProvider::new();
        let qwen = QwenProvider::new();
        let openai_custom = OpenAICustomProvider::new();
        let claude_custom = ClaudeCustomProvider::new();

        Self {
            config,
            running: false,
            requests: 0,
            start_time: None,
            kiro_provider: kiro,
            gemini_provider: gemini,
            qwen_provider: qwen,
            openai_custom_provider: openai_custom,
            claude_custom_provider: claude_custom,
            shutdown_tx: None,
        }
    }

    pub fn status(&self) -> ServerStatus {
        ServerStatus {
            running: self.running,
            host: self.config.server.host.clone(),
            port: self.config.server.port,
            requests: self.requests,
            uptime_secs: self.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0),
        }
    }

    pub async fn start(
        &mut self,
        logs: Arc<RwLock<LogStore>>,
        pool_service: Arc<ProviderPoolService>,
        token_cache: Arc<TokenCacheService>,
        db: Option<DbConnection>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.running {
            return Ok(());
        }

        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        let host = self.config.server.host.clone();
        let port = self.config.server.port;
        let api_key = self.config.server.api_key.clone();

        // 重新加载凭证
        let _ = self.kiro_provider.load_credentials().await;
        let kiro = self.kiro_provider.clone();

        tokio::spawn(async move {
            if let Err(e) = run_server(
                &host,
                port,
                &api_key,
                kiro,
                logs,
                rx,
                pool_service,
                token_cache,
                db,
            )
            .await
            {
                tracing::error!("Server error: {}", e);
            }
        });

        self.running = true;
        self.start_time = Some(std::time::Instant::now());
        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.running = false;
        self.start_time = None;
    }
}

impl Clone for KiroProvider {
    fn clone(&self) -> Self {
        Self {
            credentials: self.credentials.clone(),
            client: reqwest::Client::new(),
            creds_path: self.creds_path.clone(),
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
struct AppState {
    api_key: String,
    base_url: String,
    kiro: Arc<RwLock<KiroProvider>>,
    logs: Arc<RwLock<LogStore>>,
    kiro_refresh_lock: Arc<tokio::sync::Mutex<()>>,
    gemini_refresh_lock: Arc<tokio::sync::Mutex<()>>,
    qwen_refresh_lock: Arc<tokio::sync::Mutex<()>>,
    pool_service: Arc<ProviderPoolService>,
    token_cache: Arc<TokenCacheService>,
    db: Option<DbConnection>,
}

async fn run_server(
    host: &str,
    port: u16,
    api_key: &str,
    kiro: KiroProvider,
    logs: Arc<RwLock<LogStore>>,
    shutdown: oneshot::Receiver<()>,
    pool_service: Arc<ProviderPoolService>,
    token_cache: Arc<TokenCacheService>,
    db: Option<DbConnection>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let base_url = format!("http://{}:{}", host, port);
    let state = AppState {
        api_key: api_key.to_string(),
        base_url,
        kiro: Arc::new(RwLock::new(kiro)),
        logs,
        kiro_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        gemini_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        qwen_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        pool_service,
        token_cache,
        db,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/routes", get(list_routes))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic_messages))
        .route("/v1/messages/count_tokens", post(count_tokens))
        // 多供应商路由
        .route(
            "/:selector/v1/messages",
            post(anthropic_messages_with_selector),
        )
        .route(
            "/:selector/v1/chat/completions",
            post(chat_completions_with_selector),
        )
        .with_state(state);

    let addr: std::net::SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
        })
        .await?;

    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": "0.1.0"
    }))
}

async fn models() -> impl IntoResponse {
    Json(serde_json::json!({
        "object": "list",
        "data": [
            // Kiro/Claude models
            {"id": "claude-sonnet-4-5", "object": "model", "owned_by": "anthropic"},
            {"id": "claude-sonnet-4-5-20250929", "object": "model", "owned_by": "anthropic"},
            {"id": "claude-3-7-sonnet-20250219", "object": "model", "owned_by": "anthropic"},
            {"id": "claude-3-5-sonnet-latest", "object": "model", "owned_by": "anthropic"},
            // Gemini models
            {"id": "gemini-2.5-flash", "object": "model", "owned_by": "google"},
            {"id": "gemini-2.5-flash-lite", "object": "model", "owned_by": "google"},
            {"id": "gemini-2.5-pro", "object": "model", "owned_by": "google"},
            {"id": "gemini-2.5-pro-preview-06-05", "object": "model", "owned_by": "google"},
            {"id": "gemini-3-pro-preview", "object": "model", "owned_by": "google"},
            // Qwen models
            {"id": "qwen3-coder-plus", "object": "model", "owned_by": "alibaba"},
            {"id": "qwen3-coder-flash", "object": "model", "owned_by": "alibaba"}
        ]
    }))
}

async fn verify_api_key(
    headers: &HeaderMap,
    expected_key: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let auth = headers
        .get("authorization")
        .or_else(|| headers.get("x-api-key"))
        .and_then(|v| v.to_str().ok());

    let key = match auth {
        Some(s) if s.starts_with("Bearer ") => &s[7..],
        Some(s) => s,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": {"message": "No API key provided"}})),
            ))
        }
    };

    if key != expected_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": {"message": "Invalid API key"}})),
        ));
    }

    Ok(())
}

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    if let Err(e) = verify_api_key(&headers, &state.api_key).await {
        state
            .logs
            .write()
            .await
            .add("warn", "Unauthorized request to /v1/chat/completions");
        return e.into_response();
    }

    state.logs.write().await.add(
        "info",
        &format!(
            "POST /v1/chat/completions model={} stream={}",
            request.model, request.stream
        ),
    );

    // 检查是否需要刷新 token（无 token 或即将过期）
    {
        let _guard = state.kiro_refresh_lock.lock().await;
        let mut kiro = state.kiro.write().await;
        let needs_refresh =
            kiro.credentials.access_token.is_none() || kiro.is_token_expiring_soon();
        if needs_refresh {
            if let Err(e) = kiro.refresh_token().await {
                state
                    .logs
                    .write()
                    .await
                    .add("error", &format!("Token refresh failed: {e}"));
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {e}")}})),
                ).into_response();
            }
        }
    }

    let kiro = state.kiro.read().await;

    match kiro.call_api(&request).await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                match resp.text().await {
                    Ok(body) => {
                        let parsed = parse_cw_response(&body);
                        let has_tool_calls = !parsed.tool_calls.is_empty();

                        state.logs.write().await.add(
                            "info",
                            &format!(
                                "Request completed: content_len={}, tool_calls={}",
                                parsed.content.len(),
                                parsed.tool_calls.len()
                            ),
                        );

                        // 构建消息
                        let message = if has_tool_calls {
                            serde_json::json!({
                                "role": "assistant",
                                "content": if parsed.content.is_empty() { serde_json::Value::Null } else { serde_json::json!(parsed.content) },
                                "tool_calls": parsed.tool_calls.iter().map(|tc| {
                                    serde_json::json!({
                                        "id": tc.id,
                                        "type": "function",
                                        "function": {
                                            "name": tc.function.name,
                                            "arguments": tc.function.arguments
                                        }
                                    })
                                }).collect::<Vec<_>>()
                            })
                        } else {
                            serde_json::json!({
                                "role": "assistant",
                                "content": parsed.content
                            })
                        };

                        let response = serde_json::json!({
                            "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                            "object": "chat.completion",
                            "created": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            "model": request.model,
                            "choices": [{
                                "index": 0,
                                "message": message,
                                "finish_reason": if has_tool_calls { "tool_calls" } else { "stop" }
                            }],
                            "usage": {
                                "prompt_tokens": 0,
                                "completion_tokens": 0,
                                "total_tokens": 0
                            }
                        });
                        Json(response).into_response()
                    }
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": e.to_string()}})),
                    )
                        .into_response(),
                }
            } else if status.as_u16() == 403 || status.as_u16() == 402 {
                // Token 过期或账户问题，尝试重新加载凭证并刷新
                drop(kiro);
                let _guard = state.kiro_refresh_lock.lock().await;
                let mut kiro = state.kiro.write().await;
                state.logs.write().await.add(
                    "warn",
                    &format!(
                        "[AUTH] Got {}, reloading credentials and attempting token refresh...",
                        status.as_u16()
                    ),
                );

                // 先重新加载凭证文件（可能用户换了账户）
                if let Err(e) = kiro.load_credentials().await {
                    state.logs.write().await.add(
                        "error",
                        &format!("[AUTH] Failed to reload credentials: {e}"),
                    );
                }

                match kiro.refresh_token().await {
                    Ok(_) => {
                        state
                            .logs
                            .write()
                            .await
                            .add("info", "[AUTH] Token refreshed successfully after reload");
                        // 重试请求
                        drop(kiro);
                        let kiro = state.kiro.read().await;
                        match kiro.call_api(&request).await {
                            Ok(retry_resp) => {
                                if retry_resp.status().is_success() {
                                    match retry_resp.text().await {
                                        Ok(body) => {
                                            let parsed = parse_cw_response(&body);
                                            let has_tool_calls = !parsed.tool_calls.is_empty();

                                            let message = if has_tool_calls {
                                                serde_json::json!({
                                                    "role": "assistant",
                                                    "content": if parsed.content.is_empty() { serde_json::Value::Null } else { serde_json::json!(parsed.content) },
                                                    "tool_calls": parsed.tool_calls.iter().map(|tc| {
                                                        serde_json::json!({
                                                            "id": tc.id,
                                                            "type": "function",
                                                            "function": {
                                                                "name": tc.function.name,
                                                                "arguments": tc.function.arguments
                                                            }
                                                        })
                                                    }).collect::<Vec<_>>()
                                                })
                                            } else {
                                                serde_json::json!({
                                                    "role": "assistant",
                                                    "content": parsed.content
                                                })
                                            };

                                            let response = serde_json::json!({
                                                "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                                                "object": "chat.completion",
                                                "created": std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_secs(),
                                                "model": request.model,
                                                "choices": [{
                                                    "index": 0,
                                                    "message": message,
                                                    "finish_reason": if has_tool_calls { "tool_calls" } else { "stop" }
                                                }],
                                                "usage": {
                                                    "prompt_tokens": 0,
                                                    "completion_tokens": 0,
                                                    "total_tokens": 0
                                                }
                                            });
                                            return Json(response).into_response();
                                        }
                                        Err(e) => return (
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            Json(serde_json::json!({"error": {"message": e.to_string()}})),
                                        ).into_response(),
                                    }
                                }
                                let body = retry_resp.text().await.unwrap_or_default();
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(serde_json::json!({"error": {"message": format!("Retry failed: {}", body)}})),
                                ).into_response()
                            }
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": {"message": e.to_string()}})),
                            )
                                .into_response(),
                        }
                    }
                    Err(e) => {
                        state
                            .logs
                            .write()
                            .await
                            .add("error", &format!("[AUTH] Token refresh failed: {e}"));
                        (
                            StatusCode::UNAUTHORIZED,
                            Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {e}")}})),
                        )
                            .into_response()
                    }
                }
            } else {
                let body = resp.text().await.unwrap_or_default();
                state.logs.write().await.add(
                    "error",
                    &format!("Upstream error {}: {}", status, safe_truncate(&body, 200)),
                );
                (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(serde_json::json!({"error": {"message": format!("Upstream error: {}", body)}}))
                ).into_response()
            }
        }
        Err(e) => {
            state
                .logs
                .write()
                .await
                .add("error", &format!("API call failed: {e}"));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": {"message": e.to_string()}})),
            )
                .into_response()
        }
    }
}

async fn anthropic_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AnthropicMessagesRequest>,
) -> Response {
    if let Err(e) = verify_api_key(&headers, &state.api_key).await {
        state
            .logs
            .write()
            .await
            .add("warn", "Unauthorized request to /v1/messages");
        return e.into_response();
    }

    // 详细记录请求信息
    let msg_count = request.messages.len();
    let has_tools = request.tools.as_ref().map(|t| t.len()).unwrap_or(0);
    let has_system = request.system.is_some();
    state.logs.write().await.add(
        "info",
        &format!(
            "[REQ] POST /v1/messages model={} stream={} messages={} tools={} has_system={}",
            request.model, request.stream, msg_count, has_tools, has_system
        ),
    );

    // 记录最后一条消息的角色和内容预览
    if let Some(last_msg) = request.messages.last() {
        let content_preview = match &last_msg.content {
            serde_json::Value::String(s) => s.chars().take(100).collect::<String>(),
            serde_json::Value::Array(arr) => {
                if let Some(first) = arr.first() {
                    if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                        text.chars().take(100).collect::<String>()
                    } else {
                        format!("[{} blocks]", arr.len())
                    }
                } else {
                    "[empty]".to_string()
                }
            }
            _ => "[unknown]".to_string(),
        };
        state.logs.write().await.add(
            "debug",
            &format!(
                "[REQ] Last message: role={} content={}",
                last_msg.role, content_preview
            ),
        );
    }

    // 检查是否需要刷新 token（无 token 或即将过期）
    {
        let _guard = state.kiro_refresh_lock.lock().await;
        let mut kiro = state.kiro.write().await;
        let needs_refresh =
            kiro.credentials.access_token.is_none() || kiro.is_token_expiring_soon();
        if needs_refresh {
            state.logs.write().await.add(
                "info",
                "[AUTH] No access token or token expiring soon, attempting refresh...",
            );
            if let Err(e) = kiro.refresh_token().await {
                state
                    .logs
                    .write()
                    .await
                    .add("error", &format!("[AUTH] Token refresh failed: {e}"));
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {e}")}})),
                )
                    .into_response();
            }
            state
                .logs
                .write()
                .await
                .add("info", "[AUTH] Token refreshed successfully");
        }
    }

    // 转换为 OpenAI 格式
    let openai_request = convert_anthropic_to_openai(&request);

    // 记录转换后的请求信息
    state.logs.write().await.add(
        "debug",
        &format!(
            "[CONVERT] OpenAI format: messages={} tools={} stream={}",
            openai_request.messages.len(),
            openai_request.tools.as_ref().map(|t| t.len()).unwrap_or(0),
            openai_request.stream
        ),
    );

    let kiro = state.kiro.read().await;

    match kiro.call_api(&openai_request).await {
        Ok(resp) => {
            let status = resp.status();
            state
                .logs
                .write()
                .await
                .add("info", &format!("[RESP] Upstream status: {status}"));

            if status.is_success() {
                match resp.bytes().await {
                    Ok(bytes) => {
                        // 使用 lossy 转换，避免无效 UTF-8 导致崩溃
                        let body = String::from_utf8_lossy(&bytes).to_string();

                        // 记录原始响应长度
                        state.logs.write().await.add(
                            "debug",
                            &format!("[RESP] Raw body length: {} bytes", bytes.len()),
                        );

                        // 保存原始响应到文件用于调试
                        let request_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
                        state.logs.read().await.log_raw_response(&request_id, &body);
                        state.logs.write().await.add(
                            "debug",
                            &format!("[RESP] Raw response saved to raw_response_{request_id}.txt"),
                        );

                        // 记录响应的前200字符用于调试（减少日志量）
                        let preview: String =
                            body.chars().filter(|c| !c.is_control()).take(200).collect();
                        state
                            .logs
                            .write()
                            .await
                            .add("debug", &format!("[RESP] Body preview: {preview}"));

                        let parsed = parse_cw_response(&body);

                        // 详细记录解析结果
                        state.logs.write().await.add(
                            "info",
                            &format!(
                                "[RESP] Parsed: content_len={}, tool_calls={}, content_preview={}",
                                parsed.content.len(),
                                parsed.tool_calls.len(),
                                parsed.content.chars().take(100).collect::<String>()
                            ),
                        );

                        // 记录 tool calls 详情
                        for (i, tc) in parsed.tool_calls.iter().enumerate() {
                            state.logs.write().await.add(
                                "debug",
                                &format!(
                                    "[RESP] Tool call {}: name={} id={}",
                                    i, tc.function.name, tc.id
                                ),
                            );
                        }

                        // 如果请求流式响应，返回 SSE 格式
                        if request.stream {
                            return build_anthropic_stream_response(&request.model, &parsed);
                        }

                        // 非流式响应
                        build_anthropic_response(&request.model, &parsed)
                    }
                    Err(e) => {
                        state
                            .logs
                            .write()
                            .await
                            .add("error", &format!("[ERROR] Response body read failed: {e}"));
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": {"message": e.to_string()}})),
                        )
                            .into_response()
                    }
                }
            } else if status.as_u16() == 403 || status.as_u16() == 402 {
                // Token 过期或账户问题，尝试重新加载凭证并刷新
                drop(kiro);
                let _guard = state.kiro_refresh_lock.lock().await;
                let mut kiro = state.kiro.write().await;
                state.logs.write().await.add(
                    "warn",
                    &format!(
                        "[AUTH] Got {}, reloading credentials and attempting token refresh...",
                        status.as_u16()
                    ),
                );

                // 先重新加载凭证文件（可能用户换了账户）
                if let Err(e) = kiro.load_credentials().await {
                    state.logs.write().await.add(
                        "error",
                        &format!("[AUTH] Failed to reload credentials: {e}"),
                    );
                }

                match kiro.refresh_token().await {
                    Ok(_) => {
                        state.logs.write().await.add(
                            "info",
                            "[AUTH] Token refreshed successfully, retrying request...",
                        );
                        drop(kiro);
                        let kiro = state.kiro.read().await;
                        match kiro.call_api(&openai_request).await {
                            Ok(retry_resp) => {
                                let retry_status = retry_resp.status();
                                state.logs.write().await.add(
                                    "info",
                                    &format!("[RETRY] Response status: {retry_status}"),
                                );
                                if retry_resp.status().is_success() {
                                    match retry_resp.bytes().await {
                                        Ok(bytes) => {
                                            let body = String::from_utf8_lossy(&bytes).to_string();
                                            let parsed = parse_cw_response(&body);
                                            state.logs.write().await.add(
                                                "info",
                                                &format!(
                                                "[RETRY] Success: content_len={}, tool_calls={}",
                                                parsed.content.len(), parsed.tool_calls.len()
                                            ),
                                            );
                                            if request.stream {
                                                return build_anthropic_stream_response(
                                                    &request.model,
                                                    &parsed,
                                                );
                                            }
                                            return build_anthropic_response(
                                                &request.model,
                                                &parsed,
                                            );
                                        }
                                        Err(e) => {
                                            state.logs.write().await.add(
                                                "error",
                                                &format!("[RETRY] Body read failed: {e}"),
                                            );
                                            return (
                                                StatusCode::INTERNAL_SERVER_ERROR,
                                                Json(serde_json::json!({"error": {"message": e.to_string()}})),
                                            )
                                                .into_response();
                                        }
                                    }
                                }
                                let body = retry_resp
                                    .bytes()
                                    .await
                                    .map(|b| String::from_utf8_lossy(&b).to_string())
                                    .unwrap_or_default();
                                state.logs.write().await.add(
                                    "error",
                                    &format!(
                                        "[RETRY] Failed with status {retry_status}: {}",
                                        safe_truncate(&body, 500)
                                    ),
                                );
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(serde_json::json!({"error": {"message": format!("Retry failed: {}", body)}})),
                                )
                                    .into_response()
                            }
                            Err(e) => {
                                state
                                    .logs
                                    .write()
                                    .await
                                    .add("error", &format!("[RETRY] Request failed: {e}"));
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                                )
                                    .into_response()
                            }
                        }
                    }
                    Err(e) => {
                        state
                            .logs
                            .write()
                            .await
                            .add("error", &format!("[AUTH] Token refresh failed: {e}"));
                        (
                            StatusCode::UNAUTHORIZED,
                            Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {e}")}})),
                        )
                            .into_response()
                    }
                }
            } else {
                let body = resp.text().await.unwrap_or_default();
                state.logs.write().await.add(
                    "error",
                    &format!(
                        "[ERROR] Upstream error HTTP {}: {}",
                        status,
                        safe_truncate(&body, 500)
                    ),
                );
                (
                    StatusCode::from_u16(status.as_u16())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(
                        serde_json::json!({"error": {"message": format!("Upstream error: {}", body)}}),
                    ),
                )
                    .into_response()
            }
        }
        Err(e) => {
            // 详细记录网络/连接错误
            let error_details = format!("{e:?}");
            state
                .logs
                .write()
                .await
                .add("error", &format!("[ERROR] Kiro API call failed: {e}"));
            state.logs.write().await.add(
                "debug",
                &format!("[ERROR] Full error details: {error_details}"),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": {"message": e.to_string()}})),
            )
                .into_response()
        }
    }
}

/// 构建 Anthropic 非流式响应
fn build_anthropic_response(model: &str, parsed: &CWParsedResponse) -> Response {
    let has_tool_calls = !parsed.tool_calls.is_empty();
    let mut content_array: Vec<serde_json::Value> = Vec::new();

    if !parsed.content.is_empty() {
        content_array.push(serde_json::json!({
            "type": "text",
            "text": parsed.content
        }));
    }

    for tc in &parsed.tool_calls {
        let input: serde_json::Value =
            serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::json!({}));
        content_array.push(serde_json::json!({
            "type": "tool_use",
            "id": tc.id,
            "name": tc.function.name,
            "input": input
        }));
    }

    if content_array.is_empty() {
        content_array.push(serde_json::json!({"type": "text", "text": ""}));
    }

    // 估算 output tokens: 基于响应内容长度 (约 4 字符 = 1 token)
    let mut output_tokens: u32 = (parsed.content.len() / 4) as u32;
    for tc in &parsed.tool_calls {
        output_tokens += (tc.function.arguments.len() / 4) as u32;
    }
    // 从 context_usage_percentage 估算 input tokens
    // 假设 100% = 200k tokens (Claude 的上下文窗口)
    let input_tokens = ((parsed.context_usage_percentage / 100.0) * 200000.0) as u32;

    let response = serde_json::json!({
        "id": format!("msg_{}", uuid::Uuid::new_v4()),
        "type": "message",
        "role": "assistant",
        "content": content_array,
        "model": model,
        "stop_reason": if has_tool_calls { "tool_use" } else { "end_turn" },
        "stop_sequence": null,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }
    });
    Json(response).into_response()
}

/// 构建 Anthropic 流式响应 (SSE)
fn build_anthropic_stream_response(model: &str, parsed: &CWParsedResponse) -> Response {
    let has_tool_calls = !parsed.tool_calls.is_empty();
    let message_id = format!("msg_{}", uuid::Uuid::new_v4());
    let model = model.to_string();
    let content = parsed.content.clone();
    let tool_calls = parsed.tool_calls.clone();

    // 估算 output tokens: 基于响应内容长度 (约 4 字符 = 1 token)
    let mut output_tokens: u32 = (parsed.content.len() / 4) as u32;
    for tc in &parsed.tool_calls {
        output_tokens += (tc.function.arguments.len() / 4) as u32;
    }
    // 从 context_usage_percentage 估算 input tokens
    let input_tokens = ((parsed.context_usage_percentage / 100.0) * 200000.0) as u32;

    // 构建 SSE 事件流
    let mut events: Vec<String> = Vec::new();

    // 1. message_start
    let message_start = serde_json::json!({
        "type": "message_start",
        "message": {
            "id": message_id,
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [],
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {"input_tokens": input_tokens, "output_tokens": 0}
        }
    });
    events.push(format!("event: message_start\ndata: {message_start}\n\n"));

    let mut block_index = 0;

    // 2. 文本内容块 - 即使为空也要发送，Claude Code 需要至少一个 content block
    // content_block_start
    let block_start = serde_json::json!({
        "type": "content_block_start",
        "index": block_index,
        "content_block": {"type": "text", "text": ""}
    });
    events.push(format!(
        "event: content_block_start\ndata: {block_start}\n\n"
    ));

    if !content.is_empty() {
        // content_block_delta - 发送完整内容
        let block_delta = serde_json::json!({
            "type": "content_block_delta",
            "index": block_index,
            "delta": {"type": "text_delta", "text": content}
        });
        events.push(format!(
            "event: content_block_delta\ndata: {block_delta}\n\n"
        ));
    }

    // content_block_stop
    let block_stop = serde_json::json!({
        "type": "content_block_stop",
        "index": block_index
    });
    events.push(format!("event: content_block_stop\ndata: {block_stop}\n\n"));

    block_index += 1;

    // 3. Tool use 块
    for tc in &tool_calls {
        // content_block_start
        let block_start = serde_json::json!({
            "type": "content_block_start",
            "index": block_index,
            "content_block": {
                "type": "tool_use",
                "id": tc.id,
                "name": tc.function.name,
                "input": {}
            }
        });
        events.push(format!(
            "event: content_block_start\ndata: {block_start}\n\n"
        ));

        // content_block_delta - input_json_delta
        // 注意：partial_json 应该是原始 JSON 字符串，不是再次序列化的
        let partial_json = if tc.function.arguments.is_empty() {
            "{}".to_string()
        } else {
            tc.function.arguments.clone()
        };
        let block_delta = serde_json::json!({
            "type": "content_block_delta",
            "index": block_index,
            "delta": {
                "type": "input_json_delta",
                "partial_json": partial_json
            }
        });
        events.push(format!(
            "event: content_block_delta\ndata: {block_delta}\n\n"
        ));

        // content_block_stop
        let block_stop = serde_json::json!({
            "type": "content_block_stop",
            "index": block_index
        });
        events.push(format!("event: content_block_stop\ndata: {block_stop}\n\n"));

        block_index += 1;
    }

    // 4. message_delta
    let message_delta = serde_json::json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": if has_tool_calls { "tool_use" } else { "end_turn" },
            "stop_sequence": null
        },
        "usage": {"output_tokens": output_tokens}
    });
    events.push(format!("event: message_delta\ndata: {message_delta}\n\n"));

    // 5. message_stop
    let message_stop = serde_json::json!({"type": "message_stop"});
    events.push(format!("event: message_stop\ndata: {message_stop}\n\n"));

    // 创建 SSE 响应
    let body_stream = stream::iter(events.into_iter().map(Ok::<_, std::convert::Infallible>));
    let body = Body::from_stream(body_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .body(body)
        .unwrap_or_else(|e| {
            tracing::error!("Failed to build SSE response: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap_or_default()
        })
}

async fn count_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_request): Json<serde_json::Value>,
) -> Response {
    if let Err(e) = verify_api_key(&headers, &state.api_key).await {
        return e.into_response();
    }

    // Claude Code 需要这个端点，返回估算值
    Json(serde_json::json!({
        "input_tokens": 100
    }))
    .into_response()
}

/// CodeWhisperer 响应解析结果
#[derive(Debug, Default)]
struct CWParsedResponse {
    content: String,
    tool_calls: Vec<ToolCall>,
    usage_credits: f64,
    context_usage_percentage: f64,
}

/// 解析 CodeWhisperer AWS Event Stream 响应
/// AWS Event Stream 是二进制格式，JSON payload 嵌入在二进制头部之间
fn parse_cw_response(body: &str) -> CWParsedResponse {
    let mut result = CWParsedResponse::default();
    // 使用 HashMap 来跟踪多个并发的 tool calls
    // key: toolUseId, value: (name, input_accumulated)
    let mut tool_map: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    // 将字符串转换为字节，因为 AWS Event Stream 包含二进制数据
    let bytes = body.as_bytes();

    // 搜索所有 JSON 对象的模式
    // AWS Event Stream 格式: [binary headers]{"content":"..."}[binary trailer]
    let json_patterns: &[&[u8]] = &[
        b"{\"content\":",
        b"{\"name\":",
        b"{\"input\":",
        b"{\"stop\":",
        b"{\"followupPrompt\":",
        b"{\"toolUseId\":",
        b"{\"unit\":",                   // meteringEvent
        b"{\"contextUsagePercentage\":", // contextUsageEvent
    ];

    let mut pos = 0;
    while pos < bytes.len() {
        // 找到下一个 JSON 对象的开始
        let mut next_start: Option<usize> = None;

        for pattern in json_patterns {
            if let Some(idx) = find_subsequence(&bytes[pos..], pattern) {
                let abs_pos = pos + idx;
                if next_start.is_none_or(|start| abs_pos < start) {
                    next_start = Some(abs_pos);
                }
            }
        }

        let start = match next_start {
            Some(s) => s,
            None => break,
        };

        // 从 start 位置提取完整的 JSON 对象
        if let Some(json_str) = extract_json_from_bytes(&bytes[start..]) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
                // 处理 content 事件
                if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
                    // 跳过 followupPrompt
                    if value.get("followupPrompt").is_none() {
                        result.content.push_str(content);
                    }
                }
                // 处理 tool use 事件 (包含 toolUseId)
                else if let Some(tool_use_id) = value.get("toolUseId").and_then(|v| v.as_str()) {
                    let name = value
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input_chunk = value
                        .get("input")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let is_stop = value.get("stop").and_then(|v| v.as_bool()).unwrap_or(false);

                    // 获取或创建 tool entry
                    let entry = tool_map
                        .entry(tool_use_id.to_string())
                        .or_insert_with(|| (String::new(), String::new()));

                    // 更新 name（如果有）
                    if !name.is_empty() {
                        entry.0 = name;
                    }

                    // 累积 input
                    entry.1.push_str(&input_chunk);

                    // 如果是 stop 事件，完成这个 tool call
                    if is_stop {
                        if let Some((name, input)) = tool_map.remove(tool_use_id) {
                            if !name.is_empty() {
                                result.tool_calls.push(ToolCall {
                                    id: tool_use_id.to_string(),
                                    call_type: "function".to_string(),
                                    function: FunctionCall {
                                        name,
                                        arguments: input,
                                    },
                                });
                            }
                        }
                    }
                }
                // 处理独立的 stop 事件（没有 toolUseId）
                else if value.get("stop").and_then(|v| v.as_bool()).unwrap_or(false) {
                    // 这种情况不应该发生，但以防万一
                }
                // 处理 meteringEvent: {"unit":"credit","unitPlural":"credits","usage":0.34}
                else if let Some(usage) = value.get("usage").and_then(|v| v.as_f64()) {
                    result.usage_credits = usage;
                }
                // 处理 contextUsageEvent: {"contextUsagePercentage":54.36}
                else if let Some(ctx_usage) =
                    value.get("contextUsagePercentage").and_then(|v| v.as_f64())
                {
                    result.context_usage_percentage = ctx_usage;
                }
            }
            pos = start + json_str.len();
        } else {
            pos = start + 1;
        }
    }

    // 处理未完成的 tool calls（没有收到 stop 事件的）
    for (id, (name, input)) in tool_map {
        if !name.is_empty() {
            result.tool_calls.push(ToolCall {
                id,
                call_type: "function".to_string(),
                function: FunctionCall {
                    name,
                    arguments: input,
                },
            });
        }
    }

    // 解析 bracket 格式的 tool calls: [Called xxx with args: {...}]
    parse_bracket_tool_calls(&mut result);

    result
}

/// 在字节数组中查找子序列
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// 从字节数组中提取 JSON 对象字符串
fn extract_json_from_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes[0] != b'{' {
        return None;
    }

    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut end_pos = None;

    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match b {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => brace_count += 1,
            b'}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 {
                    end_pos = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    end_pos.and_then(|end| String::from_utf8(bytes[..end].to_vec()).ok())
}

/// 从字符串中提取完整的 JSON 对象 (保留用于兼容)
#[allow(dead_code)]
fn extract_json_object(s: &str) -> Option<&str> {
    if !s.starts_with('{') {
        return None;
    }

    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in s.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match c {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => brace_count += 1,
            '}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 {
                    return Some(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// 解析 bracket 格式的 tool calls
fn parse_bracket_tool_calls(result: &mut CWParsedResponse) {
    let re =
        regex::Regex::new(r"\[Called\s+(\w+)\s+with\s+args:\s*(\{[^}]*(?:\{[^}]*\}[^}]*)*\})\]")
            .ok();

    if let Some(re) = re {
        let mut to_remove = Vec::new();
        for cap in re.captures_iter(&result.content) {
            if let (Some(name), Some(args)) = (cap.get(1), cap.get(2)) {
                let tool_id = format!(
                    "call_{}",
                    &uuid::Uuid::new_v4().to_string().replace('-', "")[..8]
                );
                result.tool_calls.push(ToolCall {
                    id: tool_id,
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: name.as_str().to_string(),
                        arguments: args.as_str().to_string(),
                    },
                });
                if let Some(full_match) = cap.get(0) {
                    to_remove.push(full_match.as_str().to_string());
                }
            }
        }
        // 从 content 中移除 tool call 文本
        for s in to_remove {
            result.content = result.content.replace(&s, "");
        }
        result.content = result.content.trim().to_string();
    }
}

/// 列出所有可用路由
async fn list_routes(State(state): State<AppState>) -> impl IntoResponse {
    let routes = match &state.db {
        Some(db) => state
            .pool_service
            .get_available_routes(db, &state.base_url)
            .unwrap_or_default(),
        None => Vec::new(),
    };

    // 添加默认路由
    let mut all_routes = vec![RouteInfo {
        selector: "default".to_string(),
        provider_type: "kiro".to_string(),
        credential_count: 1,
        endpoints: vec![
            crate::models::route_model::RouteEndpoint {
                path: "/v1/messages".to_string(),
                protocol: "claude".to_string(),
                url: format!("{}/v1/messages", state.base_url),
            },
            crate::models::route_model::RouteEndpoint {
                path: "/v1/chat/completions".to_string(),
                protocol: "openai".to_string(),
                url: format!("{}/v1/chat/completions", state.base_url),
            },
        ],
        tags: vec!["默认".to_string()],
        enabled: true,
    }];
    all_routes.extend(routes);

    let response = RouteListResponse {
        base_url: state.base_url.clone(),
        default_provider: "kiro".to_string(),
        routes: all_routes,
    };

    Json(response)
}

/// 带选择器的 Anthropic messages 处理
async fn anthropic_messages_with_selector(
    State(state): State<AppState>,
    Path(selector): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AnthropicMessagesRequest>,
) -> Response {
    if let Err(e) = verify_api_key(&headers, &state.api_key).await {
        state.logs.write().await.add(
            "warn",
            &format!("Unauthorized request to /{}/v1/messages", selector),
        );
        return e.into_response();
    }

    state.logs.write().await.add(
        "info",
        &format!(
            "[REQ] POST /{}/v1/messages model={} stream={}",
            selector, request.model, request.stream
        ),
    );

    // 尝试解析凭证
    let credential = match &state.db {
        Some(db) => {
            // 首先尝试按名称查找
            if let Ok(Some(cred)) = state.pool_service.get_by_name(db, &selector) {
                Some(cred)
            }
            // 然后尝试按 UUID 查找
            else if let Ok(Some(cred)) = state.pool_service.get_by_uuid(db, &selector) {
                Some(cred)
            }
            // 最后尝试按 provider 类型轮询
            else if let Ok(Some(cred)) =
                state
                    .pool_service
                    .select_credential(db, &selector, Some(&request.model))
            {
                Some(cred)
            } else {
                None
            }
        }
        None => None,
    };

    match credential {
        Some(cred) => {
            state.logs.write().await.add(
                "info",
                &format!(
                    "[ROUTE] Using credential: type={} name={:?} uuid={}",
                    cred.provider_type,
                    cred.name,
                    &cred.uuid[..8]
                ),
            );

            // 根据凭证类型调用相应的 Provider
            call_provider_anthropic(&state, &cred, &request).await
        }
        None => {
            // 回退到默认 Kiro provider
            state.logs.write().await.add(
                "warn",
                &format!(
                    "[ROUTE] Credential not found for selector '{}', falling back to default",
                    selector
                ),
            );
            // 调用原有的 Kiro 处理逻辑
            anthropic_messages_internal(&state, &request).await
        }
    }
}

/// 带选择器的 OpenAI chat completions 处理
async fn chat_completions_with_selector(
    State(state): State<AppState>,
    Path(selector): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    if let Err(e) = verify_api_key(&headers, &state.api_key).await {
        state.logs.write().await.add(
            "warn",
            &format!("Unauthorized request to /{}/v1/chat/completions", selector),
        );
        return e.into_response();
    }

    state.logs.write().await.add(
        "info",
        &format!(
            "[REQ] POST /{}/v1/chat/completions model={} stream={}",
            selector, request.model, request.stream
        ),
    );

    // 尝试解析凭证
    let credential = match &state.db {
        Some(db) => {
            if let Ok(Some(cred)) = state.pool_service.get_by_name(db, &selector) {
                Some(cred)
            } else if let Ok(Some(cred)) = state.pool_service.get_by_uuid(db, &selector) {
                Some(cred)
            } else if let Ok(Some(cred)) =
                state
                    .pool_service
                    .select_credential(db, &selector, Some(&request.model))
            {
                Some(cred)
            } else {
                None
            }
        }
        None => None,
    };

    match credential {
        Some(cred) => {
            state.logs.write().await.add(
                "info",
                &format!(
                    "[ROUTE] Using credential: type={} name={:?} uuid={}",
                    cred.provider_type,
                    cred.name,
                    &cred.uuid[..8]
                ),
            );

            call_provider_openai(&state, &cred, &request).await
        }
        None => {
            state.logs.write().await.add(
                "warn",
                &format!(
                    "[ROUTE] Credential not found for selector '{}', falling back to default",
                    selector
                ),
            );
            chat_completions_internal(&state, &request).await
        }
    }
}

/// 内部 Anthropic messages 处理 (使用默认 Kiro)
async fn anthropic_messages_internal(
    state: &AppState,
    request: &AnthropicMessagesRequest,
) -> Response {
    // 检查 token
    {
        let _guard = state.kiro_refresh_lock.lock().await;
        let mut kiro = state.kiro.write().await;
        let needs_refresh =
            kiro.credentials.access_token.is_none() || kiro.is_token_expiring_soon();
        if needs_refresh {
            if let Err(e) = kiro.refresh_token().await {
                state
                    .logs
                    .write()
                    .await
                    .add("error", &format!("[AUTH] Token refresh failed: {e}"));
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {e}")}})),
                )
                    .into_response();
            }
        }
    }

    let openai_request = convert_anthropic_to_openai(request);
    let kiro = state.kiro.read().await;

    match kiro.call_api(&openai_request).await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                match resp.bytes().await {
                    Ok(bytes) => {
                        let body = String::from_utf8_lossy(&bytes).to_string();
                        let parsed = parse_cw_response(&body);
                        if request.stream {
                            build_anthropic_stream_response(&request.model, &parsed)
                        } else {
                            build_anthropic_response(&request.model, &parsed)
                        }
                    }
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": e.to_string()}})),
                    )
                        .into_response(),
                }
            } else {
                let body = resp.text().await.unwrap_or_default();
                (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(serde_json::json!({"error": {"message": format!("Upstream error: {}", body)}})),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": {"message": e.to_string()}})),
        )
            .into_response(),
    }
}

/// 内部 OpenAI chat completions 处理 (使用默认 Kiro)
async fn chat_completions_internal(state: &AppState, request: &ChatCompletionRequest) -> Response {
    {
        let _guard = state.kiro_refresh_lock.lock().await;
        let mut kiro = state.kiro.write().await;
        let needs_refresh =
            kiro.credentials.access_token.is_none() || kiro.is_token_expiring_soon();
        if needs_refresh {
            if let Err(e) = kiro.refresh_token().await {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {e}")}})),
                )
                    .into_response();
            }
        }
    }

    let kiro = state.kiro.read().await;
    match kiro.call_api(request).await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                match resp.text().await {
                    Ok(body) => {
                        let parsed = parse_cw_response(&body);
                        let has_tool_calls = !parsed.tool_calls.is_empty();

                        let message = if has_tool_calls {
                            serde_json::json!({
                                "role": "assistant",
                                "content": if parsed.content.is_empty() { serde_json::Value::Null } else { serde_json::json!(parsed.content) },
                                "tool_calls": parsed.tool_calls.iter().map(|tc| {
                                    serde_json::json!({
                                        "id": tc.id,
                                        "type": "function",
                                        "function": {
                                            "name": tc.function.name,
                                            "arguments": tc.function.arguments
                                        }
                                    })
                                }).collect::<Vec<_>>()
                            })
                        } else {
                            serde_json::json!({
                                "role": "assistant",
                                "content": parsed.content
                            })
                        };

                        let response = serde_json::json!({
                            "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                            "object": "chat.completion",
                            "created": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            "model": request.model,
                            "choices": [{
                                "index": 0,
                                "message": message,
                                "finish_reason": if has_tool_calls { "tool_calls" } else { "stop" }
                            }],
                            "usage": {
                                "prompt_tokens": 0,
                                "completion_tokens": 0,
                                "total_tokens": 0
                            }
                        });
                        Json(response).into_response()
                    }
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": e.to_string()}})),
                    )
                        .into_response(),
                }
            } else {
                let body = resp.text().await.unwrap_or_default();
                (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(serde_json::json!({"error": {"message": format!("Upstream error: {}", body)}})),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": {"message": e.to_string()}})),
        )
            .into_response(),
    }
}

use crate::models::provider_pool_model::{CredentialData, ProviderCredential};

/// 根据凭证调用 Provider (Anthropic 格式)
async fn call_provider_anthropic(
    state: &AppState,
    credential: &ProviderCredential,
    request: &AnthropicMessagesRequest,
) -> Response {
    match &credential.credential {
        CredentialData::KiroOAuth { creds_file_path } => {
            // 使用 TokenCacheService 获取有效 token
            let db = match &state.db {
                Some(db) => db,
                None => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": "Database not available"}})),
                    )
                        .into_response();
                }
            };

            // 获取缓存的 token
            let token = match state
                .token_cache
                .get_valid_token(db, &credential.uuid)
                .await
            {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("[POOL] Token cache miss, loading from source: {}", e);
                    // 回退到从源文件加载
                    let mut kiro = KiroProvider::new();
                    if let Err(e) = kiro.load_credentials_from_path(creds_file_path).await {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": {"message": format!("Failed to load Kiro credentials: {}", e)}})),
                        )
                            .into_response();
                    }
                    if let Err(e) = kiro.refresh_token().await {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {}", e)}})),
                        )
                            .into_response();
                    }
                    kiro.credentials.access_token.unwrap_or_default()
                }
            };

            // 使用获取到的 token 创建 KiroProvider
            let mut kiro = KiroProvider::new();
            kiro.credentials.access_token = Some(token);
            // 从源文件加载其他配置（region, profile_arn 等）
            let _ = kiro.load_credentials_from_path(creds_file_path).await;

            let openai_request = convert_anthropic_to_openai(request);
            let resp = match kiro.call_api(&openai_request).await {
                Ok(r) => r,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": e.to_string()}})),
                    )
                        .into_response();
                }
            };

            let status = resp.status();
            if status.is_success() {
                match resp.bytes().await {
                    Ok(bytes) => {
                        let body = String::from_utf8_lossy(&bytes).to_string();
                        let parsed = parse_cw_response(&body);
                        if request.stream {
                            build_anthropic_stream_response(&request.model, &parsed)
                        } else {
                            build_anthropic_response(&request.model, &parsed)
                        }
                    }
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": e.to_string()}})),
                    )
                        .into_response(),
                }
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                // Token 过期，强制刷新并重试
                tracing::info!(
                    "[POOL] Got {}, forcing token refresh for {}",
                    status,
                    &credential.uuid[..8]
                );

                let new_token = match state
                    .token_cache
                    .refresh_and_cache(db, &credential.uuid, true)
                    .await
                {
                    Ok(t) => t,
                    Err(e) => {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {}", e)}})),
                        )
                            .into_response();
                    }
                };

                // 使用新 token 重试
                kiro.credentials.access_token = Some(new_token);
                match kiro.call_api(&openai_request).await {
                    Ok(retry_resp) => {
                        if retry_resp.status().is_success() {
                            match retry_resp.bytes().await {
                                Ok(bytes) => {
                                    let body = String::from_utf8_lossy(&bytes).to_string();
                                    let parsed = parse_cw_response(&body);
                                    if request.stream {
                                        build_anthropic_stream_response(&request.model, &parsed)
                                    } else {
                                        build_anthropic_response(&request.model, &parsed)
                                    }
                                }
                                Err(e) => (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                                )
                                    .into_response(),
                            }
                        } else {
                            let body = retry_resp.text().await.unwrap_or_default();
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": {"message": format!("Retry failed: {}", body)}})),
                            )
                                .into_response()
                        }
                    }
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": e.to_string()}})),
                    )
                        .into_response(),
                }
            } else {
                let body = resp.text().await.unwrap_or_default();
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": body}})),
                )
                    .into_response()
            }
        }
        CredentialData::GeminiOAuth { .. } => {
            // Gemini OAuth 路由暂不支持
            (
                StatusCode::NOT_IMPLEMENTED,
                Json(serde_json::json!({"error": {"message": "Gemini OAuth routing not yet implemented. Use /v1/messages with Gemini models instead."}})),
            )
                .into_response()
        }
        CredentialData::QwenOAuth { .. } => {
            // Qwen OAuth 路由暂不支持
            (
                StatusCode::NOT_IMPLEMENTED,
                Json(serde_json::json!({"error": {"message": "Qwen OAuth routing not yet implemented. Use /v1/messages with Qwen models instead."}})),
            )
                .into_response()
        }
        CredentialData::AntigravityOAuth {
            creds_file_path,
            project_id,
        } => {
            let mut antigravity = AntigravityProvider::new();
            if let Err(e) = antigravity
                .load_credentials_from_path(creds_file_path)
                .await
            {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": format!("Failed to load Antigravity credentials: {}", e)}})),
                )
                    .into_response();
            }

            // 检查并刷新 token
            if antigravity.is_token_expiring_soon() {
                if let Err(e) = antigravity.refresh_token().await {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {}", e)}})),
                    )
                        .into_response();
                }
            }

            // 设置项目 ID
            if let Some(pid) = project_id {
                antigravity.project_id = Some(pid.clone());
            } else if let Err(e) = antigravity.discover_project().await {
                tracing::warn!("[Antigravity] Failed to discover project: {}", e);
            }

            // 先转换为 OpenAI 格式，再转换为 Antigravity 格式
            let openai_request = convert_anthropic_to_openai(request);
            let antigravity_request = convert_openai_to_antigravity(&openai_request);

            match antigravity
                .generate_content(&request.model, &antigravity_request)
                .await
            {
                Ok(resp) => {
                    // 转换为 OpenAI 格式，再构建 Anthropic 响应
                    let content = resp["candidates"][0]["content"]["parts"][0]["text"]
                        .as_str()
                        .unwrap_or("");
                    let parsed = CWParsedResponse {
                        content: content.to_string(),
                        tool_calls: Vec::new(),
                        usage_credits: 0.0,
                        context_usage_percentage: 0.0,
                    };
                    if request.stream {
                        build_anthropic_stream_response(&request.model, &parsed)
                    } else {
                        build_anthropic_response(&request.model, &parsed)
                    }
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                )
                    .into_response(),
            }
        }
        CredentialData::OpenAIKey { api_key, base_url } => {
            let openai = OpenAICustomProvider::with_config(api_key.clone(), base_url.clone());
            let openai_request = convert_anthropic_to_openai(request);
            match openai.call_api(&openai_request).await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.text().await {
                            Ok(body) => {
                                if let Ok(openai_resp) =
                                    serde_json::from_str::<serde_json::Value>(&body)
                                {
                                    let content = openai_resp["choices"][0]["message"]["content"]
                                        .as_str()
                                        .unwrap_or("");
                                    let parsed = CWParsedResponse {
                                        content: content.to_string(),
                                        tool_calls: Vec::new(),
                                        usage_credits: 0.0,
                                        context_usage_percentage: 0.0,
                                    };
                                    if request.stream {
                                        build_anthropic_stream_response(&request.model, &parsed)
                                    } else {
                                        build_anthropic_response(&request.model, &parsed)
                                    }
                                } else {
                                    (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(serde_json::json!({"error": {"message": "Failed to parse OpenAI response"}})),
                                    )
                                        .into_response()
                                }
                            }
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": {"message": e.to_string()}})),
                            )
                                .into_response(),
                        }
                    } else {
                        let body = resp.text().await.unwrap_or_default();
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": {"message": body}})),
                        )
                            .into_response()
                    }
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                )
                    .into_response(),
            }
        }
        CredentialData::ClaudeKey { api_key, base_url } => {
            let claude = ClaudeCustomProvider::with_config(api_key.clone(), base_url.clone());
            match claude.call_api(request).await {
                Ok(resp) => {
                    let status = resp.status();
                    match resp.text().await {
                        Ok(body) => {
                            if status.is_success() {
                                Response::builder()
                                    .status(StatusCode::OK)
                                    .header(header::CONTENT_TYPE, "application/json")
                                    .body(Body::from(body))
                                    .unwrap_or_else(|_| {
                                        (
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            Json(serde_json::json!({"error": {"message": "Failed to build response"}})),
                                        )
                                            .into_response()
                                    })
                            } else {
                                (
                                    StatusCode::from_u16(status.as_u16())
                                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                                    Json(serde_json::json!({"error": {"message": body}})),
                                )
                                    .into_response()
                            }
                        }
                        Err(e) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": {"message": e.to_string()}})),
                        )
                            .into_response(),
                    }
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                )
                    .into_response(),
            }
        }
    }
}

/// 根据凭证调用 Provider (OpenAI 格式)
async fn call_provider_openai(
    _state: &AppState,
    credential: &ProviderCredential,
    request: &ChatCompletionRequest,
) -> Response {
    match &credential.credential {
        CredentialData::KiroOAuth { creds_file_path } => {
            let mut kiro = KiroProvider::new();
            if let Err(e) = kiro.load_credentials_from_path(creds_file_path).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": format!("Failed to load Kiro credentials: {}", e)}})),
                )
                    .into_response();
            }
            if let Err(e) = kiro.refresh_token().await {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {}", e)}})),
                )
                    .into_response();
            }

            match kiro.call_api(request).await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.text().await {
                            Ok(body) => {
                                let parsed = parse_cw_response(&body);
                                let has_tool_calls = !parsed.tool_calls.is_empty();

                                let message = if has_tool_calls {
                                    serde_json::json!({
                                        "role": "assistant",
                                        "content": if parsed.content.is_empty() { serde_json::Value::Null } else { serde_json::json!(parsed.content) },
                                        "tool_calls": parsed.tool_calls.iter().map(|tc| {
                                            serde_json::json!({
                                                "id": tc.id,
                                                "type": "function",
                                                "function": {
                                                    "name": tc.function.name,
                                                    "arguments": tc.function.arguments
                                                }
                                            })
                                        }).collect::<Vec<_>>()
                                    })
                                } else {
                                    serde_json::json!({
                                        "role": "assistant",
                                        "content": parsed.content
                                    })
                                };

                                Json(serde_json::json!({
                                    "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                                    "object": "chat.completion",
                                    "created": std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs(),
                                    "model": request.model,
                                    "choices": [{
                                        "index": 0,
                                        "message": message,
                                        "finish_reason": if has_tool_calls { "tool_calls" } else { "stop" }
                                    }],
                                    "usage": {
                                        "prompt_tokens": 0,
                                        "completion_tokens": 0,
                                        "total_tokens": 0
                                    }
                                }))
                                .into_response()
                            }
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": {"message": e.to_string()}})),
                            )
                                .into_response(),
                        }
                    } else {
                        let body = resp.text().await.unwrap_or_default();
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": {"message": body}})),
                        )
                            .into_response()
                    }
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                )
                    .into_response(),
            }
        }
        CredentialData::GeminiOAuth { .. } => {
            (
                StatusCode::NOT_IMPLEMENTED,
                Json(serde_json::json!({"error": {"message": "Gemini OAuth routing not yet implemented."}})),
            )
                .into_response()
        }
        CredentialData::QwenOAuth { .. } => {
            (
                StatusCode::NOT_IMPLEMENTED,
                Json(serde_json::json!({"error": {"message": "Qwen OAuth routing not yet implemented."}})),
            )
                .into_response()
        }
        CredentialData::AntigravityOAuth { creds_file_path, project_id } => {
            let mut antigravity = AntigravityProvider::new();
            if let Err(e) = antigravity.load_credentials_from_path(creds_file_path).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": format!("Failed to load Antigravity credentials: {}", e)}})),
                )
                    .into_response();
            }

            // 检查并刷新 token
            if antigravity.is_token_expiring_soon() {
                if let Err(e) = antigravity.refresh_token().await {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({"error": {"message": format!("Token refresh failed: {}", e)}})),
                    )
                        .into_response();
                }
            }

            // 设置项目 ID
            if let Some(pid) = project_id {
                antigravity.project_id = Some(pid.clone());
            } else if let Err(e) = antigravity.discover_project().await {
                tracing::warn!("[Antigravity] Failed to discover project: {}", e);
            }

            // 转换请求格式
            let antigravity_request = convert_openai_to_antigravity(request);

            match antigravity.generate_content(&request.model, &antigravity_request).await {
                Ok(resp) => {
                    let openai_response = convert_antigravity_to_openai_response(&resp, &request.model);
                    Json(openai_response).into_response()
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                )
                    .into_response(),
            }
        }
        CredentialData::OpenAIKey { api_key, base_url } => {
            let openai = OpenAICustomProvider::with_config(api_key.clone(), base_url.clone());
            match openai.call_api(request).await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.text().await {
                            Ok(body) => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                                    Json(json).into_response()
                                } else {
                                    (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(serde_json::json!({"error": {"message": "Invalid JSON response"}})),
                                    )
                                        .into_response()
                                }
                            }
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": {"message": e.to_string()}})),
                            )
                                .into_response(),
                        }
                    } else {
                        let body = resp.text().await.unwrap_or_default();
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": {"message": body}})),
                        )
                            .into_response()
                    }
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                )
                    .into_response(),
            }
        }
        CredentialData::ClaudeKey { api_key, base_url } => {
            let claude = ClaudeCustomProvider::with_config(api_key.clone(), base_url.clone());
            match claude.call_openai_api(request).await {
                Ok(resp) => Json(resp).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": e.to_string()}})),
                )
                    .into_response(),
            }
        }
    }
}
