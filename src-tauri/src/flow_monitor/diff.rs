//! Flow 差异对比模块
//!
//! 该模块实现两个 LLM Flow 之间的差异对比功能，支持请求、响应、元数据和 Token 使用量的对比。
//!
//! # 主要功能
//!
//! - 对比两个 Flow 的请求差异
//! - 对比两个 Flow 的响应差异
//! - 对比消息列表的差异
//! - 计算 Token 使用量差异
//! - 支持忽略动态字段（时间戳、ID 等）

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::models::{LLMFlow, Message, MessageContent, TokenUsage};

// ============================================================================
// 差异类型
// ============================================================================

/// 差异类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffType {
    /// 新增
    Added,
    /// 删除
    Removed,
    /// 修改
    Modified,
    /// 未变化
    Unchanged,
}

impl Default for DiffType {
    fn default() -> Self {
        DiffType::Unchanged
    }
}

// ============================================================================
// 差异项
// ============================================================================

/// 差异项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffItem {
    /// 字段路径
    pub path: String,
    /// 差异类型
    pub diff_type: DiffType,
    /// 左侧值（原始）
    pub left_value: Option<Value>,
    /// 右侧值（对比）
    pub right_value: Option<Value>,
}

impl DiffItem {
    /// 创建新增差异项
    pub fn added(path: impl Into<String>, value: Value) -> Self {
        Self {
            path: path.into(),
            diff_type: DiffType::Added,
            left_value: None,
            right_value: Some(value),
        }
    }

    /// 创建删除差异项
    pub fn removed(path: impl Into<String>, value: Value) -> Self {
        Self {
            path: path.into(),
            diff_type: DiffType::Removed,
            left_value: Some(value),
            right_value: None,
        }
    }

    /// 创建修改差异项
    pub fn modified(path: impl Into<String>, left: Value, right: Value) -> Self {
        Self {
            path: path.into(),
            diff_type: DiffType::Modified,
            left_value: Some(left),
            right_value: Some(right),
        }
    }

    /// 创建未变化差异项
    pub fn unchanged(path: impl Into<String>, value: Value) -> Self {
        Self {
            path: path.into(),
            diff_type: DiffType::Unchanged,
            left_value: Some(value.clone()),
            right_value: Some(value),
        }
    }
}

// ============================================================================
// 差异配置
// ============================================================================

/// 差异配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffConfig {
    /// 要忽略的字段列表
    pub ignore_fields: Vec<String>,
    /// 是否忽略时间戳
    pub ignore_timestamps: bool,
    /// 是否忽略 ID
    pub ignore_ids: bool,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            ignore_fields: vec![],
            ignore_timestamps: true,
            ignore_ids: true,
        }
    }
}

impl DiffConfig {
    /// 创建新的配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置忽略字段
    pub fn with_ignore_fields(mut self, fields: Vec<String>) -> Self {
        self.ignore_fields = fields;
        self
    }

    /// 设置是否忽略时间戳
    pub fn with_ignore_timestamps(mut self, ignore: bool) -> Self {
        self.ignore_timestamps = ignore;
        self
    }

    /// 设置是否忽略 ID
    pub fn with_ignore_ids(mut self, ignore: bool) -> Self {
        self.ignore_ids = ignore;
        self
    }

    /// 检查字段是否应该被忽略
    pub fn should_ignore(&self, path: &str) -> bool {
        // 检查自定义忽略字段
        if self.ignore_fields.iter().any(|f| path.contains(f)) {
            return true;
        }

        // 检查时间戳字段
        if self.ignore_timestamps {
            let timestamp_fields = [
                "timestamp",
                "created",
                "updated",
                "request_start",
                "request_end",
                "response_start",
                "response_end",
                "timestamp_start",
                "timestamp_end",
                "intercepted_at",
                "added_at",
                "created_at",
                "updated_at",
            ];
            if timestamp_fields.iter().any(|f| path.ends_with(f)) {
                return true;
            }
        }

        // 检查 ID 字段
        if self.ignore_ids {
            let id_fields = ["id", "flow_id", "request_id", "credential_id", "session_id"];
            if id_fields.iter().any(|f| path.ends_with(f)) {
                return true;
            }
        }

        false
    }
}

// ============================================================================
// Token 差异
// ============================================================================

/// Token 差异
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenDiff {
    /// 输入 Token 差异
    pub input_diff: i64,
    /// 输出 Token 差异
    pub output_diff: i64,
    /// 总 Token 差异
    pub total_diff: i64,
}

impl TokenDiff {
    /// 计算两个 TokenUsage 之间的差异
    pub fn from_usage(left: &TokenUsage, right: &TokenUsage) -> Self {
        Self {
            input_diff: right.input_tokens as i64 - left.input_tokens as i64,
            output_diff: right.output_tokens as i64 - left.output_tokens as i64,
            total_diff: right.total_tokens as i64 - left.total_tokens as i64,
        }
    }

    /// 检查是否有差异
    pub fn has_diff(&self) -> bool {
        self.input_diff != 0 || self.output_diff != 0 || self.total_diff != 0
    }
}

// ============================================================================
// 消息差异
// ============================================================================

/// 消息差异项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDiffItem {
    /// 消息索引
    pub index: usize,
    /// 差异类型
    pub diff_type: DiffType,
    /// 左侧消息
    pub left_message: Option<Message>,
    /// 右侧消息
    pub right_message: Option<Message>,
    /// 内容差异详情
    pub content_diffs: Vec<DiffItem>,
}

// ============================================================================
// Flow 差异结果
// ============================================================================

/// Flow 差异结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDiffResult {
    /// 左侧 Flow ID
    pub left_flow_id: String,
    /// 右侧 Flow ID
    pub right_flow_id: String,
    /// 请求差异
    pub request_diffs: Vec<DiffItem>,
    /// 响应差异
    pub response_diffs: Vec<DiffItem>,
    /// 元数据差异
    pub metadata_diffs: Vec<DiffItem>,
    /// 消息差异
    pub message_diffs: Vec<MessageDiffItem>,
    /// Token 差异
    pub token_diff: TokenDiff,
}

impl FlowDiffResult {
    /// 检查是否有任何差异
    pub fn has_diff(&self) -> bool {
        !self
            .request_diffs
            .iter()
            .all(|d| d.diff_type == DiffType::Unchanged)
            || !self
                .response_diffs
                .iter()
                .all(|d| d.diff_type == DiffType::Unchanged)
            || !self
                .metadata_diffs
                .iter()
                .all(|d| d.diff_type == DiffType::Unchanged)
            || !self
                .message_diffs
                .iter()
                .all(|d| d.diff_type == DiffType::Unchanged)
            || self.token_diff.has_diff()
    }

    /// 获取所有有变化的差异项
    pub fn get_changed_items(&self) -> Vec<&DiffItem> {
        let mut items = Vec::new();
        items.extend(
            self.request_diffs
                .iter()
                .filter(|d| d.diff_type != DiffType::Unchanged),
        );
        items.extend(
            self.response_diffs
                .iter()
                .filter(|d| d.diff_type != DiffType::Unchanged),
        );
        items.extend(
            self.metadata_diffs
                .iter()
                .filter(|d| d.diff_type != DiffType::Unchanged),
        );
        items
    }
}

// ============================================================================
// FlowDiff 核心实现
// ============================================================================

/// Flow 差异对比器
pub struct FlowDiff;

impl FlowDiff {
    /// 对比两个 Flow
    pub fn diff(left: &LLMFlow, right: &LLMFlow, config: &DiffConfig) -> FlowDiffResult {
        let request_diffs = Self::diff_requests(&left.request, &right.request, config);
        let response_diffs =
            Self::diff_responses(left.response.as_ref(), right.response.as_ref(), config);
        let metadata_diffs = Self::diff_metadata(&left.metadata, &right.metadata, config);
        let message_diffs = Self::diff_messages(&left.request.messages, &right.request.messages);
        let token_diff = Self::diff_tokens(
            left.response.as_ref().map(|r| &r.usage),
            right.response.as_ref().map(|r| &r.usage),
        );

        FlowDiffResult {
            left_flow_id: left.id.clone(),
            right_flow_id: right.id.clone(),
            request_diffs,
            response_diffs,
            metadata_diffs,
            message_diffs,
            token_diff,
        }
    }

    /// 对比请求
    fn diff_requests(
        left: &super::models::LLMRequest,
        right: &super::models::LLMRequest,
        config: &DiffConfig,
    ) -> Vec<DiffItem> {
        let mut diffs = Vec::new();

        // 对比模型
        if !config.should_ignore("request.model") {
            if left.model != right.model {
                diffs.push(DiffItem::modified(
                    "request.model",
                    Value::String(left.model.clone()),
                    Value::String(right.model.clone()),
                ));
            }
        }

        // 对比方法
        if !config.should_ignore("request.method") {
            if left.method != right.method {
                diffs.push(DiffItem::modified(
                    "request.method",
                    Value::String(left.method.clone()),
                    Value::String(right.method.clone()),
                ));
            }
        }

        // 对比路径
        if !config.should_ignore("request.path") {
            if left.path != right.path {
                diffs.push(DiffItem::modified(
                    "request.path",
                    Value::String(left.path.clone()),
                    Value::String(right.path.clone()),
                ));
            }
        }

        // 对比系统提示词
        if !config.should_ignore("request.system_prompt") {
            match (&left.system_prompt, &right.system_prompt) {
                (Some(l), Some(r)) if l != r => {
                    diffs.push(DiffItem::modified(
                        "request.system_prompt",
                        Value::String(l.clone()),
                        Value::String(r.clone()),
                    ));
                }
                (Some(l), None) => {
                    diffs.push(DiffItem::removed(
                        "request.system_prompt",
                        Value::String(l.clone()),
                    ));
                }
                (None, Some(r)) => {
                    diffs.push(DiffItem::added(
                        "request.system_prompt",
                        Value::String(r.clone()),
                    ));
                }
                _ => {}
            }
        }

        // 对比参数
        if !config.should_ignore("request.parameters") {
            Self::diff_parameters(&left.parameters, &right.parameters, &mut diffs, config);
        }

        // 对比请求体
        if !config.should_ignore("request.body") {
            let body_diffs = Self::diff_json(&left.body, &right.body, "request.body", config);
            diffs.extend(body_diffs);
        }

        diffs
    }

    /// 对比请求参数
    fn diff_parameters(
        left: &super::models::RequestParameters,
        right: &super::models::RequestParameters,
        diffs: &mut Vec<DiffItem>,
        config: &DiffConfig,
    ) {
        // 对比 temperature
        if !config.should_ignore("request.parameters.temperature") {
            match (left.temperature, right.temperature) {
                (Some(l), Some(r)) if (l - r).abs() > f32::EPSILON => {
                    diffs.push(DiffItem::modified(
                        "request.parameters.temperature",
                        serde_json::json!(l),
                        serde_json::json!(r),
                    ));
                }
                (Some(l), None) => {
                    diffs.push(DiffItem::removed(
                        "request.parameters.temperature",
                        serde_json::json!(l),
                    ));
                }
                (None, Some(r)) => {
                    diffs.push(DiffItem::added(
                        "request.parameters.temperature",
                        serde_json::json!(r),
                    ));
                }
                _ => {}
            }
        }

        // 对比 top_p
        if !config.should_ignore("request.parameters.top_p") {
            match (left.top_p, right.top_p) {
                (Some(l), Some(r)) if (l - r).abs() > f32::EPSILON => {
                    diffs.push(DiffItem::modified(
                        "request.parameters.top_p",
                        serde_json::json!(l),
                        serde_json::json!(r),
                    ));
                }
                (Some(l), None) => {
                    diffs.push(DiffItem::removed(
                        "request.parameters.top_p",
                        serde_json::json!(l),
                    ));
                }
                (None, Some(r)) => {
                    diffs.push(DiffItem::added(
                        "request.parameters.top_p",
                        serde_json::json!(r),
                    ));
                }
                _ => {}
            }
        }

        // 对比 max_tokens
        if !config.should_ignore("request.parameters.max_tokens") {
            match (left.max_tokens, right.max_tokens) {
                (Some(l), Some(r)) if l != r => {
                    diffs.push(DiffItem::modified(
                        "request.parameters.max_tokens",
                        serde_json::json!(l),
                        serde_json::json!(r),
                    ));
                }
                (Some(l), None) => {
                    diffs.push(DiffItem::removed(
                        "request.parameters.max_tokens",
                        serde_json::json!(l),
                    ));
                }
                (None, Some(r)) => {
                    diffs.push(DiffItem::added(
                        "request.parameters.max_tokens",
                        serde_json::json!(r),
                    ));
                }
                _ => {}
            }
        }

        // 对比 stream
        if !config.should_ignore("request.parameters.stream") && left.stream != right.stream {
            diffs.push(DiffItem::modified(
                "request.parameters.stream",
                serde_json::json!(left.stream),
                serde_json::json!(right.stream),
            ));
        }
    }

    /// 对比响应
    fn diff_responses(
        left: Option<&super::models::LLMResponse>,
        right: Option<&super::models::LLMResponse>,
        config: &DiffConfig,
    ) -> Vec<DiffItem> {
        let mut diffs = Vec::new();

        match (left, right) {
            (Some(l), Some(r)) => {
                // 对比状态码
                if !config.should_ignore("response.status_code") && l.status_code != r.status_code {
                    diffs.push(DiffItem::modified(
                        "response.status_code",
                        serde_json::json!(l.status_code),
                        serde_json::json!(r.status_code),
                    ));
                }

                // 对比内容
                if !config.should_ignore("response.content") && l.content != r.content {
                    diffs.push(DiffItem::modified(
                        "response.content",
                        Value::String(l.content.clone()),
                        Value::String(r.content.clone()),
                    ));
                }

                // 对比思维链
                if !config.should_ignore("response.thinking") {
                    match (&l.thinking, &r.thinking) {
                        (Some(lt), Some(rt)) if lt.text != rt.text => {
                            diffs.push(DiffItem::modified(
                                "response.thinking.text",
                                Value::String(lt.text.clone()),
                                Value::String(rt.text.clone()),
                            ));
                        }
                        (Some(lt), None) => {
                            diffs.push(DiffItem::removed(
                                "response.thinking",
                                serde_json::to_value(lt).unwrap_or(Value::Null),
                            ));
                        }
                        (None, Some(rt)) => {
                            diffs.push(DiffItem::added(
                                "response.thinking",
                                serde_json::to_value(rt).unwrap_or(Value::Null),
                            ));
                        }
                        _ => {}
                    }
                }

                // 对比停止原因
                if !config.should_ignore("response.stop_reason") {
                    match (&l.stop_reason, &r.stop_reason) {
                        (Some(ls), Some(rs)) if ls != rs => {
                            diffs.push(DiffItem::modified(
                                "response.stop_reason",
                                serde_json::to_value(ls).unwrap_or(Value::Null),
                                serde_json::to_value(rs).unwrap_or(Value::Null),
                            ));
                        }
                        (Some(ls), None) => {
                            diffs.push(DiffItem::removed(
                                "response.stop_reason",
                                serde_json::to_value(ls).unwrap_or(Value::Null),
                            ));
                        }
                        (None, Some(rs)) => {
                            diffs.push(DiffItem::added(
                                "response.stop_reason",
                                serde_json::to_value(rs).unwrap_or(Value::Null),
                            ));
                        }
                        _ => {}
                    }
                }

                // 对比工具调用数量
                if !config.should_ignore("response.tool_calls")
                    && l.tool_calls.len() != r.tool_calls.len()
                {
                    diffs.push(DiffItem::modified(
                        "response.tool_calls.count",
                        serde_json::json!(l.tool_calls.len()),
                        serde_json::json!(r.tool_calls.len()),
                    ));
                }

                // 对比响应体
                if !config.should_ignore("response.body") {
                    let body_diffs = Self::diff_json(&l.body, &r.body, "response.body", config);
                    diffs.extend(body_diffs);
                }
            }
            (Some(l), None) => {
                diffs.push(DiffItem::removed(
                    "response",
                    serde_json::to_value(l).unwrap_or(Value::Null),
                ));
            }
            (None, Some(r)) => {
                diffs.push(DiffItem::added(
                    "response",
                    serde_json::to_value(r).unwrap_or(Value::Null),
                ));
            }
            (None, None) => {}
        }

        diffs
    }

    /// 对比元数据
    fn diff_metadata(
        left: &super::models::FlowMetadata,
        right: &super::models::FlowMetadata,
        config: &DiffConfig,
    ) -> Vec<DiffItem> {
        let mut diffs = Vec::new();

        // 对比提供商
        if !config.should_ignore("metadata.provider") && left.provider != right.provider {
            diffs.push(DiffItem::modified(
                "metadata.provider",
                serde_json::to_value(&left.provider).unwrap_or(Value::Null),
                serde_json::to_value(&right.provider).unwrap_or(Value::Null),
            ));
        }

        // 对比凭证名称
        if !config.should_ignore("metadata.credential_name") {
            match (&left.credential_name, &right.credential_name) {
                (Some(l), Some(r)) if l != r => {
                    diffs.push(DiffItem::modified(
                        "metadata.credential_name",
                        Value::String(l.clone()),
                        Value::String(r.clone()),
                    ));
                }
                (Some(l), None) => {
                    diffs.push(DiffItem::removed(
                        "metadata.credential_name",
                        Value::String(l.clone()),
                    ));
                }
                (None, Some(r)) => {
                    diffs.push(DiffItem::added(
                        "metadata.credential_name",
                        Value::String(r.clone()),
                    ));
                }
                _ => {}
            }
        }

        // 对比重试次数
        if !config.should_ignore("metadata.retry_count") && left.retry_count != right.retry_count {
            diffs.push(DiffItem::modified(
                "metadata.retry_count",
                serde_json::json!(left.retry_count),
                serde_json::json!(right.retry_count),
            ));
        }

        diffs
    }

    /// 对比消息列表
    pub fn diff_messages(left: &[Message], right: &[Message]) -> Vec<MessageDiffItem> {
        let mut diffs = Vec::new();
        let max_len = left.len().max(right.len());

        for i in 0..max_len {
            match (left.get(i), right.get(i)) {
                (Some(l), Some(r)) => {
                    let content_diffs = Self::diff_message_content(l, r, i);
                    let diff_type = if content_diffs.is_empty() {
                        DiffType::Unchanged
                    } else {
                        DiffType::Modified
                    };
                    diffs.push(MessageDiffItem {
                        index: i,
                        diff_type,
                        left_message: Some(l.clone()),
                        right_message: Some(r.clone()),
                        content_diffs,
                    });
                }
                (Some(l), None) => {
                    diffs.push(MessageDiffItem {
                        index: i,
                        diff_type: DiffType::Removed,
                        left_message: Some(l.clone()),
                        right_message: None,
                        content_diffs: vec![],
                    });
                }
                (None, Some(r)) => {
                    diffs.push(MessageDiffItem {
                        index: i,
                        diff_type: DiffType::Added,
                        left_message: None,
                        right_message: Some(r.clone()),
                        content_diffs: vec![],
                    });
                }
                (None, None) => {}
            }
        }

        diffs
    }

    /// 对比单个消息的内容
    fn diff_message_content(left: &Message, right: &Message, index: usize) -> Vec<DiffItem> {
        let mut diffs = Vec::new();
        let prefix = format!("messages[{}]", index);

        // 对比角色
        if left.role != right.role {
            diffs.push(DiffItem::modified(
                format!("{}.role", prefix),
                serde_json::to_value(&left.role).unwrap_or(Value::Null),
                serde_json::to_value(&right.role).unwrap_or(Value::Null),
            ));
        }

        // 对比内容
        let left_text = Self::get_message_text(&left.content);
        let right_text = Self::get_message_text(&right.content);
        if left_text != right_text {
            diffs.push(DiffItem::modified(
                format!("{}.content", prefix),
                Value::String(left_text),
                Value::String(right_text),
            ));
        }

        // 对比名称
        match (&left.name, &right.name) {
            (Some(l), Some(r)) if l != r => {
                diffs.push(DiffItem::modified(
                    format!("{}.name", prefix),
                    Value::String(l.clone()),
                    Value::String(r.clone()),
                ));
            }
            (Some(l), None) => {
                diffs.push(DiffItem::removed(
                    format!("{}.name", prefix),
                    Value::String(l.clone()),
                ));
            }
            (None, Some(r)) => {
                diffs.push(DiffItem::added(
                    format!("{}.name", prefix),
                    Value::String(r.clone()),
                ));
            }
            _ => {}
        }

        // 对比工具调用
        match (&left.tool_calls, &right.tool_calls) {
            (Some(l), Some(r)) if l.len() != r.len() => {
                diffs.push(DiffItem::modified(
                    format!("{}.tool_calls.count", prefix),
                    serde_json::json!(l.len()),
                    serde_json::json!(r.len()),
                ));
            }
            (Some(l), None) => {
                diffs.push(DiffItem::removed(
                    format!("{}.tool_calls", prefix),
                    serde_json::to_value(l).unwrap_or(Value::Null),
                ));
            }
            (None, Some(r)) => {
                diffs.push(DiffItem::added(
                    format!("{}.tool_calls", prefix),
                    serde_json::to_value(r).unwrap_or(Value::Null),
                ));
            }
            _ => {}
        }

        diffs
    }

    /// 获取消息文本内容
    fn get_message_text(content: &MessageContent) -> String {
        match content {
            MessageContent::Text(s) => s.clone(),
            MessageContent::MultiModal(parts) => parts
                .iter()
                .filter_map(|p| {
                    if let super::models::ContentPart::Text { text } = p {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// 对比 Token 使用量
    fn diff_tokens(left: Option<&TokenUsage>, right: Option<&TokenUsage>) -> TokenDiff {
        match (left, right) {
            (Some(l), Some(r)) => TokenDiff::from_usage(l, r),
            (Some(l), None) => TokenDiff {
                input_diff: -(l.input_tokens as i64),
                output_diff: -(l.output_tokens as i64),
                total_diff: -(l.total_tokens as i64),
            },
            (None, Some(r)) => TokenDiff {
                input_diff: r.input_tokens as i64,
                output_diff: r.output_tokens as i64,
                total_diff: r.total_tokens as i64,
            },
            (None, None) => TokenDiff::default(),
        }
    }

    /// 对比两个 JSON 值
    pub fn diff_json(
        left: &Value,
        right: &Value,
        path: &str,
        config: &DiffConfig,
    ) -> Vec<DiffItem> {
        if config.should_ignore(path) {
            return vec![];
        }

        let mut diffs = Vec::new();

        match (left, right) {
            (Value::Object(l), Value::Object(r)) => {
                // 收集所有键
                let mut all_keys: Vec<_> = l.keys().chain(r.keys()).collect();
                all_keys.sort();
                all_keys.dedup();

                for key in all_keys {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };

                    match (l.get(key), r.get(key)) {
                        (Some(lv), Some(rv)) => {
                            diffs.extend(Self::diff_json(lv, rv, &new_path, config));
                        }
                        (Some(lv), None) => {
                            if !config.should_ignore(&new_path) {
                                diffs.push(DiffItem::removed(new_path, lv.clone()));
                            }
                        }
                        (None, Some(rv)) => {
                            if !config.should_ignore(&new_path) {
                                diffs.push(DiffItem::added(new_path, rv.clone()));
                            }
                        }
                        (None, None) => {}
                    }
                }
            }
            (Value::Array(l), Value::Array(r)) => {
                let max_len = l.len().max(r.len());
                for i in 0..max_len {
                    let new_path = format!("{}[{}]", path, i);
                    match (l.get(i), r.get(i)) {
                        (Some(lv), Some(rv)) => {
                            diffs.extend(Self::diff_json(lv, rv, &new_path, config));
                        }
                        (Some(lv), None) => {
                            if !config.should_ignore(&new_path) {
                                diffs.push(DiffItem::removed(new_path, lv.clone()));
                            }
                        }
                        (None, Some(rv)) => {
                            if !config.should_ignore(&new_path) {
                                diffs.push(DiffItem::added(new_path, rv.clone()));
                            }
                        }
                        (None, None) => {}
                    }
                }
            }
            _ => {
                if left != right && !config.should_ignore(path) {
                    diffs.push(DiffItem::modified(path, left.clone(), right.clone()));
                }
            }
        }

        diffs
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_monitor::models::{
        FlowMetadata, FlowType, LLMRequest, LLMResponse, Message, MessageRole, RequestParameters,
    };
    use crate::ProviderType;

    /// 创建测试用的 Flow
    fn create_test_flow(id: &str, model: &str, content: &str) -> LLMFlow {
        let request = LLMRequest {
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            model: model.to_string(),
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Text(content.to_string()),
                ..Default::default()
            }],
            parameters: RequestParameters::default(),
            ..Default::default()
        };

        let metadata = FlowMetadata {
            provider: ProviderType::OpenAI,
            ..Default::default()
        };

        let mut flow = LLMFlow::new(id.to_string(), FlowType::ChatCompletions, request, metadata);
        flow.response = Some(LLMResponse {
            content: "Response content".to_string(),
            usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                ..Default::default()
            },
            ..Default::default()
        });
        flow
    }

    #[test]
    fn test_diff_identical_flows() {
        let flow1 = create_test_flow("id1", "gpt-4", "Hello");
        let flow2 = create_test_flow("id2", "gpt-4", "Hello");
        let config = DiffConfig::default();

        let result = FlowDiff::diff(&flow1, &flow2, &config);

        // 由于 ID 被忽略，应该没有差异
        assert!(result.request_diffs.is_empty());
        assert!(result
            .message_diffs
            .iter()
            .all(|d| d.diff_type == DiffType::Unchanged));
    }

    #[test]
    fn test_diff_different_models() {
        let flow1 = create_test_flow("id1", "gpt-4", "Hello");
        let flow2 = create_test_flow("id2", "gpt-3.5-turbo", "Hello");
        let config = DiffConfig::default();

        let result = FlowDiff::diff(&flow1, &flow2, &config);

        let model_diff = result
            .request_diffs
            .iter()
            .find(|d| d.path == "request.model");
        assert!(model_diff.is_some());
        assert_eq!(model_diff.unwrap().diff_type, DiffType::Modified);
    }

    #[test]
    fn test_diff_different_messages() {
        let flow1 = create_test_flow("id1", "gpt-4", "Hello");
        let flow2 = create_test_flow("id2", "gpt-4", "World");
        let config = DiffConfig::default();

        let result = FlowDiff::diff(&flow1, &flow2, &config);

        assert!(!result.message_diffs.is_empty());
        assert_eq!(result.message_diffs[0].diff_type, DiffType::Modified);
    }

    #[test]
    fn test_token_diff() {
        let usage1 = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            ..Default::default()
        };
        let usage2 = TokenUsage {
            input_tokens: 120,
            output_tokens: 60,
            total_tokens: 180,
            ..Default::default()
        };

        let diff = TokenDiff::from_usage(&usage1, &usage2);

        assert_eq!(diff.input_diff, 20);
        assert_eq!(diff.output_diff, 10);
        assert_eq!(diff.total_diff, 30);
        assert!(diff.has_diff());
    }

    #[test]
    fn test_diff_config_ignore_timestamps() {
        let config = DiffConfig::default();
        assert!(config.should_ignore("timestamps.created"));
        assert!(config.should_ignore("response.timestamp_start"));
        assert!(!config.should_ignore("request.model"));
    }

    #[test]
    fn test_diff_config_ignore_ids() {
        let config = DiffConfig::default();
        assert!(config.should_ignore("flow.id"));
        assert!(config.should_ignore("metadata.credential_id"));
        assert!(!config.should_ignore("request.model"));
    }

    #[test]
    fn test_diff_json_objects() {
        let left = serde_json::json!({
            "a": 1,
            "b": 2,
            "c": 3
        });
        let right = serde_json::json!({
            "a": 1,
            "b": 3,
            "d": 4
        });
        let config = DiffConfig::new()
            .with_ignore_timestamps(false)
            .with_ignore_ids(false);

        let diffs = FlowDiff::diff_json(&left, &right, "root", &config);

        // b 被修改，c 被删除，d 被添加
        assert_eq!(diffs.len(), 3);
        assert!(diffs
            .iter()
            .any(|d| d.path == "root.b" && d.diff_type == DiffType::Modified));
        assert!(diffs
            .iter()
            .any(|d| d.path == "root.c" && d.diff_type == DiffType::Removed));
        assert!(diffs
            .iter()
            .any(|d| d.path == "root.d" && d.diff_type == DiffType::Added));
    }

    #[test]
    fn test_diff_messages_added() {
        let left = vec![Message {
            role: MessageRole::User,
            content: MessageContent::Text("Hello".to_string()),
            ..Default::default()
        }];
        let right = vec![
            Message {
                role: MessageRole::User,
                content: MessageContent::Text("Hello".to_string()),
                ..Default::default()
            },
            Message {
                role: MessageRole::Assistant,
                content: MessageContent::Text("Hi there".to_string()),
                ..Default::default()
            },
        ];

        let diffs = FlowDiff::diff_messages(&left, &right);

        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].diff_type, DiffType::Unchanged);
        assert_eq!(diffs[1].diff_type, DiffType::Added);
    }

    #[test]
    fn test_diff_messages_removed() {
        let left = vec![
            Message {
                role: MessageRole::User,
                content: MessageContent::Text("Hello".to_string()),
                ..Default::default()
            },
            Message {
                role: MessageRole::Assistant,
                content: MessageContent::Text("Hi there".to_string()),
                ..Default::default()
            },
        ];
        let right = vec![Message {
            role: MessageRole::User,
            content: MessageContent::Text("Hello".to_string()),
            ..Default::default()
        }];

        let diffs = FlowDiff::diff_messages(&left, &right);

        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].diff_type, DiffType::Unchanged);
        assert_eq!(diffs[1].diff_type, DiffType::Removed);
    }
}

// ============================================================================
// 属性测试模块
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::flow_monitor::models::{
        FlowAnnotations, FlowMetadata, FlowState, FlowTimestamps, FlowType, LLMRequest,
        LLMResponse, Message, MessageRole, RequestParameters, TokenUsage,
    };
    use crate::ProviderType;
    use chrono::Utc;
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

    /// 生成随机的 MessageContent
    fn arb_message_content() -> impl Strategy<Value = MessageContent> {
        "[a-zA-Z0-9 ]{1,100}".prop_map(MessageContent::Text)
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
                    extra: std::collections::HashMap::new(),
                },
            )
    }

    /// 生成随机的 TokenUsage
    fn arb_token_usage() -> impl Strategy<Value = TokenUsage> {
        (0u32..10000u32, 0u32..10000u32).prop_map(|(input, output)| TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
            ..Default::default()
        })
    }

    /// 生成随机的 LLMRequest
    fn arb_llm_request() -> impl Strategy<Value = LLMRequest> {
        (
            "[a-z]{3,20}",                              // model
            prop::collection::vec(arb_message(), 1..5), // messages
            arb_request_parameters(),                   // parameters
            prop::option::of("[a-zA-Z0-9 ]{10,50}"),    // system_prompt
        )
            .prop_map(|(model, messages, parameters, system_prompt)| LLMRequest {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                headers: std::collections::HashMap::new(),
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

    /// 生成随机的 LLMResponse
    fn arb_llm_response() -> impl Strategy<Value = LLMResponse> {
        (
            "[a-zA-Z0-9 ]{10,200}", // content
            arb_token_usage(),      // usage
        )
            .prop_map(|(content, usage)| LLMResponse {
                status_code: 200,
                status_text: "OK".to_string(),
                headers: std::collections::HashMap::new(),
                body: serde_json::Value::Null,
                content,
                thinking: None,
                tool_calls: vec![],
                usage,
                stop_reason: None,
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
            client_info: Default::default(),
            routing_info: Default::default(),
            injected_params: None,
            context_usage_percentage: None,
        })
    }

    /// 生成随机的 LLMFlow
    fn arb_llm_flow() -> impl Strategy<Value = LLMFlow> {
        (
            "[a-f0-9]{8}",
            arb_llm_request(),
            arb_flow_metadata(),
            prop::option::of(arb_llm_response()),
        )
            .prop_map(|(id, request, metadata, response)| {
                let mut flow = LLMFlow::new(id, FlowType::ChatCompletions, request, metadata);
                flow.response = response;
                flow
            })
    }

    /// 生成随机的 DiffConfig
    fn arb_diff_config() -> impl Strategy<Value = DiffConfig> {
        (any::<bool>(), any::<bool>()).prop_map(|(ignore_timestamps, ignore_ids)| DiffConfig {
            ignore_fields: vec![],
            ignore_timestamps,
            ignore_ids,
        })
    }

    // ========================================================================
    // 属性测试
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: flow-monitor-enhancement, Property 6: 差异计算正确性**
        /// **Validates: Requirements 4.1, 4.2, 4.5, 4.6, 4.7**
        ///
        /// *对于任意* 两个 Flow，差异计算应该正确识别所有新增、删除和修改的字段，
        /// 且忽略配置中指定的字段。
        #[test]
        fn prop_diff_correctness(
            flow1 in arb_llm_flow(),
            flow2 in arb_llm_flow(),
            config in arb_diff_config(),
        ) {
            let result = FlowDiff::diff(&flow1, &flow2, &config);

            // 验证 Flow ID 正确记录
            prop_assert_eq!(&result.left_flow_id, &flow1.id);
            prop_assert_eq!(&result.right_flow_id, &flow2.id);

            // 验证模型差异检测
            if flow1.request.model != flow2.request.model {
                let model_diff = result.request_diffs.iter().find(|d| d.path == "request.model");
                prop_assert!(model_diff.is_some(), "模型不同时应该检测到差异");
                prop_assert_eq!(model_diff.unwrap().diff_type, DiffType::Modified);
            }

            // 验证消息数量差异检测
            let left_msg_count = flow1.request.messages.len();
            let right_msg_count = flow2.request.messages.len();
            prop_assert_eq!(
                result.message_diffs.len(),
                left_msg_count.max(right_msg_count),
                "消息差异数量应该等于两个消息列表的最大长度"
            );

            // 验证 Token 差异计算
            if let (Some(r1), Some(r2)) = (&flow1.response, &flow2.response) {
                let expected_input_diff = r2.usage.input_tokens as i64 - r1.usage.input_tokens as i64;
                let expected_output_diff = r2.usage.output_tokens as i64 - r1.usage.output_tokens as i64;
                prop_assert_eq!(result.token_diff.input_diff, expected_input_diff);
                prop_assert_eq!(result.token_diff.output_diff, expected_output_diff);
            }

            // 验证忽略字段配置生效
            for diff in &result.request_diffs {
                prop_assert!(
                    !config.should_ignore(&diff.path),
                    "被忽略的字段不应该出现在差异结果中: {}",
                    diff.path
                );
            }
            for diff in &result.response_diffs {
                prop_assert!(
                    !config.should_ignore(&diff.path),
                    "被忽略的字段不应该出现在差异结果中: {}",
                    diff.path
                );
            }
            for diff in &result.metadata_diffs {
                prop_assert!(
                    !config.should_ignore(&diff.path),
                    "被忽略的字段不应该出现在差异结果中: {}",
                    diff.path
                );
            }
        }

        /// **Feature: flow-monitor-enhancement, Property 7: 差异计算对称性**
        /// **Validates: Requirements 4.1, 4.2**
        ///
        /// *对于任意* 两个 Flow A 和 B，diff(A, B) 中的 "Added" 项应该对应 diff(B, A) 中的 "Removed" 项。
        #[test]
        fn prop_diff_symmetry(
            flow1 in arb_llm_flow(),
            flow2 in arb_llm_flow(),
        ) {
            let config = DiffConfig::default();
            let result_ab = FlowDiff::diff(&flow1, &flow2, &config);
            let result_ba = FlowDiff::diff(&flow2, &flow1, &config);

            // 验证请求差异对称性
            for diff_ab in &result_ab.request_diffs {
                let corresponding = result_ba.request_diffs.iter().find(|d| d.path == diff_ab.path);
                if let Some(diff_ba) = corresponding {
                    match diff_ab.diff_type {
                        DiffType::Added => {
                            prop_assert_eq!(
                                diff_ba.diff_type,
                                DiffType::Removed,
                                "A->B 的 Added 应该对应 B->A 的 Removed: {}",
                                diff_ab.path
                            );
                        }
                        DiffType::Removed => {
                            prop_assert_eq!(
                                diff_ba.diff_type,
                                DiffType::Added,
                                "A->B 的 Removed 应该对应 B->A 的 Added: {}",
                                diff_ab.path
                            );
                        }
                        DiffType::Modified => {
                            prop_assert_eq!(
                                diff_ba.diff_type,
                                DiffType::Modified,
                                "A->B 的 Modified 应该对应 B->A 的 Modified: {}",
                                diff_ab.path
                            );
                            // 验证值交换
                            prop_assert_eq!(
                                &diff_ab.left_value,
                                &diff_ba.right_value,
                                "Modified 差异的值应该交换"
                            );
                            prop_assert_eq!(
                                &diff_ab.right_value,
                                &diff_ba.left_value,
                                "Modified 差异的值应该交换"
                            );
                        }
                        DiffType::Unchanged => {}
                    }
                }
            }

            // 验证消息差异对称性
            for (i, diff_ab) in result_ab.message_diffs.iter().enumerate() {
                if let Some(diff_ba) = result_ba.message_diffs.get(i) {
                    match diff_ab.diff_type {
                        DiffType::Added => {
                            prop_assert_eq!(
                                diff_ba.diff_type,
                                DiffType::Removed,
                                "消息 {} A->B 的 Added 应该对应 B->A 的 Removed",
                                i
                            );
                        }
                        DiffType::Removed => {
                            prop_assert_eq!(
                                diff_ba.diff_type,
                                DiffType::Added,
                                "消息 {} A->B 的 Removed 应该对应 B->A 的 Added",
                                i
                            );
                        }
                        _ => {}
                    }
                }
            }

            // 验证 Token 差异对称性
            prop_assert_eq!(
                result_ab.token_diff.input_diff,
                -result_ba.token_diff.input_diff,
                "Token 输入差异应该相反"
            );
            prop_assert_eq!(
                result_ab.token_diff.output_diff,
                -result_ba.token_diff.output_diff,
                "Token 输出差异应该相反"
            );
            prop_assert_eq!(
                result_ab.token_diff.total_diff,
                -result_ba.token_diff.total_diff,
                "Token 总差异应该相反"
            );
        }

        /// **Feature: flow-monitor-enhancement, Property 6b: 相同 Flow 无差异**
        /// **Validates: Requirements 4.1, 4.2**
        ///
        /// *对于任意* Flow，与自身对比应该没有差异（除了被忽略的字段）。
        #[test]
        fn prop_diff_self_no_changes(
            flow in arb_llm_flow(),
        ) {
            let config = DiffConfig::default();
            let result = FlowDiff::diff(&flow, &flow, &config);

            // 验证请求差异为空或全部为 Unchanged
            for diff in &result.request_diffs {
                prop_assert_eq!(
                    diff.diff_type,
                    DiffType::Unchanged,
                    "自身对比不应该有请求差异: {}",
                    diff.path
                );
            }

            // 验证响应差异为空或全部为 Unchanged
            for diff in &result.response_diffs {
                prop_assert_eq!(
                    diff.diff_type,
                    DiffType::Unchanged,
                    "自身对比不应该有响应差异: {}",
                    diff.path
                );
            }

            // 验证消息差异全部为 Unchanged
            for diff in &result.message_diffs {
                prop_assert_eq!(
                    diff.diff_type,
                    DiffType::Unchanged,
                    "自身对比不应该有消息差异"
                );
            }

            // 验证 Token 差异为零
            prop_assert_eq!(result.token_diff.input_diff, 0);
            prop_assert_eq!(result.token_diff.output_diff, 0);
            prop_assert_eq!(result.token_diff.total_diff, 0);
        }

        /// **Feature: flow-monitor-enhancement, Property 6c: Token 差异计算正确性**
        /// **Validates: Requirements 4.7**
        ///
        /// *对于任意* 两个 TokenUsage，差异计算应该正确。
        #[test]
        fn prop_token_diff_correctness(
            usage1 in arb_token_usage(),
            usage2 in arb_token_usage(),
        ) {
            let diff = TokenDiff::from_usage(&usage1, &usage2);

            // 验证差异计算
            prop_assert_eq!(
                diff.input_diff,
                usage2.input_tokens as i64 - usage1.input_tokens as i64
            );
            prop_assert_eq!(
                diff.output_diff,
                usage2.output_tokens as i64 - usage1.output_tokens as i64
            );
            prop_assert_eq!(
                diff.total_diff,
                usage2.total_tokens as i64 - usage1.total_tokens as i64
            );

            // 验证 has_diff 正确性
            let expected_has_diff = diff.input_diff != 0 || diff.output_diff != 0 || diff.total_diff != 0;
            prop_assert_eq!(diff.has_diff(), expected_has_diff);
        }

        /// **Feature: flow-monitor-enhancement, Property 6d: 消息差异计算正确性**
        /// **Validates: Requirements 4.6**
        ///
        /// *对于任意* 两个消息列表，差异计算应该正确识别新增、删除和修改的消息。
        #[test]
        fn prop_message_diff_correctness(
            messages1 in prop::collection::vec(arb_message(), 0..5),
            messages2 in prop::collection::vec(arb_message(), 0..5),
        ) {
            let diffs = FlowDiff::diff_messages(&messages1, &messages2);

            // 验证差异数量
            let expected_len = messages1.len().max(messages2.len());
            prop_assert_eq!(diffs.len(), expected_len);

            // 验证每个差异项
            for (i, diff) in diffs.iter().enumerate() {
                prop_assert_eq!(diff.index, i);

                match (messages1.get(i), messages2.get(i)) {
                    (Some(_), Some(_)) => {
                        // 两边都有消息，应该是 Modified 或 Unchanged
                        prop_assert!(
                            diff.diff_type == DiffType::Modified || diff.diff_type == DiffType::Unchanged,
                            "两边都有消息时应该是 Modified 或 Unchanged"
                        );
                        prop_assert!(diff.left_message.is_some());
                        prop_assert!(diff.right_message.is_some());
                    }
                    (Some(_), None) => {
                        // 只有左边有消息，应该是 Removed
                        prop_assert_eq!(diff.diff_type, DiffType::Removed);
                        prop_assert!(diff.left_message.is_some());
                        prop_assert!(diff.right_message.is_none());
                    }
                    (None, Some(_)) => {
                        // 只有右边有消息，应该是 Added
                        prop_assert_eq!(diff.diff_type, DiffType::Added);
                        prop_assert!(diff.left_message.is_none());
                        prop_assert!(diff.right_message.is_some());
                    }
                    (None, None) => {
                        // 不应该发生
                        prop_assert!(false, "不应该有两边都没有消息的差异项");
                    }
                }
            }
        }
    }
}
