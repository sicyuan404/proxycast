//! LLM Flow Monitor 核心数据模型
//!
//! 定义 LLM 请求/响应流的完整数据结构，参考 mitmproxy 的 Flow 模型设计。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ProviderType;

// ============================================================================
// 核心 Flow 结构
// ============================================================================

/// LLM 请求/响应流
///
/// 类似 mitmproxy 的 HTTPFlow，但专门针对 LLM API 优化。
/// 包含完整的请求信息、响应信息、元数据和时间戳。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMFlow {
    /// 唯一标识符
    pub id: String,
    /// 流类型
    pub flow_type: FlowType,
    /// 请求信息
    pub request: LLMRequest,
    /// 响应信息（可能为空，如请求失败或正在进行中）
    pub response: Option<LLMResponse>,
    /// 错误信息（如果发生错误）
    pub error: Option<FlowError>,
    /// 元数据
    pub metadata: FlowMetadata,
    /// 时间戳
    pub timestamps: FlowTimestamps,
    /// 流状态
    pub state: FlowState,
    /// 用户标记和注释
    pub annotations: FlowAnnotations,
}

impl LLMFlow {
    /// 创建新的 LLM Flow
    pub fn new(
        id: String,
        flow_type: FlowType,
        request: LLMRequest,
        metadata: FlowMetadata,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            flow_type,
            request: request.clone(),
            response: None,
            error: None,
            metadata,
            timestamps: FlowTimestamps {
                created: now,
                request_start: request.timestamp,
                request_end: None,
                response_start: None,
                response_end: None,
                duration_ms: 0,
                ttfb_ms: None,
            },
            state: FlowState::Pending,
            annotations: FlowAnnotations::default(),
        }
    }
}

/// 流类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowType {
    /// OpenAI Chat Completions
    ChatCompletions,
    /// Anthropic Messages
    AnthropicMessages,
    /// Gemini Generate Content
    GeminiGenerateContent,
    /// Embeddings
    Embeddings,
    /// 其他类型
    Other(String),
}

impl Default for FlowType {
    fn default() -> Self {
        FlowType::ChatCompletions
    }
}

/// 流状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowState {
    /// 等待响应
    Pending,
    /// 正在流式传输
    Streaming,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

impl Default for FlowState {
    fn default() -> Self {
        FlowState::Pending
    }
}

// ============================================================================
// 请求数据结构
// ============================================================================

/// LLM 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRequest {
    /// HTTP 方法
    pub method: String,
    /// 请求路径
    pub path: String,
    /// 请求头
    pub headers: HashMap<String, String>,
    /// 原始请求体（JSON）
    pub body: serde_json::Value,
    /// 解析后的消息列表
    pub messages: Vec<Message>,
    /// 系统提示词（如果有）
    pub system_prompt: Option<String>,
    /// 工具定义（如果有）
    pub tools: Option<Vec<ToolDefinition>>,
    /// 请求的模型名称
    pub model: String,
    /// 原始模型名称（别名解析前）
    pub original_model: Option<String>,
    /// 请求参数
    pub parameters: RequestParameters,
    /// 请求体大小（字节）
    pub size_bytes: usize,
    /// 请求开始时间戳
    pub timestamp: DateTime<Utc>,
}

impl Default for LLMRequest {
    fn default() -> Self {
        Self {
            method: "POST".to_string(),
            path: String::new(),
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            messages: Vec::new(),
            system_prompt: None,
            tools: None,
            model: String::new(),
            original_model: None,
            parameters: RequestParameters::default(),
            size_bytes: 0,
            timestamp: Utc::now(),
        }
    }
}

/// 消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色
    pub role: MessageRole,
    /// 消息内容
    pub content: MessageContent,
    /// 工具调用（如果有）
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 工具结果（如果有）
    pub tool_result: Option<ToolResult>,
    /// 消息名称（如果有）
    pub name: Option<String>,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            role: MessageRole::User,
            content: MessageContent::Text(String::new()),
            tool_calls: None,
            tool_result: None,
            name: None,
        }
    }
}

/// 消息角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// 系统消息
    System,
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 工具消息
    Tool,
    /// 函数消息（兼容旧版 OpenAI API）
    Function,
}

impl Default for MessageRole {
    fn default() -> Self {
        MessageRole::User
    }
}

/// 消息内容（支持多模态）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 纯文本内容
    Text(String),
    /// 多模态内容（文本、图片等）
    MultiModal(Vec<ContentPart>),
}

impl Default for MessageContent {
    fn default() -> Self {
        MessageContent::Text(String::new())
    }
}

impl MessageContent {
    /// 获取文本内容
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s),
            MessageContent::MultiModal(_) => None,
        }
    }

    /// 获取所有文本内容（包括多模态中的文本部分）
    pub fn get_all_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::MultiModal(parts) => parts
                .iter()
                .filter_map(|p| {
                    if let ContentPart::Text { text } = p {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// 内容部分（多模态消息的组成部分）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// 文本部分
    Text { text: String },
    /// 图片部分
    ImageUrl { image_url: ImageUrl },
    /// 图片数据（base64）
    Image {
        #[serde(skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

/// 图片 URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具类型（通常为 "function"）
    #[serde(rename = "type")]
    pub tool_type: String,
    /// 函数定义
    pub function: FunctionDefinition,
}

/// 函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// 函数名称
    pub name: String,
    /// 函数描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 参数 schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具调用 ID
    pub id: String,
    /// 工具类型
    #[serde(rename = "type")]
    pub tool_type: String,
    /// 函数调用详情
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// 函数名称
    pub name: String,
    /// 函数参数（JSON 字符串）
    pub arguments: String,
}

/// 工具结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 工具调用 ID
    pub tool_call_id: String,
    /// 结果内容
    pub content: String,
    /// 是否为错误结果
    #[serde(default)]
    pub is_error: bool,
}

/// 请求参数
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestParameters {
    /// 温度参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p 参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// 最大 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// 是否流式响应
    #[serde(default)]
    pub stream: bool,
    /// 其他参数
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ============================================================================
// 响应数据结构
// ============================================================================

/// LLM 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// HTTP 状态码
    pub status_code: u16,
    /// 状态文本
    pub status_text: String,
    /// 响应头
    pub headers: HashMap<String, String>,
    /// 原始响应体（完整 JSON，流式响应会被重建）
    pub body: serde_json::Value,
    /// 提取的文本内容
    pub content: String,
    /// 思维链内容（如果有）
    pub thinking: Option<ThinkingContent>,
    /// 工具调用（如果有）
    pub tool_calls: Vec<ToolCall>,
    /// Token 使用统计
    pub usage: TokenUsage,
    /// 停止原因
    pub stop_reason: Option<StopReason>,
    /// 响应体大小（字节）
    pub size_bytes: usize,
    /// 响应开始时间戳
    pub timestamp_start: DateTime<Utc>,
    /// 响应结束时间戳
    pub timestamp_end: DateTime<Utc>,
    /// 流式响应信息（如果是流式）
    pub stream_info: Option<StreamInfo>,
}

impl Default for LLMResponse {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            status_code: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            content: String::new(),
            thinking: None,
            tool_calls: Vec::new(),
            usage: TokenUsage::default(),
            stop_reason: None,
            size_bytes: 0,
            timestamp_start: now,
            timestamp_end: now,
            stream_info: None,
        }
    }
}

/// 思维链内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingContent {
    /// 思维链文本
    pub text: String,
    /// 思维链 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u32>,
    /// 签名（用于验证）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Token 使用统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    /// 输入 Token 数
    pub input_tokens: u32,
    /// 输出 Token 数
    pub output_tokens: u32,
    /// 缓存读取 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,
    /// 缓存写入 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u32>,
    /// 思维链 Token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u32>,
    /// 总 Token 数
    pub total_tokens: u32,
}

impl TokenUsage {
    /// 计算总 Token 数
    pub fn calculate_total(&mut self) {
        self.total_tokens = self.input_tokens + self.output_tokens;
    }
}

/// 停止原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// 正常结束
    Stop,
    /// 达到最大长度
    Length,
    /// 工具调用
    ToolCalls,
    /// 内容过滤
    ContentFilter,
    /// 函数调用（兼容旧版）
    FunctionCall,
    /// 结束 Token
    EndTurn,
    /// 其他原因
    Other(String),
}

/// 流式响应信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Chunk 数量
    pub chunk_count: u32,
    /// 首个 Chunk 延迟（毫秒）
    pub first_chunk_latency_ms: u64,
    /// 平均 Chunk 间隔（毫秒）
    pub avg_chunk_interval_ms: f64,
    /// 原始 Chunks（可选，根据配置决定是否保存）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_chunks: Option<Vec<StreamChunk>>,
}

/// 流式 Chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Chunk 索引
    pub index: u32,
    /// 事件类型（SSE event）
    pub event: Option<String>,
    /// 数据内容
    pub data: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 解析后的内容增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_delta: Option<String>,
    /// 解析后的工具调用增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_delta: Option<ToolCallDelta>,
    /// 解析后的思维链增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_delta: Option<String>,
}

/// 工具调用增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    /// 工具调用索引
    pub index: u32,
    /// 工具调用 ID（首次出现时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 函数名称（首次出现时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_name: Option<String>,
    /// 参数增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments_delta: Option<String>,
}

// ============================================================================
// 元数据结构
// ============================================================================

/// 流元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowMetadata {
    /// 提供商类型
    pub provider: ProviderType,
    /// 提供商 ID（实际的 provider ID，如 "deepseek", "moonshot" 等）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// 凭证 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    /// 凭证名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_name: Option<String>,
    /// 重试次数
    #[serde(default)]
    pub retry_count: u32,
    /// 客户端信息
    pub client_info: ClientInfo,
    /// 路由信息
    pub routing_info: RoutingInfo,
    /// 注入的参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injected_params: Option<HashMap<String, serde_json::Value>>,
    /// 上下文使用百分比
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_usage_percentage: Option<f32>,
}

impl Default for FlowMetadata {
    fn default() -> Self {
        Self {
            provider: ProviderType::Kiro,
            provider_id: None,
            credential_id: None,
            credential_name: None,
            retry_count: 0,
            client_info: ClientInfo::default(),
            routing_info: RoutingInfo::default(),
            injected_params: None,
            context_usage_percentage: None,
        }
    }
}

/// 客户端信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientInfo {
    /// 客户端 IP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// User-Agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// 请求 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// 路由信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutingInfo {
    /// 目标 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    /// 使用的路由规则
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_rule: Option<String>,
    /// 负载均衡策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_balance_strategy: Option<String>,
}

/// 时间戳集合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowTimestamps {
    /// 创建时间
    pub created: DateTime<Utc>,
    /// 请求开始时间
    pub request_start: DateTime<Utc>,
    /// 请求结束时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_end: Option<DateTime<Utc>>,
    /// 响应开始时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_start: Option<DateTime<Utc>>,
    /// 响应结束时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_end: Option<DateTime<Utc>>,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// 首字节时间（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttfb_ms: Option<u64>,
}

impl Default for FlowTimestamps {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            created: now,
            request_start: now,
            request_end: None,
            response_start: None,
            response_end: None,
            duration_ms: 0,
            ttfb_ms: None,
        }
    }
}

impl FlowTimestamps {
    /// 计算耗时
    pub fn calculate_duration(&mut self) {
        if let Some(end) = self.response_end {
            self.duration_ms = (end - self.request_start).num_milliseconds().max(0) as u64;
        }
    }

    /// 计算 TTFB
    pub fn calculate_ttfb(&mut self) {
        if let Some(start) = self.response_start {
            self.ttfb_ms = Some((start - self.request_start).num_milliseconds().max(0) as u64);
        }
    }
}

/// 用户标注
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlowAnnotations {
    /// 标记（如 ⭐、🔴、🟢）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    /// 评论
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 是否收藏
    #[serde(default)]
    pub starred: bool,
}

// ============================================================================
// 错误结构
// ============================================================================

/// 流错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowError {
    /// 错误类型
    pub error_type: FlowErrorType,
    /// 错误消息
    pub message: String,
    /// HTTP 状态码（如果有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    /// 原始响应（如果有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<String>,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 是否可重试
    pub retryable: bool,
}

impl FlowError {
    /// 创建新的错误
    pub fn new(error_type: FlowErrorType, message: impl Into<String>) -> Self {
        Self {
            error_type,
            message: message.into(),
            status_code: None,
            raw_response: None,
            timestamp: Utc::now(),
            retryable: false,
        }
    }

    /// 设置状态码
    pub fn with_status_code(mut self, code: u16) -> Self {
        self.status_code = Some(code);
        self
    }

    /// 设置原始响应
    pub fn with_raw_response(mut self, response: impl Into<String>) -> Self {
        self.raw_response = Some(response.into());
        self
    }

    /// 设置是否可重试
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }
}

/// 错误类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowErrorType {
    /// 网络错误
    Network,
    /// 超时
    Timeout,
    /// 认证错误
    Authentication,
    /// 速率限制
    RateLimit,
    /// 内容过滤
    ContentFilter,
    /// 服务器错误
    ServerError,
    /// 请求错误
    BadRequest,
    /// 模型不可用
    ModelUnavailable,
    /// Token 限制超出
    TokenLimitExceeded,
    /// 请求被取消（用户拦截后取消）
    Cancelled,
    /// 其他错误
    Other,
}

impl Default for FlowErrorType {
    fn default() -> Self {
        FlowErrorType::Other
    }
}

impl FlowErrorType {
    /// 根据 HTTP 状态码推断错误类型
    pub fn from_status_code(code: u16) -> Self {
        match code {
            401 | 403 => FlowErrorType::Authentication,
            429 => FlowErrorType::RateLimit,
            400 => FlowErrorType::BadRequest,
            404 => FlowErrorType::ModelUnavailable,
            500..=599 => FlowErrorType::ServerError,
            _ => FlowErrorType::Other,
        }
    }

    /// 判断是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            FlowErrorType::Network
                | FlowErrorType::Timeout
                | FlowErrorType::RateLimit
                | FlowErrorType::ServerError
        )
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_creation() {
        let request = LLMRequest {
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            model: "gpt-4".to_string(),
            ..Default::default()
        };

        let metadata = FlowMetadata {
            provider: ProviderType::OpenAI,
            ..Default::default()
        };

        let flow = LLMFlow::new(
            "test-id".to_string(),
            FlowType::ChatCompletions,
            request,
            metadata,
        );

        assert_eq!(flow.id, "test-id");
        assert_eq!(flow.state, FlowState::Pending);
        assert_eq!(flow.flow_type, FlowType::ChatCompletions);
        assert!(flow.response.is_none());
        assert!(flow.error.is_none());
    }

    #[test]
    fn test_message_content_text() {
        let content = MessageContent::Text("Hello, world!".to_string());
        assert_eq!(content.as_text(), Some("Hello, world!"));
        assert_eq!(content.get_all_text(), "Hello, world!");
    }

    #[test]
    fn test_message_content_multimodal() {
        let content = MessageContent::MultiModal(vec![
            ContentPart::Text {
                text: "First part".to_string(),
            },
            ContentPart::Text {
                text: "Second part".to_string(),
            },
        ]);
        assert!(content.as_text().is_none());
        assert_eq!(content.get_all_text(), "First part\nSecond part");
    }

    #[test]
    fn test_token_usage_calculate_total() {
        let mut usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            ..Default::default()
        };
        usage.calculate_total();
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_flow_error_type_from_status_code() {
        assert_eq!(
            FlowErrorType::from_status_code(401),
            FlowErrorType::Authentication
        );
        assert_eq!(
            FlowErrorType::from_status_code(429),
            FlowErrorType::RateLimit
        );
        assert_eq!(
            FlowErrorType::from_status_code(500),
            FlowErrorType::ServerError
        );
        assert_eq!(FlowErrorType::from_status_code(200), FlowErrorType::Other);
    }

    #[test]
    fn test_flow_error_type_is_retryable() {
        assert!(FlowErrorType::Network.is_retryable());
        assert!(FlowErrorType::Timeout.is_retryable());
        assert!(FlowErrorType::RateLimit.is_retryable());
        assert!(FlowErrorType::ServerError.is_retryable());
        assert!(!FlowErrorType::Authentication.is_retryable());
        assert!(!FlowErrorType::BadRequest.is_retryable());
    }

    #[test]
    fn test_flow_timestamps_calculate() {
        let start = Utc::now();
        let response_start = start + chrono::Duration::milliseconds(100);
        let end = start + chrono::Duration::milliseconds(500);

        let mut timestamps = FlowTimestamps {
            created: start,
            request_start: start,
            request_end: Some(start + chrono::Duration::milliseconds(50)),
            response_start: Some(response_start),
            response_end: Some(end),
            duration_ms: 0,
            ttfb_ms: None,
        };

        timestamps.calculate_duration();
        timestamps.calculate_ttfb();

        assert_eq!(timestamps.duration_ms, 500);
        assert_eq!(timestamps.ttfb_ms, Some(100));
    }

    #[test]
    fn test_flow_error_builder() {
        let error = FlowError::new(FlowErrorType::RateLimit, "Too many requests")
            .with_status_code(429)
            .with_retryable(true);

        assert_eq!(error.error_type, FlowErrorType::RateLimit);
        assert_eq!(error.message, "Too many requests");
        assert_eq!(error.status_code, Some(429));
        assert!(error.retryable);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let flow = LLMFlow::new(
            "test-id".to_string(),
            FlowType::ChatCompletions,
            LLMRequest::default(),
            FlowMetadata::default(),
        );

        let json = serde_json::to_string(&flow).unwrap();
        let deserialized: LLMFlow = serde_json::from_str(&json).unwrap();

        assert_eq!(flow.id, deserialized.id);
        assert_eq!(flow.state, deserialized.state);
    }
}

// ============================================================================
// 属性测试模块
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // ========================================================================
    // 生成器
    // ========================================================================

    /// 生成随机的 ProviderType
    fn arb_provider_type() -> impl Strategy<Value = ProviderType> {
        prop_oneof![
            Just(ProviderType::Kiro),
            Just(ProviderType::Gemini),
            Just(ProviderType::Qwen),
            Just(ProviderType::OpenAI),
            Just(ProviderType::Claude),
            Just(ProviderType::Antigravity),
            Just(ProviderType::Vertex),
            Just(ProviderType::GeminiApiKey),
            Just(ProviderType::Codex),
            Just(ProviderType::ClaudeOAuth),
            Just(ProviderType::IFlow),
        ]
    }

    /// 生成随机的 FlowType
    fn arb_flow_type() -> impl Strategy<Value = FlowType> {
        prop_oneof![
            Just(FlowType::ChatCompletions),
            Just(FlowType::AnthropicMessages),
            Just(FlowType::GeminiGenerateContent),
            Just(FlowType::Embeddings),
            "[a-z]{3,10}".prop_map(FlowType::Other),
        ]
    }

    /// 生成随机的 MessageRole
    fn arb_message_role() -> impl Strategy<Value = MessageRole> {
        prop_oneof![
            Just(MessageRole::System),
            Just(MessageRole::User),
            Just(MessageRole::Assistant),
            Just(MessageRole::Tool),
            Just(MessageRole::Function),
        ]
    }

    /// 生成随机的 MessageContent
    fn arb_message_content() -> impl Strategy<Value = MessageContent> {
        prop_oneof![
            ".*".prop_map(MessageContent::Text),
            prop::collection::vec(
                "[a-zA-Z0-9 ]{1,50}".prop_map(|text| ContentPart::Text { text }),
                1..5
            )
            .prop_map(MessageContent::MultiModal),
        ]
    }

    /// 生成随机的 Message
    fn arb_message() -> impl Strategy<Value = Message> {
        (arb_message_role(), arb_message_content()).prop_map(|(role, content)| Message {
            role,
            content,
            tool_calls: None,
            tool_result: None,
            name: None,
        })
    }

    /// 生成随机的 RequestParameters
    fn arb_request_parameters() -> impl Strategy<Value = RequestParameters> {
        (
            prop::option::of(0.0f32..2.0f32),
            prop::option::of(0.0f32..1.0f32),
            prop::option::of(1u32..4096u32),
            any::<bool>(),
        )
            .prop_map(
                |(temperature, top_p, max_tokens, stream)| RequestParameters {
                    temperature,
                    top_p,
                    max_tokens,
                    stop: None,
                    stream,
                    extra: HashMap::new(),
                },
            )
    }

    /// 生成随机的 LLMRequest
    fn arb_llm_request() -> impl Strategy<Value = LLMRequest> {
        (
            "[a-z]{3,20}",                              // model
            prop::collection::vec(arb_message(), 0..5), // messages
            arb_request_parameters(),                   // parameters
            prop::option::of("[a-zA-Z0-9 ]{10,100}"),   // system_prompt
        )
            .prop_map(|(model, messages, parameters, system_prompt)| LLMRequest {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                headers: HashMap::new(),
                body: serde_json::Value::Null,
                messages,
                system_prompt,
                tools: None,
                model,
                original_model: None,
                parameters,
                size_bytes: 0,
                timestamp: Utc::now(),
            })
    }

    /// 生成随机的 FlowMetadata
    fn arb_flow_metadata() -> impl Strategy<Value = FlowMetadata> {
        (
            arb_provider_type(),
            prop::option::of("[a-f0-9]{8}"),
            prop::option::of("[a-zA-Z0-9_]{3,20}"),
        )
            .prop_map(|(provider, credential_id, credential_name)| FlowMetadata {
                provider,
                provider_id: None,
                credential_id,
                credential_name,
                retry_count: 0,
                client_info: ClientInfo::default(),
                routing_info: RoutingInfo::default(),
                injected_params: None,
                context_usage_percentage: None,
            })
    }

    /// 生成随机的 Flow ID
    fn arb_flow_id() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}"
    }

    // ========================================================================
    // 属性测试
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: llm-flow-monitor, Property 1: Flow 创建正确性**
        /// **Validates: Requirements 1.1, 1.2**
        ///
        /// *对于任意* 有效的 API 请求，当 Flow_Monitor 创建新的 LLM_Flow 时，
        /// 该 Flow 应该具有唯一的 ID、pending 状态，并且请求信息应该被正确提取和存储。
        #[test]
        fn prop_flow_creation_correctness(
            id in arb_flow_id(),
            flow_type in arb_flow_type(),
            request in arb_llm_request(),
            metadata in arb_flow_metadata(),
        ) {
            // 创建 Flow
            let flow = LLMFlow::new(id.clone(), flow_type.clone(), request.clone(), metadata.clone());

            // 验证 ID 正确设置
            prop_assert_eq!(&flow.id, &id, "Flow ID 应该与输入 ID 相同");

            // 验证初始状态为 Pending
            prop_assert_eq!(flow.state, FlowState::Pending, "新创建的 Flow 状态应该是 Pending");

            // 验证 FlowType 正确设置
            prop_assert_eq!(flow.flow_type, flow_type, "FlowType 应该正确设置");

            // 验证请求信息正确存储
            prop_assert_eq!(flow.request.model, request.model, "模型名称应该正确存储");
            prop_assert_eq!(flow.request.method, request.method, "HTTP 方法应该正确存储");
            prop_assert_eq!(flow.request.path, request.path, "请求路径应该正确存储");
            prop_assert_eq!(flow.request.messages.len(), request.messages.len(), "消息列表长度应该一致");
            prop_assert_eq!(flow.request.system_prompt, request.system_prompt, "系统提示词应该正确存储");
            prop_assert_eq!(flow.request.parameters.stream, request.parameters.stream, "流式参数应该正确存储");

            // 验证元数据正确存储
            prop_assert_eq!(flow.metadata.provider, metadata.provider, "Provider 类型应该正确存储");
            prop_assert_eq!(flow.metadata.credential_id, metadata.credential_id, "凭证 ID 应该正确存储");

            // 验证响应和错误初始为空
            prop_assert!(flow.response.is_none(), "新创建的 Flow 响应应该为空");
            prop_assert!(flow.error.is_none(), "新创建的 Flow 错误应该为空");

            // 验证时间戳已设置
            prop_assert!(flow.timestamps.created <= Utc::now(), "创建时间应该已设置");
            prop_assert!(flow.timestamps.request_start <= Utc::now(), "请求开始时间应该已设置");

            // 验证标注初始为默认值
            prop_assert!(!flow.annotations.starred, "新创建的 Flow 不应该被收藏");
            prop_assert!(flow.annotations.tags.is_empty(), "新创建的 Flow 标签应该为空");
        }

        /// **Feature: llm-flow-monitor, Property 1b: Flow 序列化往返**
        /// **Validates: Requirements 1.1, 1.2**
        ///
        /// *对于任意* 有效的 LLMFlow，序列化后再反序列化应该得到等价的对象。
        #[test]
        fn prop_flow_serialization_roundtrip(
            id in arb_flow_id(),
            flow_type in arb_flow_type(),
            request in arb_llm_request(),
            metadata in arb_flow_metadata(),
        ) {
            let flow = LLMFlow::new(id, flow_type, request, metadata);

            // 序列化
            let json = serde_json::to_string(&flow).expect("序列化应该成功");

            // 反序列化
            let deserialized: LLMFlow = serde_json::from_str(&json).expect("反序列化应该成功");

            // 验证关键字段一致
            prop_assert_eq!(flow.id, deserialized.id, "ID 应该在往返后保持一致");
            prop_assert_eq!(flow.state, deserialized.state, "状态应该在往返后保持一致");
            prop_assert_eq!(flow.request.model, deserialized.request.model, "模型应该在往返后保持一致");
            prop_assert_eq!(flow.request.method, deserialized.request.method, "方法应该在往返后保持一致");
            prop_assert_eq!(flow.metadata.provider, deserialized.metadata.provider, "Provider 应该在往返后保持一致");
        }

        /// **Feature: llm-flow-monitor, Property 1c: 消息内容提取正确性**
        /// **Validates: Requirements 1.2**
        ///
        /// *对于任意* 消息内容，get_all_text() 应该返回所有文本内容。
        #[test]
        fn prop_message_content_text_extraction(
            content in arb_message_content(),
        ) {
            let text = content.get_all_text();

            match &content {
                MessageContent::Text(s) => {
                    prop_assert_eq!(&text, s, "纯文本内容应该完整返回");
                }
                MessageContent::MultiModal(parts) => {
                    // 验证所有文本部分都包含在结果中
                    for part in parts {
                        if let ContentPart::Text { text: part_text } = part {
                            prop_assert!(
                                text.contains(part_text),
                                "多模态内容中的文本部分应该包含在结果中"
                            );
                        }
                    }
                }
            }
        }

        /// **Feature: llm-flow-monitor, Property 1d: 错误类型可重试判断**
        /// **Validates: Requirements 1.8**
        ///
        /// *对于任意* 错误类型，is_retryable() 应该正确判断是否可重试。
        #[test]
        fn prop_error_type_retryable_consistency(
            status_code in 100u16..600u16,
        ) {
            let error_type = FlowErrorType::from_status_code(status_code);
            let is_retryable = error_type.is_retryable();

            // 验证可重试的错误类型
            match error_type {
                FlowErrorType::Network
                | FlowErrorType::Timeout
                | FlowErrorType::RateLimit
                | FlowErrorType::ServerError => {
                    prop_assert!(is_retryable, "{:?} 应该是可重试的", error_type);
                }
                FlowErrorType::Authentication
                | FlowErrorType::BadRequest
                | FlowErrorType::ContentFilter
                | FlowErrorType::ModelUnavailable
                | FlowErrorType::TokenLimitExceeded
                | FlowErrorType::Cancelled
                | FlowErrorType::Other => {
                    prop_assert!(!is_retryable, "{:?} 不应该是可重试的", error_type);
                }
            }
        }

        /// **Feature: llm-flow-monitor, Property 1e: Token 使用量计算正确性**
        /// **Validates: Requirements 1.9**
        ///
        /// *对于任意* Token 使用量，calculate_total() 应该正确计算总数。
        #[test]
        fn prop_token_usage_total_calculation(
            input_tokens in 0u32..100000u32,
            output_tokens in 0u32..100000u32,
        ) {
            let mut usage = TokenUsage {
                input_tokens,
                output_tokens,
                ..Default::default()
            };

            usage.calculate_total();

            prop_assert_eq!(
                usage.total_tokens,
                input_tokens + output_tokens,
                "总 Token 数应该等于输入 + 输出"
            );
        }

        /// **Feature: llm-flow-monitor, Property 1f: 时间戳计算正确性**
        /// **Validates: Requirements 1.9**
        ///
        /// *对于任意* 有效的时间戳序列，duration 和 ttfb 计算应该正确。
        #[test]
        fn prop_timestamps_calculation(
            ttfb_ms in 0i64..10000i64,
            response_duration_ms in 0i64..100000i64,
        ) {
            let start = Utc::now();
            let response_start = start + chrono::Duration::milliseconds(ttfb_ms);
            let end = response_start + chrono::Duration::milliseconds(response_duration_ms);

            let mut timestamps = FlowTimestamps {
                created: start,
                request_start: start,
                request_end: Some(start + chrono::Duration::milliseconds(10)),
                response_start: Some(response_start),
                response_end: Some(end),
                duration_ms: 0,
                ttfb_ms: None,
            };

            timestamps.calculate_duration();
            timestamps.calculate_ttfb();

            // 验证 TTFB 计算
            prop_assert_eq!(
                timestamps.ttfb_ms,
                Some(ttfb_ms as u64),
                "TTFB 应该正确计算"
            );

            // 验证总耗时计算
            let expected_duration = ttfb_ms + response_duration_ms;
            prop_assert_eq!(
                timestamps.duration_ms,
                expected_duration as u64,
                "总耗时应该正确计算"
            );
        }
    }
}
