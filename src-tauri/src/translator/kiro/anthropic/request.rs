//! Anthropic 请求直接转换为 CodeWhisperer 请求
//!
//! 直接将 Anthropic MessagesRequest 转换为 CodeWhisperer API 格式，
//! 无需经过 OpenAI 中间格式，减少转换开销。

use crate::models::anthropic::*;
use crate::models::codewhisperer::*;
use crate::translator::kiro::openai::request::{get_model_map, DEFAULT_MODEL};
use crate::translator::traits::{RequestTranslator, TranslateError};
use std::collections::HashSet;
use uuid::Uuid;

/// Anthropic 到 Kiro 请求转换器
#[derive(Debug, Clone)]
pub struct AnthropicRequestTranslator {
    /// 可选的 Profile ARN (AWS CodeWhisperer)
    pub profile_arn: Option<String>,
}

impl Default for AnthropicRequestTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl AnthropicRequestTranslator {
    /// 创建新的转换器
    pub fn new() -> Self {
        Self { profile_arn: None }
    }

    /// 使用 Profile ARN 创建转换器
    pub fn with_profile_arn(profile_arn: String) -> Self {
        Self {
            profile_arn: Some(profile_arn),
        }
    }
}

impl RequestTranslator for AnthropicRequestTranslator {
    type Input = AnthropicMessagesRequest;
    type Output = CodeWhispererRequest;
    type Error = TranslateError;

    fn translate_request(&self, request: Self::Input) -> Result<Self::Output, Self::Error> {
        Ok(convert_anthropic_to_codewhisperer(
            &request,
            self.profile_arn.clone(),
        ))
    }
}

// ============================================================================
// 内部类型
// ============================================================================

#[derive(Debug, Clone)]
struct ProcessedMessage {
    role: String,
    content: String,
    tool_uses: Option<Vec<CWToolUse>>,
    tool_results: Option<Vec<CWToolResult>>,
    images: Option<Vec<CWImage>>,
}

// ============================================================================
// 转换函数
// ============================================================================

/// 将 Anthropic MessagesRequest 直接转换为 CodeWhisperer 请求
pub fn convert_anthropic_to_codewhisperer(
    request: &AnthropicMessagesRequest,
    profile_arn: Option<String>,
) -> CodeWhispererRequest {
    let model_map = get_model_map();
    let cw_model = model_map
        .get(request.model.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    let conversation_id = Uuid::new_v4().to_string();

    // 提取 system prompt
    let mut system_prompt = extract_system_text(&request.system);

    // 处理 tool_choice: required - CodeWhisperer 不支持此参数，通过 prompt 注入强制
    if is_tool_choice_required(&request.tool_choice) && request.tools.is_some() {
        let tool_instruction = "\n\n[CRITICAL INSTRUCTION] You MUST use one of the provided tools to respond. Do NOT respond with plain text. Call a tool function immediately.";
        system_prompt.push_str(tool_instruction);
        tracing::info!("[KIRO_TRANSLATE] tool_choice=required detected in Anthropic request, injected tool instruction");
    }

    // 预处理消息
    let messages = preprocess_anthropic_messages(&request.messages);

    // 构建历史记录
    let mut history: Vec<HistoryItem> = Vec::new();
    let mut start_idx = 0;

    // 处理 system prompt - 合并到第一条用户消息
    if !system_prompt.is_empty() && !messages.is_empty() && messages[0].role == "user" {
        let first_content = &messages[0].content;
        let combined = format!("{system_prompt}\n\n{first_content}");

        let mut user_msg = UserInputMessage {
            content: combined,
            model_id: cw_model.clone(),
            origin: "AI_EDITOR".to_string(),
            images: messages[0].images.clone(), // 传递图片
            user_input_message_context: None,
        };

        if let Some(ref tool_results) = messages[0].tool_results {
            user_msg.user_input_message_context = Some(UserInputMessageContext {
                tools: None,
                tool_results: Some(tool_results.clone()),
            });
        }

        history.push(HistoryItem::User(UserHistoryItem {
            user_input_message: user_msg,
        }));
        start_idx = 1;
    }

    // 处理历史消息（除最后一条）
    for msg in messages
        .iter()
        .take(messages.len().saturating_sub(1))
        .skip(start_idx)
    {
        match msg.role.as_str() {
            "user" => {
                let content = if msg.content.is_empty() {
                    if msg.tool_results.is_some() {
                        "Tool results provided.".to_string()
                    } else {
                        "Continue".to_string()
                    }
                } else {
                    msg.content.clone()
                };

                let mut user_msg = UserInputMessage {
                    content,
                    model_id: cw_model.clone(),
                    origin: "AI_EDITOR".to_string(),
                    images: msg.images.clone(), // 传递图片
                    user_input_message_context: None,
                };

                if let Some(ref tool_results) = msg.tool_results {
                    user_msg.user_input_message_context = Some(UserInputMessageContext {
                        tools: None,
                        tool_results: Some(tool_results.clone()),
                    });
                }

                history.push(HistoryItem::User(UserHistoryItem {
                    user_input_message: user_msg,
                }));
            }
            "assistant" => {
                let content = if msg.content.is_empty() {
                    "I understand.".to_string()
                } else {
                    msg.content.clone()
                };

                history.push(HistoryItem::Assistant(AssistantHistoryItem {
                    assistant_response_message: AssistantResponseMessage {
                        content,
                        tool_uses: msg.tool_uses.clone(),
                    },
                }));
            }
            _ => {}
        }
    }

    // 修复历史记录交替顺序
    let history = fix_history_alternation(history, &cw_model);

    // 构建当前消息
    let (current_content, current_tool_results, current_images) =
        if let Some(last_msg) = messages.last() {
            if last_msg.role == "assistant" {
                ("Continue".to_string(), None, None)
            } else {
                let content = if last_msg.content.is_empty() {
                    if last_msg.tool_results.is_some() {
                        "Tool results provided.".to_string()
                    } else {
                        "Continue".to_string()
                    }
                } else {
                    last_msg.content.clone()
                };
                (
                    content,
                    last_msg.tool_results.clone(),
                    last_msg.images.clone(),
                )
            }
        } else {
            ("Continue".to_string(), None, None)
        };

    // 构建 tools
    let tools = convert_anthropic_tools(&request.tools);

    let user_input_message_context = if tools.is_some() || current_tool_results.is_some() {
        Some(UserInputMessageContext {
            tools,
            tool_results: current_tool_results,
        })
    } else {
        None
    };

    // 记录图片信息
    if let Some(ref imgs) = current_images {
        tracing::info!(
            "[KIRO_TRANSLATE] Current message contains {} image(s)",
            imgs.len()
        );
    }

    CodeWhispererRequest {
        conversation_state: ConversationState {
            chat_trigger_type: "MANUAL".to_string(),
            conversation_id,
            current_message: CurrentMessage {
                user_input_message: UserInputMessage {
                    content: current_content,
                    model_id: cw_model,
                    origin: "AI_EDITOR".to_string(),
                    images: current_images, // 传递当前消息的图片
                    user_input_message_context,
                },
            },
            history: if history.is_empty() {
                None
            } else {
                Some(history)
            },
        },
        profile_arn,
    }
}

/// 提取 system prompt 文本
fn extract_system_text(system: &Option<serde_json::Value>) -> String {
    match system {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| {
                if item.get("type") == Some(&serde_json::Value::String("text".to_string())) {
                    item.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// 预处理 Anthropic 消息
fn preprocess_anthropic_messages(messages: &[AnthropicMessage]) -> Vec<ProcessedMessage> {
    let mut result: Vec<ProcessedMessage> = Vec::new();

    for msg in messages {
        let processed = convert_anthropic_message(msg);
        result.extend(processed);
    }

    // 合并连续的 user 消息中的 tool_results
    let mut merged: Vec<ProcessedMessage> = Vec::new();
    let mut pending_tool_results: Vec<CWToolResult> = Vec::new();

    for msg in result {
        if msg.role == "user" {
            if let Some(ref tr) = msg.tool_results {
                pending_tool_results.extend(tr.clone());
            }
            if !msg.content.is_empty() || msg.tool_results.is_none() {
                // 去重 tool_results
                let mut seen_ids = HashSet::new();
                pending_tool_results.retain(|tr| seen_ids.insert(tr.tool_use_id.clone()));

                merged.push(ProcessedMessage {
                    role: msg.role,
                    content: if msg.content.is_empty() && !pending_tool_results.is_empty() {
                        "Tool results provided.".to_string()
                    } else {
                        msg.content
                    },
                    tool_uses: None,
                    tool_results: if pending_tool_results.is_empty() {
                        None
                    } else {
                        Some(pending_tool_results.clone())
                    },
                    images: msg.images, // 保留图片
                });
                pending_tool_results.clear();
            }
        } else {
            // 如果有待处理的 tool_results，先创建 user 消息
            if !pending_tool_results.is_empty() {
                let mut seen_ids = HashSet::new();
                pending_tool_results.retain(|tr| seen_ids.insert(tr.tool_use_id.clone()));

                merged.push(ProcessedMessage {
                    role: "user".to_string(),
                    content: "Tool results provided.".to_string(),
                    tool_uses: None,
                    tool_results: Some(pending_tool_results.clone()),
                    images: None,
                });
                pending_tool_results.clear();
            }
            merged.push(msg);
        }
    }

    // 处理末尾的 tool_results
    if !pending_tool_results.is_empty() {
        let mut seen_ids = HashSet::new();
        pending_tool_results.retain(|tr| seen_ids.insert(tr.tool_use_id.clone()));

        merged.push(ProcessedMessage {
            role: "user".to_string(),
            content: "Tool results provided.".to_string(),
            tool_uses: None,
            tool_results: Some(pending_tool_results),
            images: None,
        });
    }

    merged
}

/// 转换单条 Anthropic 消息
fn convert_anthropic_message(msg: &AnthropicMessage) -> Vec<ProcessedMessage> {
    let mut result: Vec<ProcessedMessage> = Vec::new();

    match &msg.content {
        serde_json::Value::String(s) => {
            result.push(ProcessedMessage {
                role: msg.role.clone(),
                content: s.clone(),
                tool_uses: None,
                tool_results: None,
                images: None,
            });
        }
        serde_json::Value::Array(parts) => {
            let mut text_parts: Vec<String> = Vec::new();
            let mut tool_uses: Vec<CWToolUse> = Vec::new();
            let mut tool_results: Vec<CWToolResult> = Vec::new();
            let mut images: Vec<CWImage> = Vec::new();

            for part in parts {
                let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match part_type {
                    "text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                    "image" => {
                        // 处理 Anthropic 格式的图片
                        // { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": "..." } }
                        if let Some(source) = part.get("source") {
                            let source_type =
                                source.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if source_type == "base64" {
                                let media_type = source
                                    .get("media_type")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("image/jpeg");
                                let data =
                                    source.get("data").and_then(|d| d.as_str()).unwrap_or("");

                                if !data.is_empty() {
                                    // 从 media_type 提取格式 (image/jpeg -> jpeg)
                                    let format =
                                        media_type.split('/').nth(1).unwrap_or("jpeg").to_string();

                                    images.push(CWImage {
                                        format,
                                        source: CWImageSource {
                                            bytes: data.to_string(),
                                        },
                                    });
                                    tracing::debug!(
                                        "[KIRO_TRANSLATE] Converted image: media_type={}",
                                        media_type
                                    );
                                }
                            }
                        }
                    }
                    "tool_use" => {
                        let default_id = format!("toolu_{}", &Uuid::new_v4().to_string()[..8]);
                        let id = part
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or(&default_id);
                        let name = part.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let input = part.get("input").cloned().unwrap_or(serde_json::json!({}));

                        tool_uses.push(CWToolUse {
                            tool_use_id: id.to_string(),
                            name: name.to_string(),
                            input,
                        });
                    }
                    "tool_result" => {
                        let tool_use_id = part
                            .get("tool_use_id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("");
                        let content_text = extract_tool_result_content(part.get("content"));
                        let is_error = part
                            .get("is_error")
                            .and_then(|e| e.as_bool())
                            .unwrap_or(false);

                        tool_results.push(CWToolResult {
                            tool_use_id: tool_use_id.to_string(),
                            content: vec![CWTextContent { text: content_text }],
                            status: if is_error {
                                "error".to_string()
                            } else {
                                "success".to_string()
                            },
                        });
                    }
                    _ => {}
                }
            }

            // 处理 assistant 消息
            if msg.role == "assistant" {
                result.push(ProcessedMessage {
                    role: "assistant".to_string(),
                    content: text_parts.join(""),
                    tool_uses: if tool_uses.is_empty() {
                        None
                    } else {
                        Some(tool_uses)
                    },
                    tool_results: None,
                    images: None, // assistant 消息不包含图片
                });
            }
            // 处理 user 消息
            else if msg.role == "user" {
                // 先添加 tool results
                if !tool_results.is_empty() {
                    result.push(ProcessedMessage {
                        role: "user".to_string(),
                        content: String::new(),
                        tool_uses: None,
                        tool_results: Some(tool_results),
                        images: None,
                    });
                }

                // 添加文本内容和图片
                if !text_parts.is_empty() || !images.is_empty() {
                    result.push(ProcessedMessage {
                        role: "user".to_string(),
                        content: text_parts.join(""),
                        tool_uses: None,
                        tool_results: None,
                        images: if images.is_empty() {
                            None
                        } else {
                            Some(images)
                        },
                    });
                }
            }
        }
        _ => {}
    }

    result
}

/// 提取 tool_result 内容
fn extract_tool_result_content(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| {
                if item.get("type") == Some(&serde_json::Value::String("text".to_string())) {
                    item.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// 转换 Anthropic tools 为 CodeWhisperer tools
fn convert_anthropic_tools(tools: &Option<Vec<AnthropicTool>>) -> Option<Vec<CWToolItem>> {
    tools.as_ref().map(|tools| {
        let mut cw_tools: Vec<CWToolItem> = Vec::new();
        let mut function_count = 0;

        for t in tools.iter() {
            // 处理特殊工具类型
            if t.name == "web_search" || t.name == "web_search_20250305" {
                cw_tools.push(CWToolItem::WebSearch(CWWebSearchTool {
                    tool_type: "web_search".to_string(),
                }));
                continue;
            }

            // 限制最多 50 个函数工具
            if function_count >= 50 {
                continue;
            }
            function_count += 1;

            let params = t
                .input_schema
                .clone()
                .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));

            let desc = t
                .description
                .clone()
                .unwrap_or_else(|| format!("Tool: {}", t.name));

            cw_tools.push(CWToolItem::Standard(CWTool {
                tool_specification: ToolSpecification {
                    name: t.name.clone(),
                    description: if desc.len() > 500 {
                        let truncated: String = desc.chars().take(497).collect();
                        format!("{}...", truncated)
                    } else {
                        desc
                    },
                    input_schema: InputSchema { json: params },
                },
            }));
        }

        cw_tools
    })
}

/// 修复历史记录，确保 user/assistant 严格交替
fn fix_history_alternation(history: Vec<HistoryItem>, model_id: &str) -> Vec<HistoryItem> {
    if history.is_empty() {
        return history;
    }

    let mut fixed: Vec<HistoryItem> = Vec::new();

    for item in history {
        match &item {
            HistoryItem::User(user_item) => {
                if let Some(HistoryItem::User(last_user)) = fixed.last_mut() {
                    let has_tool_results = user_item
                        .user_input_message
                        .user_input_message_context
                        .as_ref()
                        .map(|ctx| ctx.tool_results.is_some())
                        .unwrap_or(false);

                    if has_tool_results {
                        let new_results = user_item
                            .user_input_message
                            .user_input_message_context
                            .as_ref()
                            .and_then(|ctx| ctx.tool_results.clone())
                            .unwrap_or_default();

                        if let Some(ref mut ctx) =
                            last_user.user_input_message.user_input_message_context
                        {
                            if let Some(ref mut existing) = ctx.tool_results {
                                existing.extend(new_results);
                            } else {
                                ctx.tool_results = Some(new_results);
                            }
                        } else {
                            last_user.user_input_message.user_input_message_context =
                                Some(UserInputMessageContext {
                                    tools: None,
                                    tool_results: Some(new_results),
                                });
                        }
                        continue;
                    } else {
                        fixed.push(HistoryItem::Assistant(AssistantHistoryItem {
                            assistant_response_message: AssistantResponseMessage {
                                content: "I understand.".to_string(),
                                tool_uses: None,
                            },
                        }));
                    }
                }
                fixed.push(item);
            }
            HistoryItem::Assistant(_) => {
                if let Some(HistoryItem::Assistant(_)) = fixed.last() {
                    fixed.push(HistoryItem::User(UserHistoryItem {
                        user_input_message: UserInputMessage {
                            content: "Continue".to_string(),
                            model_id: model_id.to_string(),
                            origin: "AI_EDITOR".to_string(),
                            images: None,
                            user_input_message_context: None,
                        },
                    }));
                }
                if fixed.is_empty() {
                    fixed.push(HistoryItem::User(UserHistoryItem {
                        user_input_message: UserInputMessage {
                            content: "Continue".to_string(),
                            model_id: model_id.to_string(),
                            origin: "AI_EDITOR".to_string(),
                            images: None,
                            user_input_message_context: None,
                        },
                    }));
                }
                fixed.push(item);
            }
        }
    }

    // 确保以 assistant 结尾
    if let Some(HistoryItem::User(_)) = fixed.last() {
        fixed.push(HistoryItem::Assistant(AssistantHistoryItem {
            assistant_response_message: AssistantResponseMessage {
                content: "I understand.".to_string(),
                tool_uses: None,
            },
        }));
    }

    fixed
}

/// 检查 tool_choice 是否为 required
///
/// Anthropic tool_choice 可以是:
/// - {"type": "any"} - 必须调用工具
/// - {"type": "tool", "name": "xxx"} - 必须调用指定工具
fn is_tool_choice_required(tool_choice: &Option<serde_json::Value>) -> bool {
    match tool_choice {
        Some(serde_json::Value::Object(obj)) => {
            if let Some(serde_json::Value::String(t)) = obj.get("type") {
                t == "any" || t == "tool"
            } else {
                false
            }
        }
        // OpenAI 风格的 "required" 字符串
        Some(serde_json::Value::String(s)) => s == "required" || s == "any",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_simple_request() {
        let request = AnthropicMessagesRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: serde_json::json!("Hello"),
            }],
            system: None,
            max_tokens: Some(1024),
            stream: true,
            temperature: None,
            tools: None,
            tool_choice: None,
        };

        let translator = AnthropicRequestTranslator::new();
        let result = translator.translate_request(request);
        assert!(result.is_ok());

        let cw_request = result.unwrap();
        assert_eq!(
            cw_request
                .conversation_state
                .current_message
                .user_input_message
                .model_id,
            "CLAUDE_SONNET_4_5_20250929_V1_0"
        );
    }

    #[test]
    fn test_extract_system_text_string() {
        let system = Some(serde_json::json!("You are a helpful assistant."));
        let text = extract_system_text(&system);
        assert_eq!(text, "You are a helpful assistant.");
    }

    #[test]
    fn test_extract_system_text_array() {
        let system = Some(serde_json::json!([
            {"type": "text", "text": "Line 1"},
            {"type": "text", "text": "Line 2"}
        ]));
        let text = extract_system_text(&system);
        assert_eq!(text, "Line 1\nLine 2");
    }
}
