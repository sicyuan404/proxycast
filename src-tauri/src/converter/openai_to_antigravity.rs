//! OpenAI 格式转换为 Antigravity (Gemini) 格式
use crate::models::openai::*;
use serde::{Deserialize, Serialize};

/// Antigravity/Gemini 内容部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

/// Antigravity/Gemini 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<GeminiPart>,
}

/// Antigravity/Gemini 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiTool {
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiFunctionDeclaration {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Antigravity/Gemini 生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

/// Antigravity 请求体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityRequestBody {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GeminiTool>>,
}

/// 将 OpenAI ChatCompletionRequest 转换为 Antigravity 请求体
pub fn convert_openai_to_antigravity(request: &ChatCompletionRequest) -> serde_json::Value {
    let mut contents: Vec<GeminiContent> = Vec::new();
    let mut system_instruction: Option<GeminiContent> = None;

    // 处理消息
    for msg in &request.messages {
        match msg.role.as_str() {
            "system" => {
                let text = msg.get_content_text();
                if !text.is_empty() {
                    system_instruction = Some(GeminiContent {
                        role: "user".to_string(),
                        parts: vec![GeminiPart {
                            text: Some(text),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                        }],
                    });
                }
            }
            "user" => {
                let parts = convert_user_content(msg);
                if !parts.is_empty() {
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts,
                    });
                }
            }
            "assistant" => {
                let parts = convert_assistant_content(msg);
                if !parts.is_empty() {
                    contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts,
                    });
                }
            }
            "tool" => {
                // Tool 响应需要合并到 user 消息
                let tool_id = msg.tool_call_id.clone().unwrap_or_default();
                let content = msg.get_content_text();

                // 尝试解析为 JSON，否则包装为对象
                let response_value = serde_json::from_str(&content)
                    .unwrap_or_else(|_| serde_json::json!({ "result": content }));

                contents.push(GeminiContent {
                    role: "user".to_string(),
                    parts: vec![GeminiPart {
                        text: None,
                        inline_data: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            name: tool_id,
                            response: response_value,
                        }),
                    }],
                });
            }
            _ => {}
        }
    }

    // 构建生成配置
    let generation_config = Some(GeminiGenerationConfig {
        temperature: request.temperature,
        max_output_tokens: request.max_tokens.map(|t| t as i32),
        top_p: None,
        top_k: None,
        stop_sequences: None,
    });

    // 转换工具
    let tools = request.tools.as_ref().map(|tools| {
        vec![GeminiTool {
            function_declarations: tools
                .iter()
                .map(|t| GeminiFunctionDeclaration {
                    name: t.function.name.clone(),
                    description: t.function.description.clone(),
                    parameters: t.function.parameters.clone(),
                })
                .collect(),
        }]
    });

    let body = AntigravityRequestBody {
        contents,
        system_instruction,
        generation_config,
        tools,
    };

    // 包装为 Antigravity 请求格式
    serde_json::json!({
        "request": body
    })
}

/// 转换用户消息内容
fn convert_user_content(msg: &ChatMessage) -> Vec<GeminiPart> {
    let mut parts = Vec::new();

    match &msg.content {
        Some(MessageContent::Text(text)) => {
            parts.push(GeminiPart {
                text: Some(text.clone()),
                inline_data: None,
                function_call: None,
                function_response: None,
            });
        }
        Some(MessageContent::Parts(content_parts)) => {
            for part in content_parts {
                match part {
                    ContentPart::Text { text } => {
                        parts.push(GeminiPart {
                            text: Some(text.clone()),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                        });
                    }
                    ContentPart::ImageUrl { image_url } => {
                        // 处理 base64 图片
                        if let Some((mime, data)) = parse_data_url(&image_url.url) {
                            parts.push(GeminiPart {
                                text: None,
                                inline_data: Some(InlineData {
                                    mime_type: mime,
                                    data,
                                }),
                                function_call: None,
                                function_response: None,
                            });
                        }
                    }
                }
            }
        }
        None => {}
    }

    parts
}

/// 转换助手消息内容
fn convert_assistant_content(msg: &ChatMessage) -> Vec<GeminiPart> {
    let mut parts = Vec::new();

    // 文本内容
    let text = msg.get_content_text();
    if !text.is_empty() {
        parts.push(GeminiPart {
            text: Some(text),
            inline_data: None,
            function_call: None,
            function_response: None,
        });
    }

    // 工具调用
    if let Some(tool_calls) = &msg.tool_calls {
        for tc in tool_calls {
            let args: serde_json::Value =
                serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::json!({}));

            parts.push(GeminiPart {
                text: None,
                inline_data: None,
                function_call: Some(GeminiFunctionCall {
                    name: tc.function.name.clone(),
                    args,
                }),
                function_response: None,
            });
        }
    }

    parts
}

/// 解析 data URL
fn parse_data_url(url: &str) -> Option<(String, String)> {
    if url.starts_with("data:") {
        let parts: Vec<&str> = url.splitn(2, ',').collect();
        if parts.len() == 2 {
            let meta = parts[0].strip_prefix("data:")?;
            let mime = meta.split(';').next()?.to_string();
            let data = parts[1].to_string();
            return Some((mime, data));
        }
    }
    None
}

/// 将 Antigravity 响应转换为 OpenAI 格式
pub fn convert_antigravity_to_openai_response(
    antigravity_resp: &serde_json::Value,
    model: &str,
) -> serde_json::Value {
    let mut choices = Vec::new();

    if let Some(candidates) = antigravity_resp
        .get("candidates")
        .and_then(|c| c.as_array())
    {
        for (i, candidate) in candidates.iter().enumerate() {
            let mut content = String::new();
            let mut tool_calls: Vec<serde_json::Value> = Vec::new();

            if let Some(parts) = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        content.push_str(text);
                    }
                    if let Some(fc) = part.get("functionCall") {
                        let call_id =
                            format!("call_{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
                        tool_calls.push(serde_json::json!({
                            "id": call_id,
                            "type": "function",
                            "function": {
                                "name": fc.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                                "arguments": serde_json::to_string(fc.get("args").unwrap_or(&serde_json::json!({}))).unwrap_or_default()
                            }
                        }));
                    }
                }
            }

            let finish_reason = candidate
                .get("finishReason")
                .and_then(|r| r.as_str())
                .map(|r| match r {
                    "STOP" => "stop",
                    "MAX_TOKENS" => "length",
                    "SAFETY" => "content_filter",
                    "RECITATION" => "content_filter",
                    _ => "stop",
                })
                .unwrap_or("stop");

            let mut message = serde_json::json!({
                "role": "assistant",
                "content": if content.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(content) }
            });

            if !tool_calls.is_empty() {
                message["tool_calls"] = serde_json::json!(tool_calls);
            }

            choices.push(serde_json::json!({
                "index": i,
                "message": message,
                "finish_reason": finish_reason
            }));
        }
    }

    // 构建 usage
    let usage = antigravity_resp.get("usageMetadata").map(|u| {
        serde_json::json!({
            "prompt_tokens": u.get("promptTokenCount").and_then(|t| t.as_i64()).unwrap_or(0),
            "completion_tokens": u.get("candidatesTokenCount").and_then(|t| t.as_i64()).unwrap_or(0),
            "total_tokens": u.get("totalTokenCount").and_then(|t| t.as_i64()).unwrap_or(0)
        })
    });

    let mut response = serde_json::json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": choices
    });

    if let Some(u) = usage {
        response["usage"] = u;
    }

    response
}
