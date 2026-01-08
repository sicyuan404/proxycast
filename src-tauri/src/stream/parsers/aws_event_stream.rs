//! AWS Event Stream 解析器
//!
//! 解析 Kiro/CodeWhisperer 的 AWS Event Stream 二进制格式，
//! 输出统一的 `StreamEvent` 类型。
//!
//! # 协议格式
//!
//! CodeWhisperer 使用 AWS Event Stream 二进制格式，每个事件包含：
//! - `{"content": "文本内容"}` - 文本增量
//! - `{"toolUseId": "id", "name": "tool_name"}` - 工具调用开始
//! - `{"toolUseId": "id", "input": "部分JSON"}` - 工具参数增量
//! - `{"toolUseId": "id", "stop": true}` - 工具调用结束
//! - `{"stop": true}` - 流结束
//! - `{"usage": 0.34}` - Credits 使用量
//! - `{"contextUsagePercentage": 54.36}` - 上下文使用百分比

use crate::stream::events::{ContentBlockType, StopReason, StreamContext, StreamEvent};
use std::collections::HashMap;

/// 解析器状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserState {
    /// 等待数据
    Idle,
    /// 正在解析
    Parsing,
    /// 已完成
    Completed,
    /// 错误状态
    Error(String),
}

impl Default for ParserState {
    fn default() -> Self {
        Self::Idle
    }
}

/// 工具调用累积器
#[derive(Debug, Clone, Default)]
struct ToolAccumulator {
    /// 工具名称
    name: String,
    /// 累积的输入
    input: String,
    /// 内容块索引
    block_index: u32,
}

/// AWS Event Stream 解析器
///
/// 解析 CodeWhisperer 的 AWS Event Stream 格式，输出统一的 `StreamEvent`。
#[derive(Debug)]
pub struct AwsEventStreamParser {
    /// 缓冲区（用于处理部分 chunk）
    buffer: Vec<u8>,
    /// 当前状态
    state: ParserState,
    /// 工具调用累积器
    tool_accumulators: HashMap<String, ToolAccumulator>,
    /// 解析错误计数
    parse_error_count: u32,
    /// 最大缓冲区大小（防止内存耗尽）
    max_buffer_size: usize,
    /// 流上下文
    context: StreamContext,
    /// 是否已发送消息开始事件
    message_started: bool,
    /// 是否已发送消息结束事件
    message_stopped: bool,
    /// 是否在文本块中
    in_text_block: bool,
    /// 当前文本块索引
    text_block_index: Option<u32>,
}

impl Default for AwsEventStreamParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AwsEventStreamParser {
    /// 默认最大缓冲区大小 (1MB)
    pub const DEFAULT_MAX_BUFFER_SIZE: usize = 1024 * 1024;

    /// 创建新的解析器
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            state: ParserState::Idle,
            tool_accumulators: HashMap::new(),
            parse_error_count: 0,
            max_buffer_size: Self::DEFAULT_MAX_BUFFER_SIZE,
            context: StreamContext::new(),
            message_started: false,
            message_stopped: false,
            in_text_block: false,
            text_block_index: None,
        }
    }

    /// 创建带模型名称的解析器
    pub fn with_model(model: String) -> Self {
        let mut parser = Self::new();
        parser.context.model = Some(model);
        parser
    }

    /// 获取当前状态
    pub fn state(&self) -> &ParserState {
        &self.state
    }

    /// 获取解析错误计数
    pub fn parse_error_count(&self) -> u32 {
        self.parse_error_count
    }

    /// 获取缓冲区大小
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// 重置解析器状态
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = ParserState::Idle;
        self.tool_accumulators.clear();
        self.parse_error_count = 0;
        self.context = StreamContext::new();
        self.message_started = false;
        self.message_stopped = false;
        self.in_text_block = false;
        self.text_block_index = None;
    }

    /// 处理接收到的字节
    ///
    /// # 返回
    ///
    /// 解析出的 `StreamEvent` 列表
    pub fn process(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        if bytes.is_empty() {
            return Vec::new();
        }

        // 调试日志：记录接收到的字节
        tracing::info!(
            "[AWS_PARSER] 收到 {} 字节, 缓冲区当前 {} 字节",
            bytes.len(),
            self.buffer.len()
        );

        // 更新状态
        if self.state == ParserState::Idle {
            self.state = ParserState::Parsing;
        }

        // 检查缓冲区大小限制
        if self.buffer.len() + bytes.len() > self.max_buffer_size {
            self.parse_error_count += 1;
            tracing::error!(
                "[AWS_PARSER] 缓冲区溢出: {} + {} > {}",
                self.buffer.len(),
                bytes.len(),
                self.max_buffer_size
            );
            return vec![StreamEvent::Error {
                error_type: "buffer_overflow".to_string(),
                message: "缓冲区溢出".to_string(),
            }];
        }

        // 将新数据添加到缓冲区
        self.buffer.extend_from_slice(bytes);

        // 解析缓冲区中的所有完整 JSON 对象
        let events = self.parse_buffer();

        tracing::info!(
            "[AWS_PARSER] 解析出 {} 个事件, 缓冲区剩余 {} 字节",
            events.len(),
            self.buffer.len()
        );

        events
    }

    /// 完成解析
    ///
    /// 处理缓冲区中剩余的数据，并完成所有未完成的工具调用。
    pub fn finish(&mut self) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        // 尝试解析缓冲区中剩余的数据
        events.extend(self.parse_buffer());

        // 完成所有未完成的工具调用
        let has_tool_calls = !self.tool_accumulators.is_empty();
        for (id, accumulator) in self.tool_accumulators.drain() {
            if !accumulator.name.is_empty() {
                events.push(StreamEvent::ToolUseStop { id: id.clone() });
                events.push(StreamEvent::ContentBlockStop {
                    index: accumulator.block_index,
                });
            }
        }

        // 关闭文本块（如果有）
        if let Some(index) = self.text_block_index.take() {
            events.push(StreamEvent::ContentBlockStop { index });
        }

        // 如果消息已经开始但还没有发送 MessageStop，生成一个
        // 这确保了即使 Kiro 后端没有发送 stop 事件，客户端也能收到完整的响应
        if self.message_started && !self.message_stopped {
            tracing::info!("[AWS_PARSER] finish() 生成 MessageStop 事件");
            let stop_reason = if has_tool_calls {
                StopReason::ToolUse
            } else {
                StopReason::EndTurn
            };
            events.push(StreamEvent::MessageStop { stop_reason });
            self.message_stopped = true;
        }

        // 更新状态
        self.state = ParserState::Completed;

        events
    }

    /// 解析缓冲区中的数据
    fn parse_buffer(&mut self) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        let mut pos = 0;

        while pos < self.buffer.len() {
            // 查找下一个 JSON 对象的开始位置
            let start = match self.find_json_start(pos) {
                Some(s) => s,
                None => break,
            };

            // 提取 JSON 对象
            match self.extract_json(start) {
                Some((json_str, end_pos)) => {
                    // 解析 JSON 并生成事件
                    match self.parse_json_event(&json_str) {
                        Ok(event_list) => events.extend(event_list),
                        Err(e) => {
                            tracing::warn!("[AWS_PARSER] JSON 解析错误: {}", e);
                            self.parse_error_count += 1;
                            events.push(StreamEvent::Error {
                                error_type: "parse_error".to_string(),
                                message: e,
                            });
                        }
                    }
                    pos = end_pos;
                }
                None => {
                    // JSON 对象不完整，等待更多数据
                    break;
                }
            }
        }

        // 移除已处理的数据
        if pos > 0 {
            self.buffer.drain(..pos);
        }

        events
    }

    /// 查找 JSON 对象的开始位置
    ///
    /// AWS Event Stream 包含二进制头部，简单的 `{` 可能匹配到错误位置。
    /// 我们只搜索有效的 JSON 模式：
    /// - `{"content":` - 文本内容
    /// - `{"name":` - 工具调用开始（包含 toolUseId）
    /// - `{"toolUseId":` - 工具调用事件
    /// - `{"input":` - 工具参数续传
    /// - `{"stop":` - 停止事件
    /// - `{"usage":` - 使用量
    /// - `{"contextUsagePercentage":` - 上下文使用百分比
    /// - `{"followupPrompt":` - 后续提示（跳过）
    fn find_json_start(&self, from: usize) -> Option<usize> {
        let buffer = &self.buffer[from..];

        // 定义所有有效的 JSON 模式
        let patterns: &[&[u8]] = &[
            b"{\"content\":",
            b"{\"name\":",
            b"{\"toolUseId\":",
            b"{\"input\":",
            b"{\"stop\":",
            b"{\"usage\":",
            b"{\"contextUsagePercentage\":",
            b"{\"followupPrompt\":",
        ];

        // 查找所有模式的位置，返回最早出现的
        let mut earliest: Option<usize> = None;

        for pattern in patterns {
            if let Some(pos) = Self::find_pattern(buffer, pattern) {
                match earliest {
                    None => earliest = Some(pos),
                    Some(e) if pos < e => earliest = Some(pos),
                    _ => {}
                }
            }
        }

        earliest.map(|p| from + p)
    }

    /// 在缓冲区中查找模式
    fn find_pattern(buffer: &[u8], pattern: &[u8]) -> Option<usize> {
        if pattern.is_empty() || buffer.len() < pattern.len() {
            return None;
        }

        buffer
            .windows(pattern.len())
            .position(|window| window == pattern)
    }

    /// 从缓冲区中提取完整的 JSON 对象
    fn extract_json(&self, start: usize) -> Option<(String, usize)> {
        if start >= self.buffer.len() || self.buffer[start] != b'{' {
            return None;
        }

        let mut brace_count = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, &b) in self.buffer[start..].iter().enumerate() {
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
                        let end = start + i + 1;
                        let json_bytes = &self.buffer[start..end];
                        if let Ok(json_str) = String::from_utf8(json_bytes.to_vec()) {
                            return Some((json_str, end));
                        } else {
                            return None;
                        }
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// 解析 JSON 事件并生成 StreamEvent
    fn parse_json_event(&mut self, json_str: &str) -> Result<Vec<StreamEvent>, String> {
        // 调试日志：记录解析到的 JSON
        tracing::info!(
            "[AWS_PARSER] 解析 JSON: {}",
            if json_str.len() > 200 {
                format!("{}...", &json_str[..200])
            } else {
                json_str.to_string()
            }
        );

        let value: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("JSON 解析错误: {}", e))?;

        let mut events = Vec::new();

        // 如果还没发送消息开始事件，先发送
        if !self.message_started {
            self.message_started = true;
            let msg_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
            self.context.message_id = Some(msg_id.clone());
            events.push(StreamEvent::MessageStart {
                id: msg_id,
                model: self
                    .context
                    .model
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            });
        }

        // 处理 content 事件
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            // 跳过 followupPrompt
            if value.get("followupPrompt").is_none() {
                // 如果还没有文本块，创建一个
                if !self.in_text_block {
                    self.in_text_block = true;
                    let index = self.context.next_block_index();
                    self.text_block_index = Some(index);
                    events.push(StreamEvent::ContentBlockStart {
                        index,
                        block_type: ContentBlockType::Text,
                    });
                }

                events.push(StreamEvent::TextDelta {
                    text: content.to_string(),
                });
            }
        }
        // 处理 tool use 事件 (包含 toolUseId)
        else if let Some(tool_use_id) = value.get("toolUseId").and_then(|v| v.as_str()) {
            // 如果有文本块，先关闭它
            if let Some(index) = self.text_block_index.take() {
                self.in_text_block = false;
                events.push(StreamEvent::ContentBlockStop { index });
            }

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

            let tool_id = tool_use_id.to_string();

            // 获取或创建工具累积器
            let accumulator = self.tool_accumulators.entry(tool_id.clone()).or_default();

            // 如果有名称，这是工具调用开始
            if !name.is_empty() && accumulator.name.is_empty() {
                accumulator.name = name.clone();
                accumulator.block_index = self.context.next_block_index();
                self.context.add_tool_call(tool_id.clone());

                events.push(StreamEvent::ContentBlockStart {
                    index: accumulator.block_index,
                    block_type: ContentBlockType::ToolUse {
                        id: tool_id.clone(),
                        name: name.clone(),
                    },
                });

                events.push(StreamEvent::ToolUseStart {
                    id: tool_id.clone(),
                    name,
                });
            }

            // 如果有输入增量
            if !input_chunk.is_empty() {
                accumulator.input.push_str(&input_chunk);
                events.push(StreamEvent::ToolUseInputDelta {
                    id: tool_id.clone(),
                    partial_json: input_chunk,
                });
            }

            // 如果是 stop 事件
            if is_stop {
                if let Some(acc) = self.tool_accumulators.remove(&tool_id) {
                    self.context.remove_tool_call(&tool_id);
                    events.push(StreamEvent::ToolUseStop { id: tool_id });
                    events.push(StreamEvent::ContentBlockStop {
                        index: acc.block_index,
                    });
                }
            }
        }
        // 处理独立的 input 事件（工具调用的 input 续传，没有 toolUseId）
        // Kiro 的工具调用可能分多个事件发送：
        // 1. {"name":"xxx","toolUseId":"xxx"} - 开始
        // 2. {"input":"..."} - input 数据（可能多次）
        // 3. {"stop":true} - 结束
        else if let Some(input_chunk) = value.get("input").and_then(|v| v.as_str()) {
            // 找到当前活跃的工具调用（应该只有一个）
            if let Some((tool_id, accumulator)) = self.tool_accumulators.iter_mut().next() {
                let tool_id = tool_id.clone();
                accumulator.input.push_str(input_chunk);
                events.push(StreamEvent::ToolUseInputDelta {
                    id: tool_id,
                    partial_json: input_chunk.to_string(),
                });
            } else {
                tracing::warn!(
                    "[AWS_PARSER] 收到独立的 input 事件但没有活跃的工具调用: {}",
                    input_chunk
                );
            }
        }
        // 处理独立的 stop 事件
        else if value.get("stop").and_then(|v| v.as_bool()).unwrap_or(false) {
            // 首先检查是否有活跃的工具调用需要关闭
            // 这处理 Kiro 发送 {"stop":true} 来结束工具调用的情况
            if !self.tool_accumulators.is_empty() {
                // 关闭所有活跃的工具调用
                let tool_ids: Vec<String> = self.tool_accumulators.keys().cloned().collect();
                for tool_id in tool_ids {
                    if let Some(acc) = self.tool_accumulators.remove(&tool_id) {
                        self.context.remove_tool_call(&tool_id);
                        events.push(StreamEvent::ToolUseStop { id: tool_id });
                        events.push(StreamEvent::ContentBlockStop {
                            index: acc.block_index,
                        });
                    }
                }
            }

            // 关闭文本块（如果有）
            if let Some(index) = self.text_block_index.take() {
                self.in_text_block = false;
                events.push(StreamEvent::ContentBlockStop { index });
            }

            // 确定停止原因
            let stop_reason = if self.context.has_active_tool_calls() {
                StopReason::ToolUse
            } else {
                StopReason::EndTurn
            };

            events.push(StreamEvent::MessageStop { stop_reason });
            self.message_stopped = true;
        }
        // 处理 usage 事件
        else if let Some(usage) = value.get("usage").and_then(|v| v.as_f64()) {
            events.push(StreamEvent::BackendUsage {
                credits: usage,
                context_percentage: 0.0,
            });
        }
        // 处理 contextUsagePercentage 事件
        else if let Some(ctx_usage) = value.get("contextUsagePercentage").and_then(|v| v.as_f64())
        {
            events.push(StreamEvent::BackendUsage {
                credits: 0.0,
                context_percentage: ctx_usage,
            });
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_event() {
        let mut parser = AwsEventStreamParser::with_model("test-model".to_string());
        let events = parser.process(br#"{"content":"Hello"}"#);

        assert!(events.len() >= 2);
        assert!(
            matches!(&events[0], StreamEvent::MessageStart { model, .. } if model == "test-model")
        );
        assert!(matches!(
            &events[1],
            StreamEvent::ContentBlockStart {
                block_type: ContentBlockType::Text,
                ..
            }
        ));
        assert!(matches!(&events[2], StreamEvent::TextDelta { text } if text == "Hello"));
    }

    #[test]
    fn test_parse_tool_use_event() {
        let mut parser = AwsEventStreamParser::new();

        // 工具调用开始
        let events = parser.process(br#"{"toolUseId":"tool_123","name":"read_file"}"#);
        assert!(events.iter().any(|e| matches!(e, StreamEvent::ToolUseStart { id, name } if id == "tool_123" && name == "read_file")));

        // 工具参数增量
        let events = parser.process(br#"{"toolUseId":"tool_123","input":"{\"path\":"}"#);
        assert!(events.iter().any(|e| matches!(e, StreamEvent::ToolUseInputDelta { id, partial_json } if id == "tool_123" && partial_json == "{\"path\":")));

        // 工具调用结束
        let events = parser.process(br#"{"toolUseId":"tool_123","stop":true}"#);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolUseStop { id } if id == "tool_123")));
    }

    #[test]
    fn test_parse_stop_event() {
        let mut parser = AwsEventStreamParser::new();

        // 先发送一些内容
        let _ = parser.process(br#"{"content":"test"}"#);

        // 发送 stop 事件
        let events = parser.process(br#"{"stop":true}"#);
        assert!(events.iter().any(|e| matches!(
            e,
            StreamEvent::MessageStop {
                stop_reason: StopReason::EndTurn
            }
        )));
    }

    #[test]
    fn test_incremental_parsing() {
        let mut parser = AwsEventStreamParser::new();

        // 发送部分数据
        let events1 = parser.process(br#"{"con"#);
        assert!(events1.is_empty()); // 不完整，没有事件

        // 发送剩余数据
        let events2 = parser.process(br#"tent":"Hello"}"#);
        assert!(!events2.is_empty()); // 现在有事件了
    }
}
