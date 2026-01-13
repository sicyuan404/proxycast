//! 原生 Agent 命令模块
//!
//! 提供原生 Rust Agent 的 Tauri 命令，替代 aster sidecar 方案

use crate::agent::{
    AgentMessage, AgentSession, ImageData, MessageContent, NativeAgentState, NativeChatRequest,
    NativeChatResponse, ProviderType, StreamEvent, ToolLoopEngine,
};
use crate::commands::network_cmd::get_local_url;
use crate::database::dao::agent::AgentDao;
use crate::database::dao::api_key_provider::ApiKeyProviderDao;
use crate::database::DbConnection;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::mpsc;

#[derive(Debug, Serialize)]
pub struct NativeAgentStatus {
    pub initialized: bool,
    pub base_url: Option<String>,
}

#[tauri::command]
pub async fn native_agent_init(
    agent_state: State<'_, NativeAgentState>,
    app_state: State<'_, AppState>,
) -> Result<NativeAgentStatus, String> {
    tracing::info!("[NativeAgent] 初始化 Agent");

    let (host, port, api_key, running, default_provider, agent_config) = {
        let state = app_state.read().await;
        (
            state.config.server.host.clone(),
            state.config.server.port,
            state.running_api_key.clone(),
            state.running,
            state.config.routing.default_provider.clone(),
            state.config.agent.clone(),
        )
    };

    if !running {
        return Err("ProxyCast API Server 未运行，请先启动服务器".to_string());
    }

    let api_key = api_key.ok_or_else(|| "ProxyCast API Server 未配置 API Key".to_string())?;

    let base_url = get_local_url(&host, port);
    let provider_type = ProviderType::from_str(&default_provider);

    tracing::info!(
        "[NativeAgent] 初始化 Agent: base_url={}, provider={:?}, use_default_prompt={}",
        base_url,
        provider_type,
        agent_config.use_default_system_prompt
    );

    // 使用带配置的初始化方法
    agent_state.init_with_config(
        base_url.clone(),
        api_key,
        provider_type,
        Some(default_provider),
        &agent_config,
    )?;

    tracing::info!("[NativeAgent] Agent 初始化成功: {}", base_url);

    Ok(NativeAgentStatus {
        initialized: true,
        base_url: Some(base_url),
    })
}

#[tauri::command]
pub async fn native_agent_status(
    agent_state: State<'_, NativeAgentState>,
) -> Result<NativeAgentStatus, String> {
    Ok(NativeAgentStatus {
        initialized: agent_state.is_initialized(),
        base_url: None,
    })
}

#[tauri::command]
pub async fn native_agent_reset(agent_state: State<'_, NativeAgentState>) -> Result<(), String> {
    agent_state.reset();
    tracing::info!("[NativeAgent] Agent 已重置");
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ImageInputParam {
    pub data: String,
    pub media_type: String,
}

#[tauri::command]
pub async fn native_agent_chat(
    agent_state: State<'_, NativeAgentState>,
    app_state: State<'_, AppState>,
    message: String,
    model: Option<String>,
    images: Option<Vec<ImageInputParam>>,
) -> Result<NativeChatResponse, String> {
    tracing::info!(
        "[NativeAgent] 发送消息: message_len={}, model={:?}",
        message.len(),
        model
    );

    // 如果 Agent 未初始化，自动初始化
    if !agent_state.is_initialized() {
        let (host, port, api_key, running, default_provider) = {
            let state = app_state.read().await;
            (
                state.config.server.host.clone(),
                state.config.server.port,
                state.running_api_key.clone(),
                state.running,
                state.config.routing.default_provider.clone(),
            )
        };

        if !running {
            return Err("ProxyCast API Server 未运行".to_string());
        }

        let api_key = api_key.ok_or_else(|| "未配置 API Key".to_string())?;
        let base_url = get_local_url(&host, port);
        let provider_type = ProviderType::from_str(&default_provider);
        agent_state.init(base_url, api_key, provider_type, Some(default_provider))?;
    }

    let request = NativeChatRequest {
        session_id: None,
        message,
        model,
        images: images.map(|imgs| {
            imgs.into_iter()
                .map(|img| ImageData {
                    data: img.data,
                    media_type: img.media_type,
                })
                .collect()
        }),
        stream: false,
    };

    // 使用 chat_sync 方法避免跨 await 持有锁
    agent_state.chat(request).await
}

#[tauri::command]
pub async fn native_agent_chat_stream(
    app_handle: tauri::AppHandle,
    agent_state: State<'_, NativeAgentState>,
    app_state: State<'_, AppState>,
    db: State<'_, DbConnection>,
    message: String,
    event_name: String,
    session_id: Option<String>,
    model: Option<String>,
    images: Option<Vec<ImageInputParam>>,
    provider: Option<String>,
    terminal_mode: Option<bool>,
) -> Result<(), String> {
    let terminal_mode = terminal_mode.unwrap_or(false);
    tracing::info!(
        "[NativeAgent] 发送流式消息: message_len={}, model={:?}, provider={:?}, event={}, session={:?}, terminal_mode={}",
        message.len(),
        model,
        provider,
        event_name,
        session_id,
        terminal_mode
    );

    // 获取配置信息
    let (host, port, api_key, running, default_provider) = {
        let state = app_state.read().await;
        (
            state.config.server.host.clone(),
            state.config.server.port,
            state.running_api_key.clone(),
            state.running,
            state.config.routing.default_provider.clone(),
        )
    };

    if !running {
        return Err("ProxyCast API Server 未运行".to_string());
    }

    let api_key = api_key.ok_or_else(|| "未配置 API Key".to_string())?;

    // 使用前端传递的 provider，如果没有则使用默认值
    let provider_str = provider.unwrap_or(default_provider);

    // 尝试从数据库查询 Provider 的类型（用于确定协议）
    let provider_type = {
        let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
        if let Ok(Some(api_provider)) = ApiKeyProviderDao::get_provider_by_id(&conn, &provider_str)
        {
            // 根据 API Key Provider 的 type 确定协议
            let api_type = api_provider.provider_type.to_string();
            tracing::info!(
                "[NativeAgent] 从数据库获取 Provider 类型: {} -> {}",
                provider_str,
                api_type
            );
            ProviderType::from_str(&api_type)
        } else {
            // 数据库中没有找到，使用默认解析
            ProviderType::from_str(&provider_str)
        }
    };

    tracing::info!(
        "[NativeAgent] 使用 provider: {:?} (原始值: {})",
        provider_type,
        provider_str
    );

    // 如果 Agent 未初始化，或者 provider 发生变化，重新初始化
    // 使用 provider_str 而不是 provider_type 来判断，因为自定义 Provider 的 type 都是 OpenAI
    let need_reinit = if !agent_state.is_initialized() {
        tracing::info!("[NativeAgent] Agent 未初始化，需要初始化");
        true
    } else if let Some(current_provider_id) = agent_state.get_provider_id() {
        if current_provider_id != provider_str {
            tracing::info!(
                "[NativeAgent] Provider 发生变化: {} -> {}，需要重新初始化",
                current_provider_id,
                provider_str
            );
            true
        } else {
            false
        }
    } else {
        true
    };

    if need_reinit {
        let base_url = get_local_url(&host, port);
        agent_state.init(base_url, api_key, provider_type, Some(provider_str.clone()))?;
    }

    // 获取工具注册表（用于创建 ToolLoopEngine）
    // 如果是 terminal_mode，使用 TerminalTool 替代 BashTool
    let tool_registry = agent_state.get_tool_registry_with_mode(terminal_mode)?;

    // 保存用户消息到数据库
    let session_id_for_db = session_id.clone();
    let message_for_db = message.clone();
    let db_clone = Arc::clone(&db);
    if let Some(ref sid) = session_id_for_db {
        let conn = db_clone
            .lock()
            .map_err(|e| format!("数据库锁定失败: {}", e))?;
        let user_message = AgentMessage {
            role: "user".to_string(),
            content: MessageContent::Text(message_for_db.clone()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_calls: None,
            tool_call_id: None,
        };
        if let Err(e) = AgentDao::add_message(&conn, sid, &user_message) {
            tracing::warn!("[NativeAgent] 保存用户消息到数据库失败: {}", e);
        }
    }

    let request = NativeChatRequest {
        session_id, // 使用前端传递的 session_id 以保持上下文
        message,
        model,
        images: images.map(|imgs| {
            imgs.into_iter()
                .map(|img| ImageData {
                    data: img.data,
                    media_type: img.media_type,
                })
                .collect()
        }),
        stream: true,
    };

    // 克隆 agent_state 用于后台任务（共享 sessions）
    let agent_state_clone = agent_state.inner().clone();
    let session_id_for_task = session_id_for_db.clone();

    // 在后台任务中处理流式响应
    let event_name_clone = event_name.clone();
    eprintln!(
        "[native_agent_chat_stream] 启动后台任务, event_name={}",
        event_name_clone
    );
    tauri::async_runtime::spawn(async move {
        eprintln!("[native_agent_chat_stream] 后台任务开始执行");

        // 创建工具循环引擎（使用共享的 tool_registry）
        let tool_loop_engine = ToolLoopEngine::new(tool_registry);
        eprintln!("[native_agent_chat_stream] 工具循环引擎创建成功");

        let (tx, mut rx) = mpsc::channel::<StreamEvent>(100);

        // 用于收集完整的助手响应
        let mut full_content = String::new();

        // 使用 agent_state 的方法（共享 sessions）
        eprintln!(
            "[native_agent_chat_stream] 开始 chat_stream_with_tools, request.session_id={:?}",
            request.session_id
        );
        let stream_task = tokio::spawn(async move {
            agent_state_clone
                .chat_stream_with_tools(request, tx, &tool_loop_engine)
                .await
        });

        eprintln!("[native_agent_chat_stream] 开始接收流式事件...");
        // 注意：不要在收到 Done 事件后立即 break，因为工具循环可能还在执行
        // 继续接收直到 channel 关闭（stream_task 完成）
        while let Some(event) = rx.recv().await {
            eprintln!("[native_agent_chat_stream] 收到事件: {:?}", event);
            tracing::debug!(
                "[NativeAgent] 收到流式事件: {:?}, 发送到: {}",
                event,
                event_name_clone
            );

            // 收集文本增量
            if let StreamEvent::TextDelta { ref text } = event {
                full_content.push_str(text);
            }

            if let Err(e) = app_handle.emit(&event_name_clone, &event) {
                tracing::error!("[NativeAgent] 发送事件失败: {}", e);
                eprintln!("[native_agent_chat_stream] 发送事件失败: {}", e);
                break;
            }
            tracing::debug!("[NativeAgent] 事件发送成功");

            // 只在 Error 时 break，Done 不 break 因为工具循环可能还会发送更多事件
            if matches!(event, StreamEvent::Error { .. }) {
                tracing::info!("[NativeAgent] 流式响应错误，停止接收");
                eprintln!("[native_agent_chat_stream] 流式响应错误");
                break;
            }
        }
        eprintln!("[native_agent_chat_stream] channel 关闭，事件接收完成");

        eprintln!("[native_agent_chat_stream] 等待 stream_task 完成...");
        match stream_task.await {
            Ok(result) => {
                eprintln!("[native_agent_chat_stream] stream_task 完成: {:?}", result);

                // 保存助手消息到数据库
                if let Some(ref sid) = session_id_for_task {
                    if !full_content.is_empty() {
                        if let Ok(conn) = db_clone.lock() {
                            let assistant_message = AgentMessage {
                                role: "assistant".to_string(),
                                content: MessageContent::Text(full_content.clone()),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                tool_calls: None,
                                tool_call_id: None,
                            };
                            if let Err(e) = AgentDao::add_message(&conn, sid, &assistant_message) {
                                tracing::warn!("[NativeAgent] 保存助手消息到数据库失败: {}", e);
                            } else {
                                tracing::info!("[NativeAgent] 助手消息已保存到数据库");
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[native_agent_chat_stream] stream_task 错误: {}", e);
            }
        }
        eprintln!("[native_agent_chat_stream] 后台任务结束");
    });

    Ok(())
}

#[tauri::command]
pub async fn native_agent_create_session(
    agent_state: State<'_, NativeAgentState>,
    model: Option<String>,
    system_prompt: Option<String>,
) -> Result<String, String> {
    agent_state.create_session(model, system_prompt)
}

#[tauri::command]
pub async fn native_agent_get_session(
    agent_state: State<'_, NativeAgentState>,
    session_id: String,
) -> Result<Option<AgentSession>, String> {
    agent_state.get_session(&session_id)
}

#[tauri::command]
pub async fn native_agent_delete_session(
    agent_state: State<'_, NativeAgentState>,
    session_id: String,
) -> Result<bool, String> {
    Ok(agent_state.delete_session(&session_id))
}

#[tauri::command]
pub async fn native_agent_list_sessions(
    agent_state: State<'_, NativeAgentState>,
) -> Result<Vec<AgentSession>, String> {
    Ok(agent_state.list_sessions())
}
