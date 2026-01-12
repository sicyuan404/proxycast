//! 会话管理器
//!
//! 根据请求内容生成稳定的会话指纹（Session Fingerprint），
//! 用于实现会话粘性和 Prompt Caching 优化。

use crate::models::openai::ChatCompletionRequest;
use sha2::{Digest, Sha256};

/// 会话管理器
pub struct SessionManager;

impl SessionManager {
    /// 根据 OpenAI 请求生成稳定的会话指纹
    ///
    /// 策略：
    /// 基于第一条用户消息内容 + 模型名称生成 SHA256 哈希
    ///
    /// # 参数
    /// - `request`: OpenAI 格式的请求
    ///
    /// # 返回
    /// 稳定的会话 ID，格式为 `sid-{hash前16位}`
    pub fn extract_session_id(request: &ChatCompletionRequest) -> String {
        // 智能内容指纹 (SHA256)
        let mut hasher = Sha256::new();

        // 混入模型名称增加区分度
        hasher.update(request.model.as_bytes());

        let mut content_found = false;
        for msg in &request.messages {
            if msg.role != "user" {
                continue;
            }

            let text = msg.get_content_text();
            let clean_text = text.trim();

            // 跳过过短的消息（可能是探测消息）或含有系统标签的消息
            if clean_text.len() > 10 && !clean_text.contains("<system-reminder>") {
                hasher.update(clean_text.as_bytes());
                content_found = true;
                break; // 只取第一条关键消息作为锚点
            }
        }

        if !content_found {
            // 如果没找到有意义的内容，退化为对最后一条消息进行哈希
            if let Some(last_msg) = request.messages.last() {
                hasher.update(last_msg.get_content_text().as_bytes());
            }
        }

        let hash = format!("{:x}", hasher.finalize());
        let sid = format!("sid-{}", &hash[..16]);

        tracing::debug!(
            "[SessionManager] Generated fingerprint: {} for model {}",
            sid,
            request.model
        );
        sid
    }

    /// 根据 JSON 请求生成稳定的会话指纹
    ///
    /// 用于处理原始 JSON 格式的请求
    pub fn extract_session_id_from_json(request: &serde_json::Value, model: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(model.as_bytes());

        let mut content_found = false;

        // 尝试从 messages 数组中提取用户消息
        if let Some(messages) = request.get("messages").and_then(|m| m.as_array()) {
            for msg in messages {
                if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
                    continue;
                }

                // 提取文本内容
                let text = if let Some(content) = msg.get("content") {
                    if let Some(s) = content.as_str() {
                        s.to_string()
                    } else if let Some(arr) = content.as_array() {
                        arr.iter()
                            .filter_map(|part| {
                                if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                                    part.get("text").and_then(|t| t.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let clean_text = text.trim();
                if clean_text.len() > 10 && !clean_text.contains("<system-reminder>") {
                    hasher.update(clean_text.as_bytes());
                    content_found = true;
                    break;
                }
            }
        }

        // 尝试从 Gemini 格式的 contents 数组中提取
        if !content_found {
            if let Some(contents) = request.get("contents").and_then(|c| c.as_array()) {
                for content in contents {
                    if content.get("role").and_then(|r| r.as_str()) != Some("user") {
                        continue;
                    }

                    if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
                        let text: String = parts
                            .iter()
                            .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join(" ");

                        let clean_text = text.trim();
                        if clean_text.len() > 10 && !clean_text.contains("<system-reminder>") {
                            hasher.update(clean_text.as_bytes());
                            content_found = true;
                            break;
                        }
                    }
                }
            }
        }

        if !content_found {
            // 兜底：对整个请求进行摘要
            hasher.update(request.to_string().as_bytes());
        }

        let hash = format!("{:x}", hasher.finalize());
        let sid = format!("sid-{}", &hash[..16]);

        tracing::debug!(
            "[SessionManager] Generated fingerprint from JSON: {} for model {}",
            sid,
            model
        );
        sid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::openai::{ChatCompletionRequest, ChatMessage};

    #[test]
    fn test_session_id_stability() {
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: Some(crate::models::openai::MessageContent::Text(
                    "Hello, how are you?".to_string(),
                )),
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stream: false,
            tools: None,
            tool_choice: None,
            reasoning_effort: None,
        };

        let sid1 = SessionManager::extract_session_id(&request);
        let sid2 = SessionManager::extract_session_id(&request);

        assert_eq!(sid1, sid2, "Same request should generate same session ID");
        assert!(
            sid1.starts_with("sid-"),
            "Session ID should start with 'sid-'"
        );
    }

    #[test]
    fn test_different_content_different_sid() {
        let request1 = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: Some(crate::models::openai::MessageContent::Text(
                    "Hello, how are you?".to_string(),
                )),
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stream: false,
            tools: None,
            tool_choice: None,
            reasoning_effort: None,
        };

        let request2 = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: Some(crate::models::openai::MessageContent::Text(
                    "What is the weather today?".to_string(),
                )),
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stream: false,
            tools: None,
            tool_choice: None,
            reasoning_effort: None,
        };

        let sid1 = SessionManager::extract_session_id(&request1);
        let sid2 = SessionManager::extract_session_id(&request2);

        assert_ne!(
            sid1, sid2,
            "Different content should generate different session IDs"
        );
    }
}
