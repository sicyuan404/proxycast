//! LLM Flow 导出服务
//!
//! 提供多种格式的 Flow 导出功能，包括 HAR、JSON、JSONL、Markdown 和 CSV。
//! 支持敏感数据脱敏和导出前过滤。

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::models::{
    FlowAnnotations, FlowError, LLMFlow, LLMRequest, LLMResponse, Message, MessageContent,
    ThinkingContent,
};
use super::FlowFilter;
#[cfg(test)]
use crate::ProviderType;

// ============================================================================
// 导出格式枚举
// ============================================================================

/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// HAR (HTTP Archive) 格式
    HAR,
    /// JSON 格式
    JSON,
    /// JSONL (JSON Lines) 格式
    JSONL,
    /// Markdown 格式
    Markdown,
    /// CSV 格式（仅元数据）
    CSV,
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::JSON
    }
}

// ============================================================================
// 导出选项
// ============================================================================

/// 导出选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// 导出格式
    pub format: ExportFormat,
    /// 过滤条件
    #[serde(default)]
    pub filter: Option<FlowFilter>,
    /// 是否包含原始请求/响应体
    #[serde(default = "default_true")]
    pub include_raw: bool,
    /// 是否包含流式 chunks
    #[serde(default)]
    pub include_stream_chunks: bool,
    /// 是否脱敏敏感数据
    #[serde(default)]
    pub redact_sensitive: bool,
    /// 脱敏规则
    #[serde(default)]
    pub redaction_rules: Vec<RedactionRule>,
    /// 是否压缩输出
    #[serde(default)]
    pub compress: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::JSON,
            filter: None,
            include_raw: true,
            include_stream_chunks: false,
            redact_sensitive: false,
            redaction_rules: Vec::new(),
            compress: false,
        }
    }
}

// ============================================================================
// 脱敏规则
// ============================================================================

/// 脱敏规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionRule {
    /// 规则名称
    pub name: String,
    /// 匹配模式（正则表达式）
    pub pattern: String,
    /// 替换文本
    pub replacement: String,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl RedactionRule {
    /// 创建新的脱敏规则
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            pattern: pattern.into(),
            replacement: replacement.into(),
            enabled: true,
        }
    }
}

/// 获取默认脱敏规则
pub fn default_redaction_rules() -> Vec<RedactionRule> {
    vec![
        // API 密钥模式
        RedactionRule::new(
            "api_key",
            r"(?i)(sk-[a-zA-Z0-9]{20,}|api[_-]?key[=:]\s*[a-zA-Z0-9_-]{20,})",
            "[REDACTED_API_KEY]",
        ),
        // Bearer Token
        RedactionRule::new(
            "bearer_token",
            r"(?i)bearer\s+[a-zA-Z0-9_.-]+",
            "Bearer [REDACTED_TOKEN]",
        ),
        // 邮箱地址
        RedactionRule::new(
            "email",
            r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
            "[REDACTED_EMAIL]",
        ),
        // 手机号（中国大陆）
        RedactionRule::new("phone_cn", r"1[3-9]\d{9}", "[REDACTED_PHONE]"),
        // 手机号（国际格式）
        RedactionRule::new(
            "phone_intl",
            r"\+\d{1,3}[-.\s]?\d{1,4}[-.\s]?\d{1,4}[-.\s]?\d{1,9}",
            "[REDACTED_PHONE]",
        ),
        // 信用卡号
        RedactionRule::new(
            "credit_card",
            r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b",
            "[REDACTED_CARD]",
        ),
        // 身份证号（中国大陆）
        RedactionRule::new("id_card_cn", r"\b\d{17}[\dXx]\b", "[REDACTED_ID]"),
        // AWS 密钥
        RedactionRule::new(
            "aws_key",
            r"(?i)(AKIA[0-9A-Z]{16}|aws[_-]?secret[_-]?access[_-]?key[=:]\s*[a-zA-Z0-9/+=]{40})",
            "[REDACTED_AWS_KEY]",
        ),
        // OpenAI API Key
        RedactionRule::new("openai_key", r"sk-[a-zA-Z0-9]{48}", "[REDACTED_OPENAI_KEY]"),
        // Anthropic API Key
        RedactionRule::new(
            "anthropic_key",
            r"sk-ant-[a-zA-Z0-9_-]{95}",
            "[REDACTED_ANTHROPIC_KEY]",
        ),
    ]
}

// ============================================================================
// 脱敏器
// ============================================================================

/// 敏感数据脱敏器
pub struct Redactor {
    rules: Vec<(String, Regex, String)>,
}

impl Redactor {
    /// 创建新的脱敏器
    pub fn new(rules: &[RedactionRule]) -> Self {
        let compiled_rules: Vec<_> = rules
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|r| {
                Regex::new(&r.pattern)
                    .ok()
                    .map(|regex| (r.name.clone(), regex, r.replacement.clone()))
            })
            .collect();

        Self {
            rules: compiled_rules,
        }
    }

    /// 使用默认规则创建脱敏器
    pub fn with_defaults() -> Self {
        Self::new(&default_redaction_rules())
    }

    /// 对文本应用脱敏
    pub fn redact(&self, text: &str) -> String {
        let mut result = text.to_string();
        for (_, regex, replacement) in &self.rules {
            result = regex.replace_all(&result, replacement.as_str()).to_string();
        }
        result
    }

    /// 对 JSON 值应用脱敏
    pub fn redact_json(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => serde_json::Value::String(self.redact(s)),
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.redact_json(v)).collect())
            }
            serde_json::Value::Object(obj) => {
                let mut new_obj = serde_json::Map::new();
                for (k, v) in obj {
                    new_obj.insert(k.clone(), self.redact_json(v));
                }
                serde_json::Value::Object(new_obj)
            }
            other => other.clone(),
        }
    }

    /// 对 Flow 应用脱敏
    pub fn redact_flow(&self, flow: &LLMFlow) -> LLMFlow {
        let mut redacted = flow.clone();

        // 脱敏请求
        redacted.request = self.redact_request(&flow.request);

        // 脱敏响应
        if let Some(ref response) = flow.response {
            redacted.response = Some(self.redact_response(response));
        }

        // 脱敏错误信息
        if let Some(ref error) = flow.error {
            redacted.error = Some(self.redact_error(error));
        }

        // 脱敏标注
        redacted.annotations = self.redact_annotations(&flow.annotations);

        redacted
    }

    fn redact_request(&self, request: &LLMRequest) -> LLMRequest {
        let mut redacted = request.clone();

        // 脱敏请求头
        redacted.headers = request
            .headers
            .iter()
            .map(|(k, v)| {
                let redacted_value = if k.to_lowercase().contains("authorization")
                    || k.to_lowercase().contains("api-key")
                    || k.to_lowercase().contains("x-api-key")
                {
                    "[REDACTED]".to_string()
                } else {
                    self.redact(v)
                };
                (k.clone(), redacted_value)
            })
            .collect();

        // 脱敏请求体
        redacted.body = self.redact_json(&request.body);

        // 脱敏消息
        redacted.messages = request
            .messages
            .iter()
            .map(|m| self.redact_message(m))
            .collect();

        // 脱敏系统提示词
        redacted.system_prompt = request.system_prompt.as_ref().map(|s| self.redact(s));

        redacted
    }

    fn redact_message(&self, message: &Message) -> Message {
        let mut redacted = message.clone();

        redacted.content = match &message.content {
            MessageContent::Text(s) => MessageContent::Text(self.redact(s)),
            MessageContent::MultiModal(parts) => MessageContent::MultiModal(
                parts
                    .iter()
                    .map(|p| match p {
                        super::models::ContentPart::Text { text } => {
                            super::models::ContentPart::Text {
                                text: self.redact(text),
                            }
                        }
                        other => other.clone(),
                    })
                    .collect(),
            ),
        };

        redacted
    }

    fn redact_response(&self, response: &LLMResponse) -> LLMResponse {
        let mut redacted = response.clone();

        // 脱敏响应头
        redacted.headers = response
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), self.redact(v)))
            .collect();

        // 脱敏响应体
        redacted.body = self.redact_json(&response.body);

        // 脱敏内容
        redacted.content = self.redact(&response.content);

        // 脱敏思维链
        if let Some(ref thinking) = response.thinking {
            redacted.thinking = Some(ThinkingContent {
                text: self.redact(&thinking.text),
                tokens: thinking.tokens,
                signature: thinking.signature.clone(),
            });
        }

        redacted
    }

    fn redact_error(&self, error: &FlowError) -> FlowError {
        let mut redacted = error.clone();
        redacted.message = self.redact(&error.message);
        redacted.raw_response = error.raw_response.as_ref().map(|s| self.redact(s));
        redacted
    }

    fn redact_annotations(&self, annotations: &FlowAnnotations) -> FlowAnnotations {
        let mut redacted = annotations.clone();
        redacted.comment = annotations.comment.as_ref().map(|s| self.redact(s));
        redacted
    }
}

// ============================================================================
// HAR 格式结构
// ============================================================================

/// HAR 存档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarArchive {
    pub log: HarLog,
}

/// HAR 日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarLog {
    pub version: String,
    pub creator: HarCreator,
    pub entries: Vec<HarEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 创建者信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarCreator {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarEntry {
    pub started_date_time: String,
    pub time: f64,
    pub request: HarRequest,
    pub response: HarResponse,
    pub cache: HarCache,
    pub timings: HarTimings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// LLM 特定扩展
    #[serde(rename = "_llm", skip_serializing_if = "Option::is_none")]
    pub llm_extension: Option<HarLlmExtension>,
}

/// HAR 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarRequest {
    pub method: String,
    pub url: String,
    pub http_version: String,
    pub cookies: Vec<HarCookie>,
    pub headers: Vec<HarHeader>,
    pub query_string: Vec<HarQueryParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_data: Option<HarPostData>,
    pub headers_size: i64,
    pub body_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarResponse {
    pub status: u16,
    pub status_text: String,
    pub http_version: String,
    pub cookies: Vec<HarCookie>,
    pub headers: Vec<HarHeader>,
    pub content: HarContent,
    pub redirect_url: String,
    pub headers_size: i64,
    pub body_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR Cookie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarCookie {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
}

/// HAR 请求头
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarHeader {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarQueryParam {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR POST 数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarPostData {
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<HarParam>>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarParam {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarContent {
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<i64>,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 缓存
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarCache {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_request: Option<HarCacheState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_request: Option<HarCacheState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 缓存状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarCacheState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    pub last_access: String,
    pub e_tag: String,
    pub hit_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// HAR 时间
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarTimings {
    pub blocked: f64,
    pub dns: f64,
    pub connect: f64,
    pub send: f64,
    pub wait: f64,
    pub receive: f64,
    pub ssl: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// LLM 特定扩展
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarLlmExtension {
    /// Flow ID
    pub flow_id: String,
    /// 提供商
    pub provider: String,
    /// 模型
    pub model: String,
    /// Flow 类型
    pub flow_type: String,
    /// Flow 状态
    pub state: String,
    /// Token 使用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<HarLlmTokens>,
    /// 是否流式
    pub streaming: bool,
    /// TTFB（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttfb_ms: Option<u64>,
    /// 停止原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// 是否有工具调用
    pub has_tool_calls: bool,
    /// 是否有思维链
    pub has_thinking: bool,
    /// 标注
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<FlowAnnotations>,
}

/// LLM Token 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarLlmTokens {
    pub input: u32,
    pub output: u32,
    pub total: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<u32>,
}

// ============================================================================
// Flow 导出器
// ============================================================================

/// Flow 导出器
pub struct FlowExporter {
    options: ExportOptions,
    redactor: Option<Redactor>,
}

impl FlowExporter {
    /// 创建新的导出器
    pub fn new(options: ExportOptions) -> Self {
        let redactor = if options.redact_sensitive {
            let rules = if options.redaction_rules.is_empty() {
                default_redaction_rules()
            } else {
                options.redaction_rules.clone()
            };
            Some(Redactor::new(&rules))
        } else {
            None
        };

        Self { options, redactor }
    }

    /// 使用默认选项创建导出器
    pub fn with_defaults() -> Self {
        Self::new(ExportOptions::default())
    }

    /// 预处理 Flow（应用脱敏等）
    fn preprocess_flow(&self, flow: &LLMFlow) -> LLMFlow {
        if let Some(ref redactor) = self.redactor {
            redactor.redact_flow(flow)
        } else {
            flow.clone()
        }
    }

    /// 预处理多个 Flow
    fn preprocess_flows(&self, flows: &[LLMFlow]) -> Vec<LLMFlow> {
        flows.iter().map(|f| self.preprocess_flow(f)).collect()
    }

    /// 导出为 HAR 格式
    pub fn export_har(&self, flows: &[LLMFlow]) -> HarArchive {
        let processed = self.preprocess_flows(flows);
        let entries: Vec<HarEntry> = processed
            .iter()
            .map(|f| self.flow_to_har_entry(f))
            .collect();

        HarArchive {
            log: HarLog {
                version: "1.2".to_string(),
                creator: HarCreator {
                    name: "ProxyCast LLM Flow Monitor".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    comment: Some("LLM API Flow Export".to_string()),
                },
                entries,
                comment: Some(format!("Exported {} flows", flows.len())),
            },
        }
    }

    /// 将 Flow 转换为 HAR Entry
    fn flow_to_har_entry(&self, flow: &LLMFlow) -> HarEntry {
        let request = &flow.request;
        let response = flow.response.as_ref();

        // 构建请求 URL
        let base_url = flow
            .metadata
            .routing_info
            .target_url
            .clone()
            .unwrap_or_else(|| "http://localhost".to_string());
        let url = format!("{}{}", base_url, request.path);

        // 构建请求头
        let headers: Vec<HarHeader> = request
            .headers
            .iter()
            .map(|(k, v)| HarHeader {
                name: k.clone(),
                value: v.clone(),
                comment: None,
            })
            .collect();

        // 构建 POST 数据
        let post_data = if self.options.include_raw {
            Some(HarPostData {
                mime_type: "application/json".to_string(),
                params: None,
                text: serde_json::to_string(&request.body).unwrap_or_default(),
                comment: None,
            })
        } else {
            None
        };

        // 构建响应
        let (har_response, _response_body_size) = if let Some(resp) = response {
            let resp_headers: Vec<HarHeader> = resp
                .headers
                .iter()
                .map(|(k, v)| HarHeader {
                    name: k.clone(),
                    value: v.clone(),
                    comment: None,
                })
                .collect();

            let content_text = if self.options.include_raw {
                Some(serde_json::to_string(&resp.body).unwrap_or_default())
            } else {
                None
            };

            (
                HarResponse {
                    status: resp.status_code,
                    status_text: resp.status_text.clone(),
                    http_version: "HTTP/1.1".to_string(),
                    cookies: Vec::new(),
                    headers: resp_headers,
                    content: HarContent {
                        size: resp.size_bytes as i64,
                        compression: None,
                        mime_type: "application/json".to_string(),
                        text: content_text,
                        encoding: None,
                        comment: None,
                    },
                    redirect_url: String::new(),
                    headers_size: -1,
                    body_size: resp.size_bytes as i64,
                    comment: None,
                },
                resp.size_bytes as i64,
            )
        } else {
            (
                HarResponse {
                    status: 0,
                    status_text: "No Response".to_string(),
                    http_version: "HTTP/1.1".to_string(),
                    cookies: Vec::new(),
                    headers: Vec::new(),
                    content: HarContent {
                        size: 0,
                        compression: None,
                        mime_type: "application/json".to_string(),
                        text: None,
                        encoding: None,
                        comment: None,
                    },
                    redirect_url: String::new(),
                    headers_size: -1,
                    body_size: 0,
                    comment: None,
                },
                0,
            )
        };

        // 构建 LLM 扩展
        let llm_extension = Some(HarLlmExtension {
            flow_id: flow.id.clone(),
            provider: format!("{:?}", flow.metadata.provider),
            model: request.model.clone(),
            flow_type: format!("{:?}", flow.flow_type),
            state: format!("{:?}", flow.state),
            tokens: response.map(|r| HarLlmTokens {
                input: r.usage.input_tokens,
                output: r.usage.output_tokens,
                total: r.usage.total_tokens,
                cache_read: r.usage.cache_read_tokens,
                cache_write: r.usage.cache_write_tokens,
                thinking: r.usage.thinking_tokens,
            }),
            streaming: request.parameters.stream,
            ttfb_ms: flow.timestamps.ttfb_ms,
            stop_reason: response.and_then(|r| r.stop_reason.as_ref().map(|s| format!("{:?}", s))),
            has_tool_calls: response.map(|r| !r.tool_calls.is_empty()).unwrap_or(false),
            has_thinking: response.map(|r| r.thinking.is_some()).unwrap_or(false),
            annotations: if flow.annotations.starred
                || flow.annotations.comment.is_some()
                || !flow.annotations.tags.is_empty()
            {
                Some(flow.annotations.clone())
            } else {
                None
            },
        });

        // 计算时间
        let ttfb = flow.timestamps.ttfb_ms.unwrap_or(0) as f64;
        let total_time = flow.timestamps.duration_ms as f64;

        HarEntry {
            started_date_time: flow.timestamps.request_start.to_rfc3339(),
            time: total_time,
            request: HarRequest {
                method: request.method.clone(),
                url,
                http_version: "HTTP/1.1".to_string(),
                cookies: Vec::new(),
                headers,
                query_string: Vec::new(),
                post_data,
                headers_size: -1,
                body_size: request.size_bytes as i64,
                comment: None,
            },
            response: har_response,
            cache: HarCache {
                before_request: None,
                after_request: None,
                comment: None,
            },
            timings: HarTimings {
                blocked: -1.0,
                dns: -1.0,
                connect: -1.0,
                send: 0.0,
                wait: ttfb,
                receive: total_time - ttfb,
                ssl: -1.0,
                comment: None,
            },
            server_ip_address: None,
            connection: None,
            comment: flow.annotations.comment.clone(),
            llm_extension,
        }
    }

    /// 导出为 JSON 格式
    pub fn export_json(&self, flows: &[LLMFlow]) -> serde_json::Value {
        let processed = self.preprocess_flows(flows);
        serde_json::to_value(&processed).unwrap_or(serde_json::Value::Array(Vec::new()))
    }

    /// 导出为 JSONL 格式
    pub fn export_jsonl(&self, flows: &[LLMFlow]) -> String {
        let processed = self.preprocess_flows(flows);
        processed
            .iter()
            .filter_map(|f| serde_json::to_string(f).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 导出单个 Flow 为 Markdown 格式
    pub fn export_markdown(&self, flow: &LLMFlow) -> String {
        let processed = self.preprocess_flow(flow);
        self.flow_to_markdown(&processed)
    }

    /// 导出多个 Flow 为 Markdown 格式
    pub fn export_markdown_multiple(&self, flows: &[LLMFlow]) -> String {
        let processed = self.preprocess_flows(flows);
        processed
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let md = self.flow_to_markdown(f);
                if i > 0 {
                    format!("\n---\n\n{}", md)
                } else {
                    md
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// 将 Flow 转换为 Markdown
    fn flow_to_markdown(&self, flow: &LLMFlow) -> String {
        let mut md = String::new();

        // 标题
        md.push_str(&format!("# LLM Flow: {}\n\n", flow.id));

        // 元信息
        md.push_str("## 基本信息\n\n");
        md.push_str(&format!("- **Flow ID**: `{}`\n", flow.id));
        md.push_str(&format!("- **类型**: {:?}\n", flow.flow_type));
        md.push_str(&format!("- **状态**: {:?}\n", flow.state));
        md.push_str(&format!("- **提供商**: {:?}\n", flow.metadata.provider));
        md.push_str(&format!("- **模型**: {}\n", flow.request.model));
        md.push_str(&format!(
            "- **创建时间**: {}\n",
            flow.timestamps.created.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        md.push_str(&format!("- **耗时**: {} ms\n", flow.timestamps.duration_ms));
        if let Some(ttfb) = flow.timestamps.ttfb_ms {
            md.push_str(&format!("- **TTFB**: {} ms\n", ttfb));
        }
        md.push_str(&format!("- **流式**: {}\n", flow.request.parameters.stream));
        md.push('\n');

        // Token 使用
        if let Some(ref response) = flow.response {
            md.push_str("## Token 使用\n\n");
            md.push_str(&format!(
                "- **输入 Token**: {}\n",
                response.usage.input_tokens
            ));
            md.push_str(&format!(
                "- **输出 Token**: {}\n",
                response.usage.output_tokens
            ));
            md.push_str(&format!(
                "- **总 Token**: {}\n",
                response.usage.total_tokens
            ));
            if let Some(cache_read) = response.usage.cache_read_tokens {
                md.push_str(&format!("- **缓存读取**: {}\n", cache_read));
            }
            if let Some(thinking) = response.usage.thinking_tokens {
                md.push_str(&format!("- **思维链 Token**: {}\n", thinking));
            }
            md.push('\n');
        }

        // 请求
        md.push_str("## 请求\n\n");
        md.push_str(&format!(
            "**{} {}**\n\n",
            flow.request.method, flow.request.path
        ));

        // 系统提示词
        if let Some(ref system) = flow.request.system_prompt {
            md.push_str("### 系统提示词\n\n");
            md.push_str("```\n");
            md.push_str(system);
            md.push_str("\n```\n\n");
        }

        // 消息
        if !flow.request.messages.is_empty() {
            md.push_str("### 消息\n\n");
            for (i, msg) in flow.request.messages.iter().enumerate() {
                md.push_str(&format!(
                    "#### {} {}\n\n",
                    i + 1,
                    format!("{:?}", msg.role).to_uppercase()
                ));
                let content = msg.content.get_all_text();
                if !content.is_empty() {
                    md.push_str("```\n");
                    md.push_str(&content);
                    md.push_str("\n```\n\n");
                }
            }
        }

        // 响应
        if let Some(ref response) = flow.response {
            md.push_str("## 响应\n\n");
            md.push_str(&format!(
                "**状态**: {} {}\n\n",
                response.status_code, response.status_text
            ));

            // 思维链
            if let Some(ref thinking) = response.thinking {
                md.push_str("### 思维链\n\n");
                md.push_str("<details>\n<summary>展开查看思维链内容</summary>\n\n");
                md.push_str("```\n");
                md.push_str(&thinking.text);
                md.push_str("\n```\n\n");
                md.push_str("</details>\n\n");
            }

            // 内容
            if !response.content.is_empty() {
                md.push_str("### 内容\n\n");
                md.push_str("```\n");
                md.push_str(&response.content);
                md.push_str("\n```\n\n");
            }

            // 工具调用
            if !response.tool_calls.is_empty() {
                md.push_str("### 工具调用\n\n");
                for (i, tc) in response.tool_calls.iter().enumerate() {
                    md.push_str(&format!("#### 工具调用 {}\n\n", i + 1));
                    md.push_str(&format!("- **ID**: `{}`\n", tc.id));
                    md.push_str(&format!("- **函数**: `{}`\n", tc.function.name));
                    md.push_str("- **参数**:\n");
                    md.push_str("```json\n");
                    // 尝试格式化 JSON
                    if let Ok(parsed) =
                        serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                    {
                        md.push_str(
                            &serde_json::to_string_pretty(&parsed)
                                .unwrap_or(tc.function.arguments.clone()),
                        );
                    } else {
                        md.push_str(&tc.function.arguments);
                    }
                    md.push_str("\n```\n\n");
                }
            }

            // 停止原因
            if let Some(ref stop_reason) = response.stop_reason {
                md.push_str(&format!("**停止原因**: {:?}\n\n", stop_reason));
            }
        }

        // 错误
        if let Some(ref error) = flow.error {
            md.push_str("## 错误\n\n");
            md.push_str(&format!("- **类型**: {:?}\n", error.error_type));
            md.push_str(&format!("- **消息**: {}\n", error.message));
            if let Some(code) = error.status_code {
                md.push_str(&format!("- **状态码**: {}\n", code));
            }
            md.push_str(&format!("- **可重试**: {}\n", error.retryable));
            md.push('\n');
        }

        // 标注
        if flow.annotations.starred
            || flow.annotations.comment.is_some()
            || !flow.annotations.tags.is_empty()
        {
            md.push_str("## 标注\n\n");
            if flow.annotations.starred {
                md.push_str("- ⭐ **已收藏**\n");
            }
            if let Some(ref marker) = flow.annotations.marker {
                md.push_str(&format!("- **标记**: {}\n", marker));
            }
            if !flow.annotations.tags.is_empty() {
                md.push_str(&format!(
                    "- **标签**: {}\n",
                    flow.annotations.tags.join(", ")
                ));
            }
            if let Some(ref comment) = flow.annotations.comment {
                md.push_str(&format!("- **评论**: {}\n", comment));
            }
            md.push('\n');
        }

        md
    }

    /// 导出为 CSV 格式（仅元数据）
    pub fn export_csv(&self, flows: &[LLMFlow]) -> String {
        let processed = self.preprocess_flows(flows);
        let mut csv = String::new();

        // CSV 头
        csv.push_str("id,created_at,provider,model,flow_type,state,method,path,");
        csv.push_str("status_code,duration_ms,ttfb_ms,input_tokens,output_tokens,total_tokens,");
        csv.push_str("streaming,has_error,has_tool_calls,has_thinking,starred,tags\n");

        // 数据行
        for flow in &processed {
            let response = flow.response.as_ref();
            let row = format!(
                "{},{},{:?},{},{:?},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                escape_csv(&flow.id),
                flow.timestamps.created.to_rfc3339(),
                flow.metadata.provider,
                escape_csv(&flow.request.model),
                flow.flow_type,
                flow.state,
                escape_csv(&flow.request.method),
                escape_csv(&flow.request.path),
                response.map(|r| r.status_code).unwrap_or(0),
                flow.timestamps.duration_ms,
                flow.timestamps.ttfb_ms.unwrap_or(0),
                response.map(|r| r.usage.input_tokens).unwrap_or(0),
                response.map(|r| r.usage.output_tokens).unwrap_or(0),
                response.map(|r| r.usage.total_tokens).unwrap_or(0),
                flow.request.parameters.stream,
                flow.error.is_some(),
                response.map(|r| !r.tool_calls.is_empty()).unwrap_or(false),
                response.map(|r| r.thinking.is_some()).unwrap_or(false),
                flow.annotations.starred,
                escape_csv(&flow.annotations.tags.join(";"))
            );
            csv.push_str(&row);
        }

        csv
    }

    /// 根据选项导出
    pub fn export(&self, flows: &[LLMFlow]) -> ExportResult {
        match self.options.format {
            ExportFormat::HAR => {
                let har = self.export_har(flows);
                ExportResult::Har(har)
            }
            ExportFormat::JSON => {
                let json = self.export_json(flows);
                ExportResult::Json(json)
            }
            ExportFormat::JSONL => {
                let jsonl = self.export_jsonl(flows);
                ExportResult::Text(jsonl)
            }
            ExportFormat::Markdown => {
                let md = self.export_markdown_multiple(flows);
                ExportResult::Text(md)
            }
            ExportFormat::CSV => {
                let csv = self.export_csv(flows);
                ExportResult::Text(csv)
            }
        }
    }
}

/// CSV 字段转义
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// 导出结果
#[derive(Debug, Clone)]
pub enum ExportResult {
    /// HAR 格式
    Har(HarArchive),
    /// JSON 格式
    Json(serde_json::Value),
    /// 文本格式（JSONL、Markdown、CSV）
    Text(String),
}

impl ExportResult {
    /// 转换为字符串
    pub fn to_string_pretty(&self) -> String {
        match self {
            ExportResult::Har(har) => serde_json::to_string_pretty(har).unwrap_or_default(),
            ExportResult::Json(json) => serde_json::to_string_pretty(json).unwrap_or_default(),
            ExportResult::Text(text) => text.clone(),
        }
    }

    /// 转换为紧凑字符串
    pub fn to_string_compact(&self) -> String {
        match self {
            ExportResult::Har(har) => serde_json::to_string(har).unwrap_or_default(),
            ExportResult::Json(json) => serde_json::to_string(json).unwrap_or_default(),
            ExportResult::Text(text) => text.clone(),
        }
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_monitor::models::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_flow() -> LLMFlow {
        let request = LLMRequest {
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            headers: {
                let mut h = HashMap::new();
                h.insert(
                    "Authorization".to_string(),
                    "Bearer sk-test123456789".to_string(),
                );
                h.insert("Content-Type".to_string(), "application/json".to_string());
                h
            },
            body: serde_json::json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Text("Hello, my email is test@example.com".to_string()),
                tool_calls: None,
                tool_result: None,
                name: None,
            }],
            system_prompt: Some("You are a helpful assistant.".to_string()),
            tools: None,
            model: "gpt-4".to_string(),
            original_model: None,
            parameters: RequestParameters {
                temperature: Some(0.7),
                stream: true,
                ..Default::default()
            },
            size_bytes: 256,
            timestamp: Utc::now(),
        };

        let response = LLMResponse {
            status_code: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: serde_json::json!({"choices": [{"message": {"content": "Hi there!"}}]}),
            content: "Hi there!".to_string(),
            thinking: None,
            tool_calls: Vec::new(),
            usage: TokenUsage {
                input_tokens: 10,
                output_tokens: 5,
                total_tokens: 15,
                ..Default::default()
            },
            stop_reason: Some(StopReason::Stop),
            size_bytes: 128,
            timestamp_start: Utc::now(),
            timestamp_end: Utc::now(),
            stream_info: None,
        };

        let metadata = FlowMetadata {
            provider: ProviderType::OpenAI,
            credential_id: Some("cred-123".to_string()),
            credential_name: Some("Test Credential".to_string()),
            ..Default::default()
        };

        let mut flow = LLMFlow::new(
            "test-flow-001".to_string(),
            FlowType::ChatCompletions,
            request,
            metadata,
        );
        flow.response = Some(response);
        flow.state = FlowState::Completed;
        flow.timestamps.duration_ms = 500;
        flow.timestamps.ttfb_ms = Some(100);

        flow
    }

    #[test]
    fn test_export_format_default() {
        assert_eq!(ExportFormat::default(), ExportFormat::JSON);
    }

    #[test]
    fn test_export_options_default() {
        let options = ExportOptions::default();
        assert_eq!(options.format, ExportFormat::JSON);
        assert!(options.include_raw);
        assert!(!options.redact_sensitive);
    }

    #[test]
    fn test_redaction_rule_creation() {
        let rule = RedactionRule::new("test", r"\d+", "[NUMBER]");
        assert_eq!(rule.name, "test");
        assert_eq!(rule.pattern, r"\d+");
        assert_eq!(rule.replacement, "[NUMBER]");
        assert!(rule.enabled);
    }

    #[test]
    fn test_default_redaction_rules() {
        let rules = default_redaction_rules();
        assert!(!rules.is_empty());

        // 验证包含常见规则
        let rule_names: Vec<_> = rules.iter().map(|r| r.name.as_str()).collect();
        assert!(rule_names.contains(&"api_key"));
        assert!(rule_names.contains(&"email"));
        assert!(rule_names.contains(&"phone_cn"));
    }

    #[test]
    fn test_redactor_email() {
        let redactor = Redactor::with_defaults();
        let text = "Contact me at john@example.com for more info.";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("john@example.com"));
        assert!(redacted.contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn test_redactor_phone() {
        let redactor = Redactor::with_defaults();
        let text = "My phone is 13812345678";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("13812345678"));
        assert!(redacted.contains("[REDACTED_PHONE]"));
    }

    #[test]
    fn test_redactor_api_key() {
        let redactor = Redactor::with_defaults();
        let text = "Use this key: sk-abcdefghijklmnopqrstuvwxyz123456";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("sk-abcdefghijklmnopqrstuvwxyz123456"));
    }

    #[test]
    fn test_redactor_json() {
        let redactor = Redactor::with_defaults();
        let json = serde_json::json!({
            "email": "test@example.com",
            "nested": {
                "phone": "13812345678"
            }
        });
        let redacted = redactor.redact_json(&json);
        let redacted_str = serde_json::to_string(&redacted).unwrap();
        assert!(!redacted_str.contains("test@example.com"));
        assert!(!redacted_str.contains("13812345678"));
    }

    #[test]
    fn test_export_json() {
        let flow = create_test_flow();
        let exporter = FlowExporter::with_defaults();
        let json = exporter.export_json(&[flow]);

        assert!(json.is_array());
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn test_export_jsonl() {
        let flow = create_test_flow();
        let exporter = FlowExporter::with_defaults();
        let jsonl = exporter.export_jsonl(&[flow.clone(), flow]);

        let lines: Vec<_> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);

        // 验证每行都是有效的 JSON
        for line in lines {
            assert!(serde_json::from_str::<LLMFlow>(line).is_ok());
        }
    }

    #[test]
    fn test_export_har() {
        let flow = create_test_flow();
        let exporter = FlowExporter::with_defaults();
        let har = exporter.export_har(&[flow]);

        assert_eq!(har.log.version, "1.2");
        assert_eq!(har.log.entries.len(), 1);

        let entry = &har.log.entries[0];
        assert_eq!(entry.request.method, "POST");
        assert!(entry.llm_extension.is_some());

        let llm_ext = entry.llm_extension.as_ref().unwrap();
        assert_eq!(llm_ext.model, "gpt-4");
        assert!(llm_ext.streaming);
    }

    #[test]
    fn test_export_markdown() {
        let flow = create_test_flow();
        let exporter = FlowExporter::with_defaults();
        let md = exporter.export_markdown(&flow);

        assert!(md.contains("# LLM Flow:"));
        assert!(md.contains("test-flow-001"));
        assert!(md.contains("gpt-4"));
        assert!(md.contains("## 请求"));
        assert!(md.contains("## 响应"));
    }

    #[test]
    fn test_export_csv() {
        let flow = create_test_flow();
        let exporter = FlowExporter::with_defaults();
        let csv = exporter.export_csv(&[flow]);

        let lines: Vec<_> = csv.lines().collect();
        assert_eq!(lines.len(), 2); // header + 1 data row

        // 验证头部
        assert!(lines[0].contains("id,created_at,provider"));

        // 验证数据行
        assert!(lines[1].contains("test-flow-001"));
    }

    #[test]
    fn test_export_with_redaction() {
        let flow = create_test_flow();
        let options = ExportOptions {
            format: ExportFormat::JSON,
            redact_sensitive: true,
            ..Default::default()
        };
        let exporter = FlowExporter::new(options);
        let json = exporter.export_json(&[flow]);

        let json_str = serde_json::to_string(&json).unwrap();
        // 验证敏感数据已被脱敏
        assert!(!json_str.contains("test@example.com"));
    }

    #[test]
    fn test_export_result_to_string() {
        let flow = create_test_flow();
        let exporter = FlowExporter::with_defaults();
        let result = exporter.export(&[flow]);

        let pretty = result.to_string_pretty();
        let compact = result.to_string_compact();

        assert!(!pretty.is_empty());
        assert!(!compact.is_empty());
        // Pretty 格式应该比 compact 更长（有缩进）
        assert!(pretty.len() >= compact.len());
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("simple"), "simple");
        assert_eq!(escape_csv("with,comma"), "\"with,comma\"");
        assert_eq!(escape_csv("with\"quote"), "\"with\"\"quote\"");
        assert_eq!(escape_csv("with\nnewline"), "\"with\nnewline\"");
    }

    #[test]
    fn test_har_llm_extension() {
        let flow = create_test_flow();
        let exporter = FlowExporter::with_defaults();
        let har = exporter.export_har(&[flow]);

        let entry = &har.log.entries[0];
        let llm_ext = entry.llm_extension.as_ref().unwrap();

        assert_eq!(llm_ext.flow_id, "test-flow-001");
        assert!(llm_ext.tokens.is_some());

        let tokens = llm_ext.tokens.as_ref().unwrap();
        assert_eq!(tokens.input, 10);
        assert_eq!(tokens.output, 5);
        assert_eq!(tokens.total, 15);
    }
}

// ============================================================================
// 属性测试模块
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::flow_monitor::models::*;
    use chrono::Utc;
    use proptest::prelude::*;
    use std::collections::HashMap;

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
        ]
    }

    /// 生成随机的 FlowType
    fn arb_flow_type() -> impl Strategy<Value = FlowType> {
        prop_oneof![
            Just(FlowType::ChatCompletions),
            Just(FlowType::AnthropicMessages),
            Just(FlowType::GeminiGenerateContent),
            Just(FlowType::Embeddings),
        ]
    }

    /// 生成随机的 MessageRole
    fn arb_message_role() -> impl Strategy<Value = MessageRole> {
        prop_oneof![
            Just(MessageRole::System),
            Just(MessageRole::User),
            Just(MessageRole::Assistant),
        ]
    }

    /// 生成随机的文本内容（不包含敏感数据）
    fn arb_safe_text() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ,.!?]{0,100}"
    }

    /// 生成随机的 MessageContent
    fn arb_message_content() -> impl Strategy<Value = MessageContent> {
        arb_safe_text().prop_map(MessageContent::Text)
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
            "[a-z0-9-]{3,20}",                          // model
            prop::collection::vec(arb_message(), 0..3), // messages
            arb_request_parameters(),                   // parameters
            prop::option::of(arb_safe_text()),          // system_prompt
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

    /// 生成随机的 TokenUsage
    fn arb_token_usage() -> impl Strategy<Value = TokenUsage> {
        (0u32..10000u32, 0u32..10000u32).prop_map(|(input, output)| TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
            cache_read_tokens: None,
            cache_write_tokens: None,
            thinking_tokens: None,
        })
    }

    /// 生成随机的 LLMResponse
    fn arb_llm_response() -> impl Strategy<Value = LLMResponse> {
        (arb_safe_text(), arb_token_usage()).prop_map(|(content, usage)| LLMResponse {
            status_code: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            content,
            thinking: None,
            tool_calls: Vec::new(),
            usage,
            stop_reason: Some(StopReason::Stop),
            size_bytes: 0,
            timestamp_start: Utc::now(),
            timestamp_end: Utc::now(),
            stream_info: None,
        })
    }

    /// 生成随机的 FlowMetadata
    fn arb_flow_metadata() -> impl Strategy<Value = FlowMetadata> {
        arb_provider_type().prop_map(|provider| FlowMetadata {
            provider,
            provider_id: None,
            credential_id: None,
            credential_name: None,
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

    /// 生成随机的 LLMFlow
    fn arb_llm_flow() -> impl Strategy<Value = LLMFlow> {
        (
            arb_flow_id(),
            arb_flow_type(),
            arb_llm_request(),
            arb_flow_metadata(),
            prop::option::of(arb_llm_response()),
        )
            .prop_map(|(id, flow_type, request, metadata, response)| {
                let mut flow = LLMFlow::new(id, flow_type, request, metadata);
                flow.response = response;
                if flow.response.is_some() {
                    flow.state = FlowState::Completed;
                }
                flow.timestamps.duration_ms = 100;
                flow
            })
    }

    // ========================================================================
    // 属性测试
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: llm-flow-monitor, Property 8: 导出 Round-Trip**
        /// **Validates: Requirements 5.2**
        ///
        /// *对于任意* 有效的 LLM_Flow，导出为 JSON 格式后再解析，
        /// 解析的 Flow 应该与原始 Flow 等价。
        #[test]
        fn prop_export_json_roundtrip(flow in arb_llm_flow()) {
            let exporter = FlowExporter::with_defaults();

            // 导出为 JSON
            let json = exporter.export_json(&[flow.clone()]);

            // 验证是数组
            prop_assert!(json.is_array(), "导出结果应该是 JSON 数组");

            let arr = json.as_array().unwrap();
            prop_assert_eq!(arr.len(), 1, "数组应该包含一个元素");

            // 反序列化
            let deserialized: LLMFlow = serde_json::from_value(arr[0].clone())
                .expect("应该能够反序列化");

            // 验证关键字段一致
            prop_assert_eq!(&flow.id, &deserialized.id, "ID 应该在往返后保持一致");
            prop_assert_eq!(flow.state, deserialized.state, "状态应该在往返后保持一致");
            prop_assert_eq!(flow.flow_type, deserialized.flow_type, "FlowType 应该在往返后保持一致");
            prop_assert_eq!(&flow.request.model, &deserialized.request.model, "模型应该在往返后保持一致");
            prop_assert_eq!(&flow.request.method, &deserialized.request.method, "方法应该在往返后保持一致");
            prop_assert_eq!(flow.metadata.provider, deserialized.metadata.provider, "Provider 应该在往返后保持一致");

            // 验证响应
            prop_assert_eq!(flow.response.is_some(), deserialized.response.is_some(), "响应存在性应该一致");
            if let (Some(ref orig), Some(ref deser)) = (&flow.response, &deserialized.response) {
                prop_assert_eq!(orig.status_code, deser.status_code, "状态码应该一致");
                prop_assert_eq!(&orig.content, &deser.content, "内容应该一致");
                prop_assert_eq!(orig.usage.input_tokens, deser.usage.input_tokens, "输入 Token 应该一致");
                prop_assert_eq!(orig.usage.output_tokens, deser.usage.output_tokens, "输出 Token 应该一致");
            }
        }

        /// **Feature: llm-flow-monitor, Property 8b: JSONL 导出 Round-Trip**
        /// **Validates: Requirements 5.3**
        ///
        /// *对于任意* 有效的 LLM_Flow 列表，导出为 JSONL 格式后再解析，
        /// 每行都应该能够正确反序列化为 LLMFlow。
        #[test]
        fn prop_export_jsonl_roundtrip(
            flows in prop::collection::vec(arb_llm_flow(), 1..5)
        ) {
            let exporter = FlowExporter::with_defaults();

            // 导出为 JSONL
            let jsonl = exporter.export_jsonl(&flows);

            // 验证行数
            let lines: Vec<_> = jsonl.lines().collect();
            prop_assert_eq!(lines.len(), flows.len(), "JSONL 行数应该等于 Flow 数量");

            // 验证每行都能反序列化
            for (i, line) in lines.iter().enumerate() {
                let deserialized: LLMFlow = serde_json::from_str(line)
                    .expect(&format!("第 {} 行应该能够反序列化", i));

                prop_assert_eq!(
                    &flows[i].id, &deserialized.id,
                    "第 {} 个 Flow 的 ID 应该一致", i
                );
            }
        }

        /// **Feature: llm-flow-monitor, Property 8c: HAR 导出结构正确性**
        /// **Validates: Requirements 5.1, 5.7**
        ///
        /// *对于任意* 有效的 LLM_Flow 列表，导出为 HAR 格式后，
        /// HAR 结构应该符合规范，且包含 LLM 特定扩展。
        #[test]
        fn prop_export_har_structure(
            flows in prop::collection::vec(arb_llm_flow(), 1..5)
        ) {
            let exporter = FlowExporter::with_defaults();

            // 导出为 HAR
            let har = exporter.export_har(&flows);

            // 验证 HAR 结构
            prop_assert_eq!(har.log.version, "1.2", "HAR 版本应该是 1.2");
            prop_assert_eq!(har.log.entries.len(), flows.len(), "HAR 条目数应该等于 Flow 数量");

            // 验证每个条目
            for (i, entry) in har.log.entries.iter().enumerate() {
                // 验证请求
                prop_assert_eq!(&entry.request.method, &flows[i].request.method, "请求方法应该一致");

                // 验证 LLM 扩展存在
                prop_assert!(entry.llm_extension.is_some(), "应该包含 LLM 扩展");

                let llm_ext = entry.llm_extension.as_ref().unwrap();
                prop_assert_eq!(&llm_ext.flow_id, &flows[i].id, "Flow ID 应该一致");
                prop_assert_eq!(&llm_ext.model, &flows[i].request.model, "模型应该一致");
                prop_assert_eq!(llm_ext.streaming, flows[i].request.parameters.stream, "流式标志应该一致");
            }
        }

        /// **Feature: llm-flow-monitor, Property 8d: CSV 导出包含所有 Flow**
        /// **Validates: Requirements 5.5**
        ///
        /// *对于任意* 有效的 LLM_Flow 列表，导出为 CSV 格式后，
        /// CSV 应该包含头部和所有 Flow 的数据行。
        #[test]
        fn prop_export_csv_completeness(
            flows in prop::collection::vec(arb_llm_flow(), 1..5)
        ) {
            let exporter = FlowExporter::with_defaults();

            // 导出为 CSV
            let csv = exporter.export_csv(&flows);

            // 验证行数（头部 + 数据行）
            let lines: Vec<_> = csv.lines().collect();
            prop_assert_eq!(lines.len(), flows.len() + 1, "CSV 行数应该等于 Flow 数量 + 1（头部）");

            // 验证头部
            prop_assert!(lines[0].contains("id"), "头部应该包含 id 列");
            prop_assert!(lines[0].contains("provider"), "头部应该包含 provider 列");
            prop_assert!(lines[0].contains("model"), "头部应该包含 model 列");

            // 验证每个数据行包含 Flow ID
            for (i, flow) in flows.iter().enumerate() {
                prop_assert!(
                    lines[i + 1].contains(&flow.id),
                    "第 {} 行应该包含 Flow ID", i
                );
            }
        }

        /// **Feature: llm-flow-monitor, Property 8e: Markdown 导出包含关键信息**
        /// **Validates: Requirements 5.4**
        ///
        /// *对于任意* 有效的 LLM_Flow，导出为 Markdown 格式后，
        /// 应该包含 Flow 的关键信息。
        #[test]
        fn prop_export_markdown_content(flow in arb_llm_flow()) {
            let exporter = FlowExporter::with_defaults();

            // 导出为 Markdown
            let md = exporter.export_markdown(&flow);

            // 验证包含关键信息
            prop_assert!(md.contains(&flow.id), "Markdown 应该包含 Flow ID");
            prop_assert!(md.contains(&flow.request.model), "Markdown 应该包含模型名称");
            prop_assert!(md.contains("## 请求"), "Markdown 应该包含请求部分");

            // 如果有响应，验证包含响应部分
            if flow.response.is_some() {
                prop_assert!(md.contains("## 响应"), "Markdown 应该包含响应部分");
            }
        }
    }
}

// ============================================================================
// 脱敏属性测试模块
// ============================================================================

#[cfg(test)]
mod redaction_property_tests {
    use super::*;
    use crate::flow_monitor::models::*;
    use chrono::Utc;
    use proptest::prelude::*;
    use std::collections::HashMap;

    // ========================================================================
    // 敏感数据生成器
    // ========================================================================

    /// 生成随机邮箱地址
    fn arb_email() -> impl Strategy<Value = String> {
        (
            "[a-z]{3,10}",
            "[a-z]{3,10}",
            prop_oneof!["com", "org", "net", "io"],
        )
            .prop_map(|(user, domain, tld)| format!("{}@{}.{}", user, domain, tld))
    }

    /// 生成随机中国手机号
    fn arb_phone_cn() -> impl Strategy<Value = String> {
        (
            prop_oneof![Just("13"), Just("15"), Just("18"), Just("19")],
            "[0-9]{9}",
        )
            .prop_map(|(prefix, suffix)| format!("{}{}", prefix, suffix))
    }

    /// 生成随机 API 密钥
    fn arb_api_key() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9]{20,40}".prop_map(|s| format!("sk-{}", s))
    }

    /// 生成随机 Bearer Token
    fn arb_bearer_token() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9_.-]{20,50}".prop_map(|s| format!("Bearer {}", s))
    }

    /// 生成包含敏感数据的文本
    fn arb_text_with_sensitive_data() -> impl Strategy<Value = (String, Vec<String>)> {
        prop_oneof![
            // 包含邮箱
            arb_email().prop_map(|email| {
                let text = format!("Contact me at {} for more info.", email);
                (text, vec![email])
            }),
            // 包含手机号
            arb_phone_cn().prop_map(|phone| {
                let text = format!("My phone number is {}.", phone);
                (text, vec![phone])
            }),
            // 包含 API 密钥
            arb_api_key().prop_map(|key| {
                let text = format!("Use this API key: {}", key);
                (text, vec![key])
            }),
            // 包含 Bearer Token
            arb_bearer_token().prop_map(|token| {
                let text = format!("Authorization: {}", token);
                (text, vec![token])
            }),
            // 包含多种敏感数据
            (arb_email(), arb_phone_cn()).prop_map(|(email, phone)| {
                let text = format!("Email: {}, Phone: {}", email, phone);
                (text, vec![email, phone])
            }),
        ]
    }

    /// 生成随机的 ProviderType
    fn arb_provider_type() -> impl Strategy<Value = ProviderType> {
        prop_oneof![
            Just(ProviderType::Kiro),
            Just(ProviderType::OpenAI),
            Just(ProviderType::Claude),
        ]
    }

    /// 生成随机的 FlowType
    fn arb_flow_type() -> impl Strategy<Value = FlowType> {
        prop_oneof![
            Just(FlowType::ChatCompletions),
            Just(FlowType::AnthropicMessages),
        ]
    }

    /// 生成包含敏感数据的 LLMFlow
    fn arb_flow_with_sensitive_data() -> impl Strategy<Value = (LLMFlow, Vec<String>)> {
        (
            "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}",
            arb_flow_type(),
            arb_provider_type(),
            arb_text_with_sensitive_data(),
            arb_text_with_sensitive_data(),
        )
            .prop_map(
                |(
                    id,
                    flow_type,
                    provider,
                    (req_content, req_sensitive),
                    (resp_content, resp_sensitive),
                )| {
                    let request = LLMRequest {
                        method: "POST".to_string(),
                        path: "/v1/chat/completions".to_string(),
                        headers: HashMap::new(),
                        body: serde_json::Value::Null,
                        messages: vec![Message {
                            role: MessageRole::User,
                            content: MessageContent::Text(req_content),
                            tool_calls: None,
                            tool_result: None,
                            name: None,
                        }],
                        system_prompt: None,
                        tools: None,
                        model: "gpt-4".to_string(),
                        original_model: None,
                        parameters: RequestParameters::default(),
                        size_bytes: 0,
                        timestamp: Utc::now(),
                    };

                    let response = LLMResponse {
                        status_code: 200,
                        status_text: "OK".to_string(),
                        headers: HashMap::new(),
                        body: serde_json::Value::Null,
                        content: resp_content,
                        thinking: None,
                        tool_calls: Vec::new(),
                        usage: TokenUsage::default(),
                        stop_reason: Some(StopReason::Stop),
                        size_bytes: 0,
                        timestamp_start: Utc::now(),
                        timestamp_end: Utc::now(),
                        stream_info: None,
                    };

                    let metadata = FlowMetadata {
                        provider,
                        provider_id: None,
                        credential_id: None,
                        credential_name: None,
                        retry_count: 0,
                        client_info: ClientInfo::default(),
                        routing_info: RoutingInfo::default(),
                        injected_params: None,
                        context_usage_percentage: None,
                    };

                    let mut flow = LLMFlow::new(id, flow_type, request, metadata);
                    flow.response = Some(response);
                    flow.state = FlowState::Completed;

                    // 合并所有敏感数据
                    let mut all_sensitive = req_sensitive;
                    all_sensitive.extend(resp_sensitive);

                    (flow, all_sensitive)
                },
            )
    }

    // ========================================================================
    // 属性测试
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: llm-flow-monitor, Property 11: 脱敏正确性**
        /// **Validates: Requirements 8.1, 8.2, 8.3**
        ///
        /// *对于任意* 包含敏感数据（API 密钥、邮箱、手机号）的 Flow，
        /// 应用脱敏规则后，输出不应该包含原始敏感数据。
        #[test]
        fn prop_redaction_removes_sensitive_data(
            (flow, sensitive_data) in arb_flow_with_sensitive_data()
        ) {
            let redactor = Redactor::with_defaults();

            // 应用脱敏
            let redacted_flow = redactor.redact_flow(&flow);

            // 序列化为 JSON 以便检查
            let redacted_json = serde_json::to_string(&redacted_flow)
                .expect("应该能够序列化");

            // 验证所有敏感数据都已被脱敏
            for sensitive in &sensitive_data {
                prop_assert!(
                    !redacted_json.contains(sensitive),
                    "脱敏后的 JSON 不应该包含敏感数据: {}",
                    sensitive
                );
            }
        }

        /// **Feature: llm-flow-monitor, Property 11b: 脱敏后导出不包含敏感数据**
        /// **Validates: Requirements 8.1, 8.2, 8.3**
        ///
        /// *对于任意* 包含敏感数据的 Flow，使用启用脱敏的导出器导出后，
        /// 导出结果不应该包含原始敏感数据。
        #[test]
        fn prop_export_with_redaction_removes_sensitive_data(
            (flow, sensitive_data) in arb_flow_with_sensitive_data()
        ) {
            let options = ExportOptions {
                format: ExportFormat::JSON,
                redact_sensitive: true,
                ..Default::default()
            };
            let exporter = FlowExporter::new(options);

            // 导出
            let json = exporter.export_json(&[flow]);
            let json_str = serde_json::to_string(&json).expect("应该能够序列化");

            // 验证所有敏感数据都已被脱敏
            for sensitive in &sensitive_data {
                prop_assert!(
                    !json_str.contains(sensitive),
                    "导出的 JSON 不应该包含敏感数据: {}",
                    sensitive
                );
            }
        }

        /// **Feature: llm-flow-monitor, Property 11c: 脱敏保留非敏感数据**
        /// **Validates: Requirements 8.1, 8.2, 8.3**
        ///
        /// *对于任意* Flow，脱敏后应该保留非敏感的关键字段。
        #[test]
        fn prop_redaction_preserves_non_sensitive_data(
            (flow, _) in arb_flow_with_sensitive_data()
        ) {
            let redactor = Redactor::with_defaults();

            // 应用脱敏
            let redacted_flow = redactor.redact_flow(&flow);

            // 验证关键字段保持不变
            prop_assert_eq!(&flow.id, &redacted_flow.id, "Flow ID 应该保持不变");
            prop_assert_eq!(flow.state, redacted_flow.state, "状态应该保持不变");
            prop_assert_eq!(flow.flow_type, redacted_flow.flow_type, "FlowType 应该保持不变");
            prop_assert_eq!(&flow.request.model, &redacted_flow.request.model, "模型应该保持不变");
            prop_assert_eq!(&flow.request.method, &redacted_flow.request.method, "方法应该保持不变");
            prop_assert_eq!(flow.metadata.provider, redacted_flow.metadata.provider, "Provider 应该保持不变");

            // 验证响应存在性
            prop_assert_eq!(
                flow.response.is_some(),
                redacted_flow.response.is_some(),
                "响应存在性应该保持不变"
            );

            // 验证 Token 使用量保持不变
            if let (Some(ref orig), Some(ref redacted)) = (&flow.response, &redacted_flow.response) {
                prop_assert_eq!(
                    orig.usage.input_tokens,
                    redacted.usage.input_tokens,
                    "输入 Token 应该保持不变"
                );
                prop_assert_eq!(
                    orig.usage.output_tokens,
                    redacted.usage.output_tokens,
                    "输出 Token 应该保持不变"
                );
            }
        }

        /// **Feature: llm-flow-monitor, Property 11d: 邮箱脱敏**
        /// **Validates: Requirements 8.2**
        ///
        /// *对于任意* 包含邮箱的文本，脱敏后不应该包含原始邮箱。
        #[test]
        fn prop_redact_email(email in arb_email()) {
            let redactor = Redactor::with_defaults();
            let text = format!("Contact: {}", email);

            let redacted = redactor.redact(&text);

            prop_assert!(
                !redacted.contains(&email),
                "脱敏后不应该包含邮箱: {}",
                email
            );
            prop_assert!(
                redacted.contains("[REDACTED_EMAIL]"),
                "脱敏后应该包含占位符"
            );
        }

        /// **Feature: llm-flow-monitor, Property 11e: 手机号脱敏**
        /// **Validates: Requirements 8.2**
        ///
        /// *对于任意* 包含中国手机号的文本，脱敏后不应该包含原始手机号。
        #[test]
        fn prop_redact_phone(phone in arb_phone_cn()) {
            let redactor = Redactor::with_defaults();
            let text = format!("Phone: {}", phone);

            let redacted = redactor.redact(&text);

            prop_assert!(
                !redacted.contains(&phone),
                "脱敏后不应该包含手机号: {}",
                phone
            );
            prop_assert!(
                redacted.contains("[REDACTED_PHONE]"),
                "脱敏后应该包含占位符"
            );
        }

        /// **Feature: llm-flow-monitor, Property 11f: API 密钥脱敏**
        /// **Validates: Requirements 8.1**
        ///
        /// *对于任意* 包含 API 密钥的文本，脱敏后不应该包含原始密钥。
        #[test]
        fn prop_redact_api_key(key in arb_api_key()) {
            let redactor = Redactor::with_defaults();
            let text = format!("API Key: {}", key);

            let redacted = redactor.redact(&text);

            prop_assert!(
                !redacted.contains(&key),
                "脱敏后不应该包含 API 密钥: {}",
                key
            );
        }
    }
}
