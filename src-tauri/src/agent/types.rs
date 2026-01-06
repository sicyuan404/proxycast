//! Agent 类型定义
//!
//! 定义 Agent 模块使用的核心类型
//! 参考 goose 项目的 Conversation 设计，支持连续对话和工具调用

use serde::{Deserialize, Serialize};

/// Provider 类型枚举
///
/// 决定使用哪种 API 协议
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// Claude (Anthropic 协议)
    Claude,
    /// Claude OAuth (Anthropic 协议)
    ClaudeOauth,
    /// Kiro/CodeWhisperer (AWS Event Stream 协议)
    Kiro,
    /// Gemini (Gemini 协议)
    Gemini,
    /// OpenAI 及其兼容服务 (默认)
    #[default]
    OpenAI,
    /// 通义千问 (OpenAI 兼容)
    Qwen,
    /// Codex (OpenAI 兼容)
    Codex,
    /// Antigravity (OpenAI 兼容)
    Antigravity,
    /// iFlow (OpenAI 兼容)
    IFlow,
}

impl ProviderType {
    /// 从字符串解析 provider 类型
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "claude" | "anthropic" => Self::Claude,
            "claude_oauth" => Self::ClaudeOauth,
            "kiro" => Self::Kiro,
            "gemini" | "gemini_api_key" => Self::Gemini,
            "openai" => Self::OpenAI,
            "qwen" => Self::Qwen,
            "codex" => Self::Codex,
            "antigravity" => Self::Antigravity,
            "iflow" => Self::IFlow,
            _ => Self::OpenAI, // 默认使用 OpenAI 协议
        }
    }

    /// 获取 API 端点路径
    pub fn endpoint(&self) -> &'static str {
        match self {
            Self::Claude | Self::ClaudeOauth => "/v1/messages",
            Self::Kiro => "/v1/chat/completions", // Kiro 使用 OpenAI 兼容格式，但后端会转换
            Self::Gemini => "/v1/gemini/chat/completions",
            _ => "/v1/chat/completions",
        }
    }

    /// 是否使用 Anthropic 协议
    pub fn is_anthropic(&self) -> bool {
        matches!(self, Self::Claude | Self::ClaudeOauth)
    }

    /// 是否使用 OpenAI 兼容协议
    pub fn is_openai_compatible(&self) -> bool {
        matches!(
            self,
            Self::OpenAI | Self::Qwen | Self::Codex | Self::Antigravity | Self::IFlow | Self::Kiro
        )
    }
}

/// Agent 会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    /// 会话 ID
    pub id: String,
    /// 使用的模型
    pub model: String,
    /// 会话消息历史（支持连续对话）
    pub messages: Vec<AgentMessage>,
    /// 系统提示词
    pub system_prompt: Option<String>,
    /// 创建时间
    pub created_at: String,
    /// 最后活动时间
    pub updated_at: String,
}

/// Agent 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// 角色: user, assistant, system, tool
    pub role: String,
    /// 消息内容（文本或结构化内容）
    pub content: MessageContent,
    /// 时间戳
    pub timestamp: String,
    /// 工具调用（assistant 消息可能包含）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 工具调用 ID（tool 角色消息需要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 消息内容类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 纯文本
    Text(String),
    /// 多部分内容（文本 + 图片）
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// 获取文本内容
    pub fn as_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// 内容部分
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// 文本
    Text { text: String },
    /// 图片 URL
    ImageUrl { image_url: ImageUrl },
}

/// 图片 URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具调用 ID
    pub id: String,
    /// 工具类型
    #[serde(rename = "type")]
    pub call_type: String,
    /// 函数调用详情
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// 函数名
    pub name: String,
    /// 参数（JSON 字符串）
    pub arguments: String,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具类型
    #[serde(rename = "type")]
    pub tool_type: String,
    /// 函数定义
    pub function: FunctionDefinition,
}

/// 函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// 函数名
    pub name: String,
    /// 函数描述
    pub description: String,
    /// 参数 schema
    pub parameters: serde_json::Value,
}

/// Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// 模型名称
    pub model: String,
    /// 系统提示词
    pub system_prompt: Option<String>,
    /// 温度参数
    pub temperature: Option<f32>,
    /// 最大 token 数
    pub max_tokens: Option<u32>,
    /// 可用工具
    pub tools: Vec<ToolDefinition>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            system_prompt: Some(DEFAULT_SYSTEM_PROMPT.to_string()),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            tools: Vec::new(),
        }
    }
}

/// 默认系统提示词
///
/// 参考 Manus Agent 的模块化设计，使用结构化的提示词组织
/// 支持通过配置文件覆盖
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"你是 ProxyCast 内置的 AI 助手。

<identity>
- 你是一个友好、专业的 AI 助手
- 擅长编程、文件操作和系统任务
- 使用中文与用户交流
</identity>

<core_principles>
1. **自然交流优先**：问候、闲聊、问答类对话，直接用文字回复
2. **显式授权操作**：只有当用户明确提供路径或命令时，才能执行工具
3. **不主动探索**：不要未经请求就读取文件或执行命令
</core_principles>

<tool_use_rules>
## 何时使用工具

✅ **使用工具的情况**：
- 用户明确提供了文件路径（如 "读取 /path/to/file"）
- 用户明确要求执行命令（如 "运行 npm install"）
- 用户要求创建或修改文件

❌ **禁止使用工具的情况**：
- 用户说 "你好"、"嗨"、"hello" 等问候语
- 用户进行闲聊或一般性提问
- 用户没有提供具体路径时猜测路径
- 为了 "了解环境" 或 "打招呼" 而读取文件

## 可用工具

- **read_file**：读取用户指定的文件或目录内容
- **write_file**：创建或覆盖用户指定的文件
- **edit_file**：修改用户指定文件的特定内容
- **bash**：执行用户要求的 shell 命令
</tool_use_rules>

<response_examples>
## 正确示例

用户: "你好"
助手: "你好！有什么我可以帮助你的吗？"
（直接文字回复，不调用任何工具）

用户: "看看 /tmp/test.txt"
助手: 调用 read_file 工具读取 /tmp/test.txt

用户: "帮我列出当前目录"
助手: "请告诉我你想查看哪个目录？"
（询问具体路径，不要猜测）
</response_examples>

<output_format>
- 使用 Markdown 格式
- 回复简洁明了
- 使用中文
</output_format>"#;

/// 聊天请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChatRequest {
    /// 会话 ID（用于连续对话）
    pub session_id: Option<String>,
    /// 用户消息
    pub message: String,
    /// 模型名称（可选）
    pub model: Option<String>,
    /// 图片列表（可选）
    pub images: Option<Vec<ImageData>>,
    /// 是否流式响应
    pub stream: bool,
}

/// 图片数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    /// base64 编码的图片数据
    pub data: String,
    /// MIME 类型
    pub media_type: String,
}

/// 聊天响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChatResponse {
    /// 响应内容
    pub content: String,
    /// 使用的模型
    pub model: String,
    /// Token 使用量
    pub usage: Option<TokenUsage>,
    /// 是否成功
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
}

/// Token 使用量
///
/// 记录 API 调用的 token 消耗
/// Requirements: 1.3 - THE Streaming_Handler SHALL emit a done event with token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    /// 输入 token 数
    pub input_tokens: u32,
    /// 输出 token 数
    pub output_tokens: u32,
}

impl TokenUsage {
    /// 创建新的 TokenUsage
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    /// 计算总 token 数
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// 流式响应事件
///
/// 定义流式输出过程中的各种事件类型
/// Requirements: 1.1, 1.3, 1.4
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// 文本增量
    /// Requirements: 1.1 - THE Streaming_Handler SHALL emit text deltas to the frontend in real-time
    #[serde(rename = "text_delta")]
    TextDelta { text: String },

    /// 工具调用开始
    /// Requirements: 7.6 - WHILE the Tool_Loop is executing, THE Frontend SHALL display the current tool being executed
    #[serde(rename = "tool_start")]
    ToolStart {
        /// 工具名称
        tool_name: String,
        /// 工具调用 ID
        tool_id: String,
        /// 工具参数（JSON 字符串）
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<String>,
    },

    /// 工具调用结束
    /// Requirements: 7.6 - 工具执行完成后通知前端
    #[serde(rename = "tool_end")]
    ToolEnd {
        /// 工具调用 ID
        tool_id: String,
        /// 工具执行结果
        result: ToolExecutionResult,
    },

    /// 完成（单次 API 响应完成，工具循环可能继续）
    /// Requirements: 1.3 - THE Streaming_Handler SHALL emit a done event with token usage statistics
    #[serde(rename = "done")]
    Done { usage: Option<TokenUsage> },

    /// 最终完成（整个对话完成，包括所有工具调用循环）
    /// 前端收到此事件后才能取消监听
    #[serde(rename = "final_done")]
    FinalDone { usage: Option<TokenUsage> },

    /// 错误
    /// Requirements: 1.4 - IF a streaming error occurs, THEN THE Streaming_Handler SHALL emit an error event
    #[serde(rename = "error")]
    Error { message: String },
}

/// 工具执行结果（用于 StreamEvent）
///
/// 简化版的工具结果，用于前端显示
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 输出内容
    pub output: String,
    /// 错误信息（如果失败）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolExecutionResult {
    /// 创建成功结果
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
        }
    }

    /// 创建失败结果
    pub fn failure(error: impl Into<String>) -> Self {
        let error_msg = error.into();
        Self {
            success: false,
            output: String::new(),
            error: Some(error_msg),
        }
    }

    /// 创建带输出的失败结果
    pub fn failure_with_output(output: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            error: Some(error.into()),
        }
    }
}

/// 流式响应结果
///
/// 流式处理完成后的最终结果
/// Requirements: 1.1, 1.3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamResult {
    /// 完整的响应内容
    pub content: String,
    /// 工具调用列表（如果有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Token 使用量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

impl StreamResult {
    /// 创建新的流式结果
    pub fn new(content: String) -> Self {
        Self {
            content,
            tool_calls: None,
            usage: None,
        }
    }

    /// 设置工具调用
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }

    /// 设置 token 使用量
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// 是否有工具调用
    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls
            .as_ref()
            .map(|tc| !tc.is_empty())
            .unwrap_or(false)
    }
}
