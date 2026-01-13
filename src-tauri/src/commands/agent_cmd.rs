//! Agent 命令模块
//!
//! 提供原生 Agent 的 Tauri 命令（兼容旧 API）

use crate::agent::tools::{
    handle_term_scrollback_response, handle_terminal_command_response, GetScrollbackResponse,
    TerminalCommandResponse,
};
use crate::agent::{
    AgentMessage, AgentSession, ImageData, NativeAgentState, NativeChatRequest, ProviderType,
};
use crate::commands::network_cmd::get_local_url;
use crate::database::dao::agent::AgentDao;
use crate::database::DbConnection;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Agent 进程状态响应
#[derive(Debug, Serialize)]
pub struct AgentProcessStatus {
    pub running: bool,
    pub base_url: Option<String>,
    pub port: Option<u16>,
}

/// 创建会话响应
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub credential_name: String,
    pub credential_uuid: String,
    pub provider_type: String,
    pub model: Option<String>,
}

/// 启动 Agent（原生实现，无需外部进程）
#[tauri::command]
pub async fn agent_start_process(
    agent_state: State<'_, NativeAgentState>,
    app_state: State<'_, AppState>,
    _port: Option<u16>,
) -> Result<AgentProcessStatus, String> {
    tracing::info!("[Agent] 初始化原生 Agent");

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
        return Err("ProxyCast API Server 未运行，请先启动服务器".to_string());
    }

    let api_key = api_key.ok_or_else(|| "ProxyCast API Server 未配置 API Key".to_string())?;
    let base_url = get_local_url(&host, port);
    let provider_type = ProviderType::from_str(&default_provider);

    agent_state.init(
        base_url.clone(),
        api_key,
        provider_type,
        Some(default_provider),
    )?;

    Ok(AgentProcessStatus {
        running: true,
        base_url: Some(base_url),
        port: Some(port),
    })
}

/// 停止 Agent
#[tauri::command]
pub async fn agent_stop_process(agent_state: State<'_, NativeAgentState>) -> Result<(), String> {
    tracing::info!("[Agent] 停止原生 Agent");
    agent_state.reset();
    Ok(())
}

/// 获取 Agent 状态
#[tauri::command]
pub async fn agent_get_process_status(
    agent_state: State<'_, NativeAgentState>,
    app_state: State<'_, AppState>,
) -> Result<AgentProcessStatus, String> {
    let initialized = agent_state.is_initialized();

    if initialized {
        let state = app_state.read().await;
        let base_url = get_local_url(&state.config.server.host, state.config.server.port);
        Ok(AgentProcessStatus {
            running: true,
            base_url: Some(base_url),
            port: Some(state.config.server.port),
        })
    } else {
        Ok(AgentProcessStatus {
            running: false,
            base_url: None,
            port: None,
        })
    }
}

/// Skill 信息
#[derive(Debug, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: Option<String>,
    pub path: Option<String>,
}

/// 创建 Agent 会话
#[tauri::command]
pub async fn agent_create_session(
    agent_state: State<'_, NativeAgentState>,
    app_state: State<'_, AppState>,
    db: State<'_, DbConnection>,
    provider_type: String,
    model: Option<String>,
    system_prompt: Option<String>,
    skills: Option<Vec<SkillInfo>>,
) -> Result<CreateSessionResponse, String> {
    tracing::info!(
        "[Agent] 创建会话: provider_type={}, model={:?}, skills_count={:?}",
        provider_type,
        model,
        skills.as_ref().map(|s| s.len())
    );

    // 如果未初始化，自动初始化
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

    // 构建包含 Skills 的 System Prompt
    let final_system_prompt = build_system_prompt_with_skills(system_prompt, skills.as_ref());

    let session_id = agent_state.create_session(model.clone(), final_system_prompt.clone())?;

    // 保存会话到数据库
    let now = chrono::Utc::now().to_rfc3339();
    let session = AgentSession {
        id: session_id.clone(),
        model: model.clone().unwrap_or_else(|| "default".to_string()),
        messages: Vec::new(),
        system_prompt: final_system_prompt,
        created_at: now.clone(),
        updated_at: now,
    };

    {
        let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
        if let Err(e) = AgentDao::create_session(&conn, &session) {
            tracing::warn!("[Agent] 保存会话到数据库失败: {}", e);
        }
    }

    Ok(CreateSessionResponse {
        session_id,
        credential_name: "ProxyCast".to_string(),
        credential_uuid: "native-agent".to_string(),
        provider_type,
        model,
    })
}

/// 构建包含 Skills 的 System Prompt
fn build_system_prompt_with_skills(
    base_prompt: Option<String>,
    skills: Option<&Vec<SkillInfo>>,
) -> Option<String> {
    let skills_xml = match skills {
        Some(skills) if !skills.is_empty() => {
            let mut xml = String::from("<available_skills>\n");
            for skill in skills {
                xml.push_str("  <skill>\n");
                xml.push_str(&format!("    <name>{}</name>\n", skill.name));
                if let Some(desc) = &skill.description {
                    xml.push_str(&format!("    <description>{}</description>\n", desc));
                }
                if let Some(path) = &skill.path {
                    xml.push_str(&format!("    <location>{}</location>\n", path));
                }
                xml.push_str("  </skill>\n");
            }
            xml.push_str("</available_skills>\n\n");
            xml.push_str("当用户的请求匹配某个 Skill 的描述时，请使用该 Skill 来完成任务。\n");
            xml.push_str("如果需要使用 Skill，请先读取对应的 SKILL.md 文件获取详细指令。\n");
            Some(xml)
        }
        _ => None,
    };

    match (base_prompt, skills_xml) {
        (Some(base), Some(skills)) => Some(format!("{}\n\n{}", base, skills)),
        (Some(base), None) => Some(base),
        (None, Some(skills)) => Some(skills),
        (None, None) => None,
    }
}

/// 图片输入参数
#[derive(Debug, Deserialize)]
pub struct ImageInputParam {
    pub data: String,
    pub media_type: String,
}

/// 发送消息到 Agent
#[tauri::command]
pub async fn agent_send_message(
    agent_state: State<'_, NativeAgentState>,
    app_state: State<'_, AppState>,
    session_id: Option<String>,
    message: String,
    images: Option<Vec<ImageInputParam>>,
    model: Option<String>,
    web_search: Option<bool>,
    thinking: Option<bool>,
) -> Result<String, String> {
    let images_count = images.as_ref().map(|v| v.len()).unwrap_or(0);
    let images_sizes: Vec<usize> = images
        .as_ref()
        .map(|imgs| imgs.iter().map(|i| i.data.len()).collect())
        .unwrap_or_default();

    tracing::info!(
        "[Agent] 发送消息: len={}, session={:?}, images_count={}, images_sizes={:?}, web_search={:?}, thinking={:?}",
        message.len(),
        session_id,
        images_count,
        images_sizes,
        web_search,
        thinking
    );

    // 如果未初始化，自动初始化
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

    // 根据启用的模式构建最终消息
    let web_search_enabled = web_search.unwrap_or(false);
    let thinking_enabled = thinking.unwrap_or(false);

    let final_message = match (web_search_enabled, thinking_enabled) {
        (true, true) => format!(
            "[深度思考 + 联网搜索模式] 请深入分析问题，并搜索网络获取最新信息，然后给出详细的回答：\n\n{}",
            message
        ),
        (true, false) => format!(
            "[联网搜索模式] 请先搜索网络获取最新信息，然后回答以下问题：\n\n{}",
            message
        ),
        (false, true) => format!(
            "[深度思考模式] 请深入分析这个问题，考虑多个角度，给出详细的推理过程和结论：\n\n{}",
            message
        ),
        (false, false) => message,
    };

    let request = NativeChatRequest {
        session_id,
        message: final_message,
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

    let response = agent_state.chat(request).await?;

    if response.success {
        Ok(response.content)
    } else {
        Err(response.error.unwrap_or_else(|| "未知错误".to_string()))
    }
}

/// 会话信息
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub provider_type: String,
    pub model: Option<String>,
    pub created_at: String,
    pub last_activity: String,
    pub messages_count: usize,
}

/// 获取会话列表
#[tauri::command]
pub async fn agent_list_sessions(db: State<'_, DbConnection>) -> Result<Vec<SessionInfo>, String> {
    let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;

    let sessions =
        AgentDao::list_sessions(&conn).map_err(|e| format!("获取会话列表失败: {}", e))?;

    // 获取每个会话的消息数量
    let result: Vec<SessionInfo> = sessions
        .into_iter()
        .map(|s| {
            let messages_count = AgentDao::get_message_count(&conn, &s.id).unwrap_or(0);
            SessionInfo {
                session_id: s.id,
                provider_type: "native".to_string(),
                model: Some(s.model),
                created_at: s.created_at.clone(),
                last_activity: s.updated_at,
                messages_count,
            }
        })
        .collect();

    Ok(result)
}

/// 获取会话详情
#[tauri::command]
pub async fn agent_get_session(
    agent_state: State<'_, NativeAgentState>,
    session_id: String,
) -> Result<SessionInfo, String> {
    let session = agent_state
        .get_session(&session_id)?
        .ok_or_else(|| "会话不存在".to_string())?;

    Ok(SessionInfo {
        session_id: session.id,
        provider_type: "native".to_string(),
        model: Some(session.model),
        created_at: session.created_at.clone(),
        last_activity: session.created_at,
        messages_count: session.messages.len(),
    })
}

/// 删除会话
#[tauri::command]
pub async fn agent_delete_session(
    agent_state: State<'_, NativeAgentState>,
    db: State<'_, DbConnection>,
    session_id: String,
) -> Result<(), String> {
    // 从内存中删除
    agent_state.delete_session(&session_id);

    // 从数据库中删除
    let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
    AgentDao::delete_session(&conn, &session_id).map_err(|e| format!("删除会话失败: {}", e))?;

    Ok(())
}

/// 获取会话消息列表
#[tauri::command]
pub async fn agent_get_session_messages(
    db: State<'_, DbConnection>,
    session_id: String,
) -> Result<Vec<AgentMessage>, String> {
    let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;

    let messages =
        AgentDao::get_messages(&conn, &session_id).map_err(|e| format!("获取消息失败: {}", e))?;

    Ok(messages)
}

/// 处理终端命令响应
///
/// 前端在用户批准/拒绝命令后调用此命令，将结果传递给 TerminalTool
#[tauri::command]
pub async fn agent_terminal_command_response(
    request_id: String,
    success: bool,
    output: String,
    error: Option<String>,
    exit_code: Option<i32>,
    rejected: bool,
) -> Result<(), String> {
    tracing::info!(
        "[Agent] 收到终端命令响应: request_id={}, success={}, rejected={}",
        request_id,
        success,
        rejected
    );

    let response = TerminalCommandResponse {
        request_id,
        success,
        output,
        error,
        exit_code,
        rejected,
    };

    handle_terminal_command_response(response);

    Ok(())
}

/// 前端返回终端滚动缓冲区数据
#[tauri::command]
pub async fn agent_term_scrollback_response(
    request_id: String,
    success: bool,
    total_lines: usize,
    line_start: usize,
    line_end: usize,
    content: String,
    has_more: bool,
    error: Option<String>,
) -> Result<(), String> {
    tracing::info!(
        "[Agent] 收到终端滚动缓冲区响应: request_id={}, success={}, lines={}-{}",
        request_id,
        success,
        line_start,
        line_end
    );

    let response = GetScrollbackResponse {
        request_id,
        success,
        total_lines,
        line_start,
        line_end,
        content,
        has_more,
        error,
    };

    handle_term_scrollback_response(response);

    Ok(())
}
