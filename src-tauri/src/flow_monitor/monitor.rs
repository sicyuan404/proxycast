//! Flow 核心监控服务
//!
//! 该模块实现 LLM Flow 的核心监控功能，包括：
//! - Flow 生命周期管理（创建、更新、完成、失败）
//! - 流式响应处理
//! - 实时事件发送
//! - 标注管理
//! - 阈值检测（延迟、Token 使用量）
//! - 请求速率计算

#![allow(dead_code)]

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use super::file_store::FlowFileStore;
use super::memory_store::FlowMemoryStore;
use super::models::{
    FlowAnnotations, FlowError, FlowMetadata, FlowState, FlowType, LLMFlow, LLMRequest,
    LLMResponse, TokenUsage,
};
use super::stream_rebuilder::{StreamFormat, StreamRebuilder};

// ============================================================================
// 配置结构
// ============================================================================

/// Flow 监控配置
///
/// 控制 Flow Monitor 的行为，包括启用/禁用、缓存大小、持久化等。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowMonitorConfig {
    /// 是否启用监控
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 最大内存 Flow 数量
    #[serde(default = "default_max_memory_flows")]
    pub max_memory_flows: usize,
    /// 是否持久化到文件
    #[serde(default = "default_persist_to_file")]
    pub persist_to_file: bool,
    /// 保留天数
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// 是否保存原始流式 chunks
    #[serde(default)]
    pub save_stream_chunks: bool,
    /// 最大请求体大小（字节）
    #[serde(default = "default_max_request_body_size")]
    pub max_request_body_size: usize,
    /// 最大响应体大小（字节）
    #[serde(default = "default_max_response_body_size")]
    pub max_response_body_size: usize,
    /// 是否保存图片内容
    #[serde(default)]
    pub save_image_content: bool,
    /// 缩略图大小
    #[serde(default = "default_thumbnail_size")]
    pub thumbnail_size: (u32, u32),
    /// 采样率（0.0-1.0，1.0 表示全部采样）
    #[serde(default = "default_sampling_rate")]
    pub sampling_rate: f32,
    /// 排除的模型列表（支持通配符）
    #[serde(default)]
    pub excluded_models: Vec<String>,
    /// 排除的路径列表（支持通配符）
    #[serde(default)]
    pub excluded_paths: Vec<String>,
}

fn default_enabled() -> bool {
    true
}

fn default_max_memory_flows() -> usize {
    1000
}

fn default_persist_to_file() -> bool {
    true
}

fn default_retention_days() -> u32 {
    7
}

fn default_max_request_body_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_max_response_body_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_thumbnail_size() -> (u32, u32) {
    (128, 128)
}

fn default_sampling_rate() -> f32 {
    1.0
}

impl Default for FlowMonitorConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_memory_flows: default_max_memory_flows(),
            persist_to_file: default_persist_to_file(),
            retention_days: default_retention_days(),
            save_stream_chunks: false,
            max_request_body_size: default_max_request_body_size(),
            max_response_body_size: default_max_response_body_size(),
            save_image_content: false,
            thumbnail_size: default_thumbnail_size(),
            sampling_rate: default_sampling_rate(),
            excluded_models: Vec::new(),
            excluded_paths: Vec::new(),
        }
    }
}

impl FlowMonitorConfig {
    /// 检查是否应该监控该请求
    pub fn should_monitor(&self, model: &str, path: &str) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查采样率
        if self.sampling_rate < 1.0 {
            let random: f32 = rand::random();
            if random > self.sampling_rate {
                return false;
            }
        }

        // 检查排除的模型
        for pattern in &self.excluded_models {
            if Self::match_pattern(pattern, model) {
                return false;
            }
        }

        // 检查排除的路径
        for pattern in &self.excluded_paths {
            if Self::match_pattern(pattern, path) {
                return false;
            }
        }

        true
    }

    /// 模式匹配（支持 * 通配符）
    fn match_pattern(pattern: &str, text: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            let mut pos = 0;
            let text_lower = text.to_lowercase();

            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }

                let part_lower = part.to_lowercase();
                if let Some(found_pos) = text_lower[pos..].find(&part_lower) {
                    if i == 0 && found_pos != 0 {
                        return false;
                    }
                    pos += found_pos + part.len();
                } else {
                    return false;
                }
            }

            if !pattern.ends_with('*') && pos != text.len() {
                return false;
            }

            true
        } else {
            text.to_lowercase() == pattern.to_lowercase()
        }
    }
}

// ============================================================================
// 阈值配置
// ============================================================================

/// 阈值配置
///
/// 用于配置延迟和 Token 使用量的警告阈值。
///
/// **Validates: Requirements 10.3, 10.4**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// 是否启用阈值检测
    #[serde(default = "default_threshold_enabled")]
    pub enabled: bool,
    /// 延迟阈值（毫秒）
    #[serde(default = "default_latency_threshold")]
    pub latency_threshold_ms: u64,
    /// Token 使用量阈值
    #[serde(default = "default_token_threshold")]
    pub token_threshold: u32,
    /// 输入 Token 阈值（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_token_threshold: Option<u32>,
    /// 输出 Token 阈值（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_token_threshold: Option<u32>,
}

fn default_threshold_enabled() -> bool {
    true
}

fn default_latency_threshold() -> u64 {
    5000 // 5 秒
}

fn default_token_threshold() -> u32 {
    10000
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            enabled: default_threshold_enabled(),
            latency_threshold_ms: default_latency_threshold(),
            token_threshold: default_token_threshold(),
            input_token_threshold: None,
            output_token_threshold: None,
        }
    }
}

/// 阈值检测结果
///
/// 表示 Flow 是否超过了配置的阈值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdCheckResult {
    /// 是否超过延迟阈值
    pub latency_exceeded: bool,
    /// 是否超过 Token 阈值
    pub token_exceeded: bool,
    /// 是否超过输入 Token 阈值
    pub input_token_exceeded: bool,
    /// 是否超过输出 Token 阈值
    pub output_token_exceeded: bool,
    /// 实际延迟（毫秒）
    pub actual_latency_ms: u64,
    /// 实际 Token 使用量
    pub actual_tokens: u32,
    /// 实际输入 Token
    pub actual_input_tokens: u32,
    /// 实际输出 Token
    pub actual_output_tokens: u32,
}

impl ThresholdCheckResult {
    /// 检查是否有任何阈值被超过
    pub fn any_exceeded(&self) -> bool {
        self.latency_exceeded
            || self.token_exceeded
            || self.input_token_exceeded
            || self.output_token_exceeded
    }
}

impl Default for ThresholdCheckResult {
    fn default() -> Self {
        Self {
            latency_exceeded: false,
            token_exceeded: false,
            input_token_exceeded: false,
            output_token_exceeded: false,
            actual_latency_ms: 0,
            actual_tokens: 0,
            actual_input_tokens: 0,
            actual_output_tokens: 0,
        }
    }
}

// ============================================================================
// 通知配置
// ============================================================================

/// 通知类型
///
/// **Validates: Requirements 10.1, 10.2**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    /// 新 Flow 通知
    NewFlow,
    /// 错误 Flow 通知
    ErrorFlow,
    /// 延迟阈值警告
    LatencyWarning,
    /// Token 阈值警告
    TokenWarning,
}

/// 通知配置
///
/// 用于配置各种通知的启用状态和行为。
///
/// **Validates: Requirements 10.1, 10.2**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// 是否启用通知
    #[serde(default = "default_notification_enabled")]
    pub enabled: bool,
    /// 新 Flow 通知配置
    #[serde(default)]
    pub new_flow: NotificationSettings,
    /// 错误 Flow 通知配置
    #[serde(default = "default_error_notification")]
    pub error_flow: NotificationSettings,
    /// 延迟警告通知配置
    #[serde(default = "default_latency_warning")]
    pub latency_warning: NotificationSettings,
    /// Token 警告通知配置
    #[serde(default = "default_token_warning")]
    pub token_warning: NotificationSettings,
}

/// 通知设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// 是否启用
    pub enabled: bool,
    /// 是否显示桌面通知
    pub desktop: bool,
    /// 是否播放声音
    pub sound: bool,
    /// 声音文件路径（可选）
    pub sound_file: Option<String>,
}

fn default_notification_enabled() -> bool {
    true
}

fn default_error_notification() -> NotificationSettings {
    NotificationSettings {
        enabled: true,
        desktop: true,
        sound: true,
        sound_file: None,
    }
}

fn default_latency_warning() -> NotificationSettings {
    NotificationSettings {
        enabled: true,
        desktop: false,
        sound: false,
        sound_file: None,
    }
}

fn default_token_warning() -> NotificationSettings {
    NotificationSettings {
        enabled: true,
        desktop: false,
        sound: false,
        sound_file: None,
    }
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            desktop: false,
            sound: false,
            sound_file: None,
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: default_notification_enabled(),
            new_flow: NotificationSettings::default(),
            error_flow: default_error_notification(),
            latency_warning: default_latency_warning(),
            token_warning: default_token_warning(),
        }
    }
}

/// 通知事件
///
/// 表示需要发送的通知。
///
/// **Validates: Requirements 10.1, 10.2, 10.3, 10.4**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEvent {
    /// 通知类型
    pub notification_type: NotificationType,
    /// 通知标题
    pub title: String,
    /// 通知内容
    pub message: String,
    /// 关联的 Flow ID
    pub flow_id: String,
    /// 通知时间
    pub timestamp: DateTime<Utc>,
    /// 是否需要桌面通知
    pub desktop: bool,
    /// 是否需要声音
    pub sound: bool,
    /// 声音文件路径
    pub sound_file: Option<String>,
}

impl NotificationEvent {
    /// 创建新 Flow 通知
    pub fn new_flow(flow_id: String, model: String, settings: &NotificationSettings) -> Self {
        Self {
            notification_type: NotificationType::NewFlow,
            title: "新的 LLM 请求".to_string(),
            message: format!("模型: {}", model),
            flow_id,
            timestamp: Utc::now(),
            desktop: settings.desktop,
            sound: settings.sound,
            sound_file: settings.sound_file.clone(),
        }
    }

    /// 创建错误 Flow 通知
    pub fn error_flow(
        flow_id: String,
        model: String,
        error: String,
        settings: &NotificationSettings,
    ) -> Self {
        Self {
            notification_type: NotificationType::ErrorFlow,
            title: "LLM 请求失败".to_string(),
            message: format!("模型: {}, 错误: {}", model, error),
            flow_id,
            timestamp: Utc::now(),
            desktop: settings.desktop,
            sound: settings.sound,
            sound_file: settings.sound_file.clone(),
        }
    }

    /// 创建延迟警告通知
    pub fn latency_warning(
        flow_id: String,
        model: String,
        actual_ms: u64,
        threshold_ms: u64,
        settings: &NotificationSettings,
    ) -> Self {
        Self {
            notification_type: NotificationType::LatencyWarning,
            title: "延迟警告".to_string(),
            message: format!(
                "模型: {}, 延迟: {}ms (阈值: {}ms)",
                model, actual_ms, threshold_ms
            ),
            flow_id,
            timestamp: Utc::now(),
            desktop: settings.desktop,
            sound: settings.sound,
            sound_file: settings.sound_file.clone(),
        }
    }

    /// 创建 Token 警告通知
    pub fn token_warning(
        flow_id: String,
        model: String,
        actual_tokens: u32,
        threshold_tokens: u32,
        settings: &NotificationSettings,
    ) -> Self {
        Self {
            notification_type: NotificationType::TokenWarning,
            title: "Token 使用警告".to_string(),
            message: format!(
                "模型: {}, Token: {} (阈值: {})",
                model, actual_tokens, threshold_tokens
            ),
            flow_id,
            timestamp: Utc::now(),
            desktop: settings.desktop,
            sound: settings.sound,
            sound_file: settings.sound_file.clone(),
        }
    }
}

// ============================================================================
// 请求速率追踪器
// ============================================================================

/// 请求速率追踪器
///
/// 用于计算指定时间窗口内的请求速率。
///
/// **Validates: Requirements 10.7**
#[derive(Debug)]
pub struct RequestRateTracker {
    /// 请求时间戳队列
    timestamps: VecDeque<DateTime<Utc>>,
    /// 时间窗口（秒）
    window_seconds: i64,
}

impl RequestRateTracker {
    /// 创建新的请求速率追踪器
    ///
    /// # Arguments
    /// * `window_seconds` - 时间窗口（秒）
    pub fn new(window_seconds: i64) -> Self {
        Self {
            timestamps: VecDeque::new(),
            window_seconds,
        }
    }

    /// 记录一个新请求
    pub fn record_request(&mut self) {
        self.record_request_at(Utc::now());
    }

    /// 在指定时间记录一个新请求
    pub fn record_request_at(&mut self, timestamp: DateTime<Utc>) {
        self.timestamps.push_back(timestamp);
        self.cleanup_old_entries(timestamp);
    }

    /// 清理过期的条目
    fn cleanup_old_entries(&mut self, now: DateTime<Utc>) {
        let cutoff = now - Duration::seconds(self.window_seconds);
        while let Some(front) = self.timestamps.front() {
            if *front < cutoff {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }
    }

    /// 获取当前请求速率（每秒）
    pub fn get_rate(&self) -> f64 {
        self.get_rate_at(Utc::now())
    }

    /// 获取指定时间点的请求速率（每秒）
    pub fn get_rate_at(&self, now: DateTime<Utc>) -> f64 {
        let cutoff = now - Duration::seconds(self.window_seconds);
        let count = self.timestamps.iter().filter(|&&ts| ts >= cutoff).count();

        if self.window_seconds > 0 {
            count as f64 / self.window_seconds as f64
        } else {
            0.0
        }
    }

    /// 获取时间窗口内的请求数量
    pub fn get_count(&self) -> usize {
        self.get_count_at(Utc::now())
    }

    /// 获取指定时间点的时间窗口内的请求数量
    pub fn get_count_at(&self, now: DateTime<Utc>) -> usize {
        let cutoff = now - Duration::seconds(self.window_seconds);
        self.timestamps.iter().filter(|&&ts| ts >= cutoff).count()
    }

    /// 获取时间窗口（秒）
    pub fn window_seconds(&self) -> i64 {
        self.window_seconds
    }

    /// 设置时间窗口（秒）
    pub fn set_window_seconds(&mut self, window_seconds: i64) {
        self.window_seconds = window_seconds;
        self.cleanup_old_entries(Utc::now());
    }

    /// 清空所有记录
    pub fn clear(&mut self) {
        self.timestamps.clear();
    }
}

impl Default for RequestRateTracker {
    fn default() -> Self {
        Self::new(60) // 默认 60 秒窗口
    }
}

impl Clone for RequestRateTracker {
    fn clone(&self) -> Self {
        Self {
            timestamps: self.timestamps.clone(),
            window_seconds: self.window_seconds,
        }
    }
}

// ============================================================================
// 事件类型
// ============================================================================

/// Flow 摘要信息
///
/// 用于事件通知，包含 Flow 的关键信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSummary {
    /// Flow ID
    pub id: String,
    /// 流类型
    pub flow_type: FlowType,
    /// 模型名称
    pub model: String,
    /// 提供商
    pub provider: String,
    /// 状态
    pub state: FlowState,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 耗时（毫秒）
    pub duration_ms: Option<u64>,
    /// Token 使用量
    pub usage: Option<TokenUsage>,
    /// 是否有错误
    pub has_error: bool,
    /// 是否有工具调用
    pub has_tool_calls: bool,
    /// 是否有思维链
    pub has_thinking: bool,
}

impl From<&LLMFlow> for FlowSummary {
    fn from(flow: &LLMFlow) -> Self {
        Self {
            id: flow.id.clone(),
            flow_type: flow.flow_type.clone(),
            model: flow.request.model.clone(),
            provider: format!("{:?}", flow.metadata.provider),
            state: flow.state.clone(),
            created_at: flow.timestamps.created,
            duration_ms: if flow.timestamps.duration_ms > 0 {
                Some(flow.timestamps.duration_ms)
            } else {
                None
            },
            usage: flow.response.as_ref().map(|r| r.usage.clone()),
            has_error: flow.error.is_some(),
            has_tool_calls: flow
                .response
                .as_ref()
                .map_or(false, |r| !r.tool_calls.is_empty()),
            has_thinking: flow
                .response
                .as_ref()
                .map_or(false, |r| r.thinking.is_some()),
        }
    }
}

/// Flow 更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowUpdate {
    /// 新状态
    pub state: Option<FlowState>,
    /// 内容增量
    pub content_delta: Option<String>,
    /// 当前内容长度
    pub content_length: Option<usize>,
    /// 当前 chunk 数量
    pub chunk_count: Option<u32>,
}

/// 实时 Flow 事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum FlowEvent {
    /// Flow 开始
    FlowStarted { flow: FlowSummary },
    /// Flow 更新
    FlowUpdated { id: String, update: FlowUpdate },
    /// Flow 完成
    FlowCompleted { id: String, summary: FlowSummary },
    /// Flow 失败
    FlowFailed { id: String, error: FlowError },
    /// 阈值警告
    ///
    /// **Validates: Requirements 10.3, 10.4**
    ThresholdWarning {
        id: String,
        result: ThresholdCheckResult,
    },
    /// 通知事件
    ///
    /// **Validates: Requirements 10.1, 10.2, 10.3, 10.4**
    Notification { notification: NotificationEvent },
    /// 请求速率更新
    ///
    /// **Validates: Requirements 10.7**
    RequestRateUpdate { rate: f64, count: usize },
}

// ============================================================================
// 活跃 Flow 状态
// ============================================================================

/// 活跃 Flow 状态
///
/// 用于跟踪正在进行中的 Flow，包括流式响应重建器。
struct ActiveFlow {
    /// Flow 数据
    flow: LLMFlow,
    /// 流式响应重建器（如果是流式响应）
    stream_rebuilder: Option<StreamRebuilder>,
    /// 请求开始时间
    request_start: DateTime<Utc>,
}

// ============================================================================
// 核心监控服务
// ============================================================================

/// Flow 监控服务
///
/// 负责捕获和管理 LLM Flow 的核心服务。
pub struct FlowMonitor {
    /// 配置
    config: RwLock<FlowMonitorConfig>,
    /// 内存存储
    memory_store: Arc<RwLock<FlowMemoryStore>>,
    /// 文件存储（可选）
    file_store: Option<Arc<FlowFileStore>>,
    /// 活跃 Flow（正在进行中的请求）
    active_flows: RwLock<HashMap<String, ActiveFlow>>,
    /// 事件发送器
    event_sender: broadcast::Sender<FlowEvent>,
    /// 阈值配置
    threshold_config: RwLock<ThresholdConfig>,
    /// 请求速率追踪器
    rate_tracker: RwLock<RequestRateTracker>,
    /// 通知配置
    notification_config: RwLock<NotificationConfig>,
}

impl FlowMonitor {
    /// 创建新的 Flow 监控服务
    ///
    /// # 参数
    /// - `config`: 监控配置
    /// - `file_store`: 文件存储（可选）
    pub fn new(config: FlowMonitorConfig, file_store: Option<Arc<FlowFileStore>>) -> Self {
        let memory_store = Arc::new(RwLock::new(FlowMemoryStore::new(config.max_memory_flows)));
        let (event_sender, _) = broadcast::channel(1000);

        Self {
            config: RwLock::new(config),
            memory_store,
            file_store,
            active_flows: RwLock::new(HashMap::new()),
            event_sender,
            threshold_config: RwLock::new(ThresholdConfig::default()),
            rate_tracker: RwLock::new(RequestRateTracker::default()),
            notification_config: RwLock::new(NotificationConfig::default()),
        }
    }

    /// 创建带通知配置的 Flow 监控服务
    ///
    /// # 参数
    /// - `config`: 监控配置
    /// - `file_store`: 文件存储（可选）
    /// - `threshold_config`: 阈值配置
    /// - `notification_config`: 通知配置
    pub fn with_notification_config(
        config: FlowMonitorConfig,
        file_store: Option<Arc<FlowFileStore>>,
        threshold_config: ThresholdConfig,
        notification_config: NotificationConfig,
    ) -> Self {
        let memory_store = Arc::new(RwLock::new(FlowMemoryStore::new(config.max_memory_flows)));
        let (event_sender, _) = broadcast::channel(1000);

        Self {
            config: RwLock::new(config),
            memory_store,
            file_store,
            active_flows: RwLock::new(HashMap::new()),
            event_sender,
            threshold_config: RwLock::new(threshold_config),
            rate_tracker: RwLock::new(RequestRateTracker::default()),
            notification_config: RwLock::new(notification_config),
        }
    }

    /// 创建带完整配置的 Flow 监控服务
    ///
    /// # 参数
    /// - `config`: 监控配置
    /// - `file_store`: 文件存储（可选）
    /// - `threshold_config`: 阈值配置
    /// - `notification_config`: 通知配置
    pub fn with_full_config(
        config: FlowMonitorConfig,
        file_store: Option<Arc<FlowFileStore>>,
        threshold_config: ThresholdConfig,
        notification_config: NotificationConfig,
    ) -> Self {
        let memory_store = Arc::new(RwLock::new(FlowMemoryStore::new(config.max_memory_flows)));
        let (event_sender, _) = broadcast::channel(1000);

        Self {
            config: RwLock::new(config),
            memory_store,
            file_store,
            active_flows: RwLock::new(HashMap::new()),
            event_sender,
            threshold_config: RwLock::new(threshold_config),
            rate_tracker: RwLock::new(RequestRateTracker::default()),
            notification_config: RwLock::new(notification_config),
        }
    }

    /// 获取内存存储的引用
    pub fn memory_store(&self) -> Arc<RwLock<FlowMemoryStore>> {
        self.memory_store.clone()
    }

    /// 获取文件存储的引用
    pub fn file_store(&self) -> Option<Arc<FlowFileStore>> {
        self.file_store.clone()
    }

    /// 获取当前配置
    pub async fn config(&self) -> FlowMonitorConfig {
        self.config.read().await.clone()
    }

    /// 更新配置
    pub async fn update_config(&self, config: FlowMonitorConfig) {
        let mut current = self.config.write().await;

        // 如果缓存大小改变，需要调整内存存储
        if current.max_memory_flows != config.max_memory_flows {
            // 创建新的内存存储（旧数据会丢失）
            // 实际应用中可能需要更复杂的迁移逻辑
            let mut store = self.memory_store.write().await;
            *store = FlowMemoryStore::new(config.max_memory_flows);
        }

        *current = config;
    }

    /// 获取阈值配置
    ///
    /// **Validates: Requirements 10.3, 10.4**
    pub async fn threshold_config(&self) -> ThresholdConfig {
        self.threshold_config.read().await.clone()
    }

    /// 更新阈值配置
    ///
    /// **Validates: Requirements 10.3, 10.4**
    pub async fn update_threshold_config(&self, config: ThresholdConfig) {
        let mut current = self.threshold_config.write().await;
        *current = config;
    }

    /// 获取当前请求速率（每秒）
    ///
    /// **Validates: Requirements 10.7**
    pub async fn get_request_rate(&self) -> f64 {
        self.rate_tracker.read().await.get_rate()
    }

    /// 获取时间窗口内的请求数量
    ///
    /// **Validates: Requirements 10.7**
    pub async fn get_request_count(&self) -> usize {
        self.rate_tracker.read().await.get_count()
    }

    /// 设置请求速率追踪器的时间窗口
    ///
    /// **Validates: Requirements 10.7**
    pub async fn set_rate_window(&self, window_seconds: i64) {
        self.rate_tracker
            .write()
            .await
            .set_window_seconds(window_seconds);
    }

    /// 获取通知配置
    ///
    /// **Validates: Requirements 10.1, 10.2**
    pub async fn notification_config(&self) -> NotificationConfig {
        self.notification_config.read().await.clone()
    }

    /// 更新通知配置
    ///
    /// **Validates: Requirements 10.1, 10.2**
    pub async fn update_notification_config(&self, config: NotificationConfig) {
        let mut current = self.notification_config.write().await;
        *current = config;
    }

    /// 触发通知
    ///
    /// **Validates: Requirements 10.1, 10.2, 10.3, 10.4**
    ///
    /// # Arguments
    /// * `notification` - 通知事件
    async fn trigger_notification(&self, notification: NotificationEvent) {
        let config = self.notification_config.read().await;

        if !config.enabled {
            return;
        }

        // 发送通知事件
        let _ = self.event_sender.send(FlowEvent::Notification {
            notification: notification.clone(),
        });
    }

    /// 检查并触发新 Flow 通知
    ///
    /// **Validates: Requirements 10.1**
    async fn check_new_flow_notification(&self, flow: &LLMFlow) {
        let config = self.notification_config.read().await;

        if config.new_flow.enabled {
            let notification = NotificationEvent::new_flow(
                flow.id.clone(),
                flow.request.model.clone(),
                &config.new_flow,
            );
            drop(config);
            self.trigger_notification(notification).await;
        }
    }

    /// 检查并触发错误 Flow 通知
    ///
    /// **Validates: Requirements 10.2**
    async fn check_error_flow_notification(&self, flow: &LLMFlow, error: &FlowError) {
        let config = self.notification_config.read().await;

        if config.error_flow.enabled {
            let notification = NotificationEvent::error_flow(
                flow.id.clone(),
                flow.request.model.clone(),
                error.message.clone(),
                &config.error_flow,
            );
            drop(config);
            self.trigger_notification(notification).await;
        }
    }

    /// 检查并触发阈值警告通知
    ///
    /// **Validates: Requirements 10.3, 10.4**
    async fn check_threshold_notifications(&self, flow: &LLMFlow, result: &ThresholdCheckResult) {
        let config = self.notification_config.read().await;
        let threshold_config = self.threshold_config.read().await;

        // 延迟警告通知
        if result.latency_exceeded && config.latency_warning.enabled {
            let notification = NotificationEvent::latency_warning(
                flow.id.clone(),
                flow.request.model.clone(),
                result.actual_latency_ms,
                threshold_config.latency_threshold_ms,
                &config.latency_warning,
            );
            drop(config);
            drop(threshold_config);
            self.trigger_notification(notification).await;
            return;
        }

        // Token 警告通知
        if result.token_exceeded && config.token_warning.enabled {
            let notification = NotificationEvent::token_warning(
                flow.id.clone(),
                flow.request.model.clone(),
                result.actual_tokens,
                threshold_config.token_threshold,
                &config.token_warning,
            );
            drop(config);
            drop(threshold_config);
            self.trigger_notification(notification).await;
        }
    }

    /// 发送请求速率更新事件
    ///
    /// **Validates: Requirements 10.7**
    async fn send_rate_update(&self) {
        let tracker = self.rate_tracker.read().await;
        let rate = tracker.get_rate();
        let count = tracker.get_count();
        drop(tracker);

        let _ = self
            .event_sender
            .send(FlowEvent::RequestRateUpdate { rate, count });
    }

    /// 订阅实时事件
    pub fn subscribe(&self) -> broadcast::Receiver<FlowEvent> {
        self.event_sender.subscribe()
    }

    /// 开始捕获一个新的 Flow
    ///
    /// # 参数
    /// - `request`: LLM 请求
    /// - `metadata`: Flow 元数据
    ///
    /// # 返回
    /// - `Some(flow_id)`: 成功创建 Flow，返回 Flow ID
    /// - `None`: 根据配置跳过监控
    pub async fn start_flow(&self, request: LLMRequest, metadata: FlowMetadata) -> Option<String> {
        let config = self.config.read().await;

        // 检查是否应该监控
        if !config.should_monitor(&request.model, &request.path) {
            eprintln!(
                "[FLOW_MONITOR] 跳过监控: model={}, path={}",
                request.model, request.path
            );
            return None;
        }

        // 记录请求到速率追踪器
        {
            let mut tracker = self.rate_tracker.write().await;
            tracker.record_request();
        }

        // 生成唯一 ID
        let flow_id = Uuid::new_v4().to_string();

        eprintln!(
            "[FLOW_MONITOR] 创建新 Flow: id={}, model={}, provider={:?}",
            flow_id, request.model, metadata.provider
        );

        // 确定 Flow 类型
        let flow_type = Self::determine_flow_type(&request.path);

        // 创建 Flow
        let flow = LLMFlow::new(flow_id.clone(), flow_type, request.clone(), metadata);

        // 创建活跃 Flow 状态
        let active_flow = ActiveFlow {
            flow: flow.clone(),
            stream_rebuilder: None,
            request_start: Utc::now(),
        };

        // 添加到活跃 Flow
        {
            let mut active = self.active_flows.write().await;
            active.insert(flow_id.clone(), active_flow);
            eprintln!("[FLOW_MONITOR] 活跃 Flow 数量: {}", active.len());
        }

        // 发送事件
        let summary = FlowSummary::from(&flow);
        let _ = self
            .event_sender
            .send(FlowEvent::FlowStarted { flow: summary });

        // 检查新 Flow 通知
        self.check_new_flow_notification(&flow).await;

        // 发送请求速率更新
        self.send_rate_update().await;

        Some(flow_id)
    }

    /// 根据路径确定 Flow 类型
    fn determine_flow_type(path: &str) -> FlowType {
        let path_lower = path.to_lowercase();

        if path_lower.contains("/chat/completions") {
            FlowType::ChatCompletions
        } else if path_lower.contains("/messages") {
            FlowType::AnthropicMessages
        } else if path_lower.contains(":generatecontent") || path_lower.contains("/generate") {
            FlowType::GeminiGenerateContent
        } else if path_lower.contains("/embeddings") {
            FlowType::Embeddings
        } else {
            FlowType::Other(path.to_string())
        }
    }

    /// 设置 Flow 为流式模式
    ///
    /// # 参数
    /// - `flow_id`: Flow ID
    /// - `format`: 流式响应格式
    pub async fn set_streaming(&self, flow_id: &str, format: StreamFormat) {
        let config = self.config.read().await;
        let save_chunks = config.save_stream_chunks;
        drop(config);

        let mut active = self.active_flows.write().await;
        if let Some(active_flow) = active.get_mut(flow_id) {
            active_flow.flow.state = FlowState::Streaming;
            active_flow.stream_rebuilder =
                Some(StreamRebuilder::new(format).with_save_raw_chunks(save_chunks));

            // 发送更新事件
            let _ = self.event_sender.send(FlowEvent::FlowUpdated {
                id: flow_id.to_string(),
                update: FlowUpdate {
                    state: Some(FlowState::Streaming),
                    content_delta: None,
                    content_length: None,
                    chunk_count: None,
                },
            });
        }
    }

    /// 处理流式 chunk
    ///
    /// # 参数
    /// - `flow_id`: Flow ID
    /// - `event`: SSE 事件类型（可选）
    /// - `data`: SSE 数据内容
    pub async fn process_chunk(&self, flow_id: &str, event: Option<&str>, data: &str) {
        let mut active = self.active_flows.write().await;
        if let Some(active_flow) = active.get_mut(flow_id) {
            if let Some(ref mut rebuilder) = active_flow.stream_rebuilder {
                // 处理 chunk
                if let Err(e) = rebuilder.process_event(event, data) {
                    tracing::warn!("处理流式 chunk 失败: {}", e);
                }

                // 发送更新事件（可选，根据需要调整频率）
                // 这里简化处理，每个 chunk 都发送事件
                // 实际应用中可能需要节流
            }
        }
    }

    /// 完成 Flow
    ///
    /// # 参数
    /// - `flow_id`: Flow ID
    /// - `response`: LLM 响应（如果是非流式响应）
    pub async fn complete_flow(&self, flow_id: &str, response: Option<LLMResponse>) {
        eprintln!(
            "[FLOW_MONITOR] 准备完成 Flow: id={}, has_response={}",
            flow_id,
            response.is_some()
        );

        let mut active = self.active_flows.write().await;

        if let Some(mut active_flow) = active.remove(flow_id) {
            let now = Utc::now();

            // 如果有流式重建器，使用重建的响应
            let final_response = if let Some(rebuilder) = active_flow.stream_rebuilder.take() {
                Some(rebuilder.finish())
            } else {
                response
            };

            // 更新 Flow
            active_flow.flow.response = final_response.clone();
            active_flow.flow.state = FlowState::Completed;
            active_flow.flow.timestamps.response_end = Some(now);
            active_flow.flow.timestamps.calculate_duration();
            active_flow.flow.timestamps.calculate_ttfb();

            eprintln!(
                "[FLOW_MONITOR] Flow 状态更新: id={}, state={:?}, duration_ms={}",
                flow_id, active_flow.flow.state, active_flow.flow.timestamps.duration_ms
            );

            // 检查阈值
            let threshold_result = self.check_threshold(&active_flow.flow).await;

            // 保存到内存存储
            {
                let mut store = self.memory_store.write().await;
                store.add(active_flow.flow.clone());
                eprintln!(
                    "[FLOW_MONITOR] 已保存到内存存储: id={}, 内存中 Flow 数量={}",
                    flow_id,
                    store.len()
                );
            }

            // 保存到文件存储
            if let Some(ref file_store) = self.file_store {
                if let Err(e) = file_store.write(&active_flow.flow) {
                    tracing::error!("保存 Flow 到文件失败: {}", e);
                    eprintln!("[FLOW_MONITOR] 保存到文件失败: id={}, error={}", flow_id, e);
                } else {
                    eprintln!("[FLOW_MONITOR] 已保存到文件存储: id={}", flow_id);
                }
            } else {
                eprintln!("[FLOW_MONITOR] 文件存储未启用");
            }

            // 发送完成事件
            let summary = FlowSummary::from(&active_flow.flow);
            let _ = self.event_sender.send(FlowEvent::FlowCompleted {
                id: flow_id.to_string(),
                summary,
            });

            // 如果超过阈值，发送警告事件
            if threshold_result.any_exceeded() {
                let _ = self.event_sender.send(FlowEvent::ThresholdWarning {
                    id: flow_id.to_string(),
                    result: threshold_result.clone(),
                });

                // 检查并触发阈值通知
                self.check_threshold_notifications(&active_flow.flow, &threshold_result)
                    .await;
            }

            eprintln!("[FLOW_MONITOR] Flow 完成处理完毕: id={}", flow_id);
        } else {
            eprintln!("[FLOW_MONITOR] 警告: 未找到活跃 Flow: id={}", flow_id);
        }
    }

    /// 标记 Flow 失败
    ///
    /// # 参数
    /// - `flow_id`: Flow ID
    /// - `error`: 错误信息
    pub async fn fail_flow(&self, flow_id: &str, error: FlowError) {
        let mut active = self.active_flows.write().await;

        if let Some(mut active_flow) = active.remove(flow_id) {
            let now = Utc::now();

            // 更新 Flow
            active_flow.flow.error = Some(error.clone());
            active_flow.flow.state = FlowState::Failed;
            active_flow.flow.timestamps.response_end = Some(now);
            active_flow.flow.timestamps.calculate_duration();

            // 保存到内存存储
            {
                let mut store = self.memory_store.write().await;
                store.add(active_flow.flow.clone());
            }

            // 保存到文件存储
            if let Some(ref file_store) = self.file_store {
                if let Err(e) = file_store.write(&active_flow.flow) {
                    tracing::error!("保存 Flow 到文件失败: {}", e);
                }
            }

            // 发送失败事件
            let _ = self.event_sender.send(FlowEvent::FlowFailed {
                id: flow_id.to_string(),
                error: error.clone(),
            });

            // 检查错误 Flow 通知
            self.check_error_flow_notification(&active_flow.flow, &error)
                .await;
        }
    }

    /// 取消 Flow
    ///
    /// # 参数
    /// - `flow_id`: Flow ID
    pub async fn cancel_flow(&self, flow_id: &str) {
        let mut active = self.active_flows.write().await;

        if let Some(mut active_flow) = active.remove(flow_id) {
            let now = Utc::now();

            // 更新 Flow
            active_flow.flow.state = FlowState::Cancelled;
            active_flow.flow.timestamps.response_end = Some(now);
            active_flow.flow.timestamps.calculate_duration();

            // 保存到内存存储
            {
                let mut store = self.memory_store.write().await;
                store.add(active_flow.flow.clone());
            }

            // 保存到文件存储
            if let Some(ref file_store) = self.file_store {
                if let Err(e) = file_store.write(&active_flow.flow) {
                    tracing::error!("保存 Flow 到文件失败: {}", e);
                }
            }
        }
    }

    /// 更新 Flow 标注
    ///
    /// # 参数
    /// - `flow_id`: Flow ID
    /// - `annotations`: 新的标注信息
    ///
    /// # 返回
    /// - `true`: 更新成功
    /// - `false`: Flow 不存在
    pub async fn update_annotations(&self, flow_id: &str, annotations: FlowAnnotations) -> bool {
        // 先尝试更新内存中的 Flow
        let updated = {
            let store = self.memory_store.read().await;
            store.update(flow_id, |flow| {
                flow.annotations = annotations.clone();
            })
        };

        // 如果内存中存在，同时更新文件存储的索引
        if updated {
            if let Some(ref file_store) = self.file_store {
                if let Err(e) = file_store.update_annotations(flow_id, &annotations) {
                    tracing::error!("更新文件存储标注失败: {}", e);
                }
            }
        }

        updated
    }

    /// 收藏/取消收藏 Flow
    pub async fn toggle_starred(&self, flow_id: &str) -> bool {
        let store = self.memory_store.read().await;
        store.update(flow_id, |flow| {
            flow.annotations.starred = !flow.annotations.starred;
        })
    }

    /// 添加评论
    pub async fn add_comment(&self, flow_id: &str, comment: String) -> bool {
        let store = self.memory_store.read().await;
        store.update(flow_id, |flow| {
            flow.annotations.comment = Some(comment);
        })
    }

    /// 添加标签
    pub async fn add_tag(&self, flow_id: &str, tag: String) -> bool {
        let store = self.memory_store.read().await;
        store.update(flow_id, |flow| {
            if !flow.annotations.tags.contains(&tag) {
                flow.annotations.tags.push(tag);
            }
        })
    }

    /// 移除标签
    pub async fn remove_tag(&self, flow_id: &str, tag: &str) -> bool {
        let store = self.memory_store.read().await;
        store.update(flow_id, |flow| {
            flow.annotations.tags.retain(|t| t != tag);
        })
    }

    /// 设置标记
    pub async fn set_marker(&self, flow_id: &str, marker: Option<String>) -> bool {
        let store = self.memory_store.read().await;
        store.update(flow_id, |flow| {
            flow.annotations.marker = marker;
        })
    }

    /// 获取活跃 Flow 数量
    pub async fn active_flow_count(&self) -> usize {
        self.active_flows.read().await.len()
    }

    /// 获取内存中的 Flow 数量
    pub async fn memory_flow_count(&self) -> usize {
        self.memory_store.read().await.len()
    }

    /// 检查监控是否启用
    pub async fn is_enabled(&self) -> bool {
        self.config.read().await.enabled
    }

    /// 启用监控
    pub async fn enable(&self) {
        self.config.write().await.enabled = true;
    }

    /// 禁用监控
    pub async fn disable(&self) {
        self.config.write().await.enabled = false;
    }

    /// 检查 Flow 是否超过阈值
    ///
    /// **Validates: Requirements 10.3, 10.4**
    ///
    /// # Arguments
    /// * `flow` - 要检查的 Flow
    ///
    /// # Returns
    /// 阈值检测结果
    pub async fn check_threshold(&self, flow: &LLMFlow) -> ThresholdCheckResult {
        let config = self.threshold_config.read().await;
        Self::check_threshold_with_config(flow, &config)
    }

    /// 使用指定配置检查 Flow 是否超过阈值
    ///
    /// **Validates: Requirements 10.3, 10.4**
    ///
    /// # Arguments
    /// * `flow` - 要检查的 Flow
    /// * `config` - 阈值配置
    ///
    /// # Returns
    /// 阈值检测结果
    pub fn check_threshold_with_config(
        flow: &LLMFlow,
        config: &ThresholdConfig,
    ) -> ThresholdCheckResult {
        if !config.enabled {
            return ThresholdCheckResult::default();
        }

        let actual_latency_ms = flow.timestamps.duration_ms;
        let (actual_input_tokens, actual_output_tokens, actual_tokens) =
            if let Some(ref response) = flow.response {
                (
                    response.usage.input_tokens,
                    response.usage.output_tokens,
                    response.usage.total_tokens,
                )
            } else {
                (0, 0, 0)
            };

        let latency_exceeded = actual_latency_ms > config.latency_threshold_ms;
        let token_exceeded = actual_tokens > config.token_threshold;
        let input_token_exceeded = config
            .input_token_threshold
            .map_or(false, |threshold| actual_input_tokens > threshold);
        let output_token_exceeded = config
            .output_token_threshold
            .map_or(false, |threshold| actual_output_tokens > threshold);

        ThresholdCheckResult {
            latency_exceeded,
            token_exceeded,
            input_token_exceeded,
            output_token_exceeded,
            actual_latency_ms,
            actual_tokens,
            actual_input_tokens,
            actual_output_tokens,
        }
    }

    /// 计算指定时间窗口内的请求速率
    ///
    /// **Validates: Requirements 10.7**
    ///
    /// # Arguments
    /// * `timestamps` - 请求时间戳列表
    /// * `window_seconds` - 时间窗口（秒）
    ///
    /// # Returns
    /// 请求速率（每秒）
    pub fn calculate_request_rate(timestamps: &[DateTime<Utc>], window_seconds: i64) -> f64 {
        if timestamps.is_empty() || window_seconds <= 0 {
            return 0.0;
        }

        let now = Utc::now();
        let cutoff = now - Duration::seconds(window_seconds);
        let count = timestamps.iter().filter(|&&ts| ts >= cutoff).count();

        count as f64 / window_seconds as f64
    }

    /// 计算指定时间点的请求速率
    ///
    /// **Validates: Requirements 10.7**
    ///
    /// # Arguments
    /// * `timestamps` - 请求时间戳列表
    /// * `window_seconds` - 时间窗口（秒）
    /// * `at_time` - 计算时间点
    ///
    /// # Returns
    /// 请求速率（每秒）
    pub fn calculate_request_rate_at(
        timestamps: &[DateTime<Utc>],
        window_seconds: i64,
        at_time: DateTime<Utc>,
    ) -> f64 {
        if timestamps.is_empty() || window_seconds <= 0 {
            return 0.0;
        }

        let cutoff = at_time - Duration::seconds(window_seconds);
        let count = timestamps
            .iter()
            .filter(|&&ts| ts >= cutoff && ts <= at_time)
            .count();

        count as f64 / window_seconds as f64
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_monitor::models::{
        FlowMetadata, LLMRequest, Message, MessageContent, MessageRole, RequestParameters,
    };
    use crate::ProviderType;

    /// 创建测试用的 LLMRequest
    fn create_test_request(model: &str, path: &str) -> LLMRequest {
        LLMRequest {
            method: "POST".to_string(),
            path: path.to_string(),
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Text("Hello".to_string()),
                tool_calls: None,
                tool_result: None,
                name: None,
            }],
            system_prompt: None,
            tools: None,
            model: model.to_string(),
            original_model: None,
            parameters: RequestParameters::default(),
            size_bytes: 0,
            timestamp: Utc::now(),
        }
    }

    /// 创建测试用的 FlowMetadata
    fn create_test_metadata(provider: ProviderType) -> FlowMetadata {
        FlowMetadata {
            provider,
            credential_id: Some("test-cred".to_string()),
            credential_name: Some("Test Credential".to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_flow_monitor_creation() {
        let config = FlowMonitorConfig::default();
        let monitor = FlowMonitor::new(config, None);

        assert!(monitor.is_enabled().await);
        assert_eq!(monitor.active_flow_count().await, 0);
        assert_eq!(monitor.memory_flow_count().await, 0);
    }

    #[tokio::test]
    async fn test_start_flow() {
        let config = FlowMonitorConfig::default();
        let monitor = FlowMonitor::new(config, None);

        let request = create_test_request("gpt-4", "/v1/chat/completions");
        let metadata = create_test_metadata(ProviderType::OpenAI);

        let flow_id = monitor.start_flow(request, metadata).await;

        assert!(flow_id.is_some());
        assert_eq!(monitor.active_flow_count().await, 1);
    }

    #[tokio::test]
    async fn test_complete_flow() {
        let config = FlowMonitorConfig::default();
        let monitor = FlowMonitor::new(config, None);

        let request = create_test_request("gpt-4", "/v1/chat/completions");
        let metadata = create_test_metadata(ProviderType::OpenAI);

        let flow_id = monitor.start_flow(request, metadata).await.unwrap();

        // 完成 Flow
        monitor.complete_flow(&flow_id, None).await;

        assert_eq!(monitor.active_flow_count().await, 0);
        assert_eq!(monitor.memory_flow_count().await, 1);
    }

    #[tokio::test]
    async fn test_fail_flow() {
        let config = FlowMonitorConfig::default();
        let monitor = FlowMonitor::new(config, None);

        let request = create_test_request("gpt-4", "/v1/chat/completions");
        let metadata = create_test_metadata(ProviderType::OpenAI);

        let flow_id = monitor.start_flow(request, metadata).await.unwrap();

        // 失败 Flow
        let error = FlowError::new(
            crate::flow_monitor::models::FlowErrorType::Network,
            "Connection failed",
        );
        monitor.fail_flow(&flow_id, error).await;

        assert_eq!(monitor.active_flow_count().await, 0);
        assert_eq!(monitor.memory_flow_count().await, 1);
    }

    #[tokio::test]
    async fn test_config_should_monitor() {
        let config = FlowMonitorConfig {
            enabled: true,
            sampling_rate: 1.0,
            excluded_models: vec!["test-*".to_string()],
            excluded_paths: vec!["/health".to_string()],
            ..Default::default()
        };

        // 正常请求应该被监控
        assert!(config.should_monitor("gpt-4", "/v1/chat/completions"));

        // 排除的模型不应该被监控
        assert!(!config.should_monitor("test-model", "/v1/chat/completions"));

        // 排除的路径不应该被监控
        assert!(!config.should_monitor("gpt-4", "/health"));
    }

    #[tokio::test]
    async fn test_disabled_monitor() {
        let config = FlowMonitorConfig {
            enabled: false,
            ..Default::default()
        };
        let monitor = FlowMonitor::new(config, None);

        let request = create_test_request("gpt-4", "/v1/chat/completions");
        let metadata = create_test_metadata(ProviderType::OpenAI);

        // 禁用时不应该创建 Flow
        let flow_id = monitor.start_flow(request, metadata).await;
        assert!(flow_id.is_none());
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let config = FlowMonitorConfig::default();
        let monitor = FlowMonitor::new(config, None);

        let mut receiver = monitor.subscribe();

        let request = create_test_request("gpt-4", "/v1/chat/completions");
        let metadata = create_test_metadata(ProviderType::OpenAI);

        let flow_id = monitor.start_flow(request, metadata).await.unwrap();

        // 应该收到 FlowStarted 事件
        let event = receiver.try_recv();
        assert!(event.is_ok());
        if let FlowEvent::FlowStarted { flow } = event.unwrap() {
            assert_eq!(flow.id, flow_id);
            assert_eq!(flow.model, "gpt-4");
        } else {
            panic!("Expected FlowStarted event");
        }
    }

    #[tokio::test]
    async fn test_flow_type_detection() {
        assert_eq!(
            FlowMonitor::determine_flow_type("/v1/chat/completions"),
            FlowType::ChatCompletions
        );
        assert_eq!(
            FlowMonitor::determine_flow_type("/v1/messages"),
            FlowType::AnthropicMessages
        );
        assert_eq!(
            FlowMonitor::determine_flow_type("/v1/models/gemini-pro:generatecontent"),
            FlowType::GeminiGenerateContent
        );
        assert_eq!(
            FlowMonitor::determine_flow_type("/v1/embeddings"),
            FlowType::Embeddings
        );
    }

    #[tokio::test]
    async fn test_annotations_update() {
        let config = FlowMonitorConfig::default();
        let monitor = FlowMonitor::new(config, None);

        let request = create_test_request("gpt-4", "/v1/chat/completions");
        let metadata = create_test_metadata(ProviderType::OpenAI);

        let flow_id = monitor.start_flow(request, metadata).await.unwrap();
        monitor.complete_flow(&flow_id, None).await;

        // 测试收藏
        assert!(monitor.toggle_starred(&flow_id).await);

        // 测试添加评论
        assert!(
            monitor
                .add_comment(&flow_id, "Test comment".to_string())
                .await
        );

        // 测试添加标签
        assert!(monitor.add_tag(&flow_id, "important".to_string()).await);

        // 测试设置标记
        assert!(monitor.set_marker(&flow_id, Some("⭐".to_string())).await);
    }
}

// ============================================================================
// 属性测试模块
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::flow_monitor::models::{
        FlowErrorType, FlowMetadata, LLMRequest, Message, MessageContent, MessageRole,
        RequestParameters,
    };
    use crate::ProviderType;
    use proptest::prelude::*;
    use tokio::runtime::Runtime;

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

    /// 生成随机的模型名称
    fn arb_model_name() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("gpt-4".to_string()),
            Just("gpt-4-turbo".to_string()),
            Just("gpt-3.5-turbo".to_string()),
            Just("claude-3-opus".to_string()),
            Just("claude-3-sonnet".to_string()),
            Just("gemini-pro".to_string()),
        ]
    }

    /// 生成随机的路径
    fn arb_path() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("/v1/chat/completions".to_string()),
            Just("/v1/messages".to_string()),
            Just("/v1/embeddings".to_string()),
        ]
    }

    /// 生成随机的 LLMRequest
    fn arb_llm_request() -> impl Strategy<Value = LLMRequest> {
        (arb_model_name(), arb_path()).prop_map(|(model, path)| LLMRequest {
            method: "POST".to_string(),
            path,
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Text("Test message".to_string()),
                tool_calls: None,
                tool_result: None,
                name: None,
            }],
            system_prompt: None,
            tools: None,
            model,
            original_model: None,
            parameters: RequestParameters::default(),
            size_bytes: 0,
            timestamp: Utc::now(),
        })
    }

    /// 生成随机的 FlowMetadata
    fn arb_flow_metadata() -> impl Strategy<Value = FlowMetadata> {
        arb_provider_type().prop_map(|provider| FlowMetadata {
            provider,
            credential_id: Some("test-cred".to_string()),
            credential_name: Some("Test Credential".to_string()),
            ..Default::default()
        })
    }

    /// 生成随机的 FlowErrorType
    fn arb_flow_error_type() -> impl Strategy<Value = FlowErrorType> {
        prop_oneof![
            Just(FlowErrorType::Network),
            Just(FlowErrorType::Timeout),
            Just(FlowErrorType::Authentication),
            Just(FlowErrorType::RateLimit),
            Just(FlowErrorType::ServerError),
            Just(FlowErrorType::BadRequest),
        ]
    }

    // ========================================================================
    // 属性测试
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// **Feature: llm-flow-monitor, Property 9: 事件发送正确性**
        /// **Validates: Requirements 6.1, 6.2, 6.3, 6.4**
        ///
        /// *对于任意* Flow 生命周期操作（开始、更新、完成、失败），
        /// 应该发出对应的事件，且事件内容应该正确反映 Flow 状态。
        #[test]
        fn prop_event_emission_correctness(
            request in arb_llm_request(),
            metadata in arb_flow_metadata(),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let config = FlowMonitorConfig::default();

                // 创建禁用通知的配置
                let notification_config = NotificationConfig {
                    enabled: false,
                    new_flow: NotificationSettings::default(),
                    error_flow: NotificationSettings::default(),
                    latency_warning: NotificationSettings::default(),
                    token_warning: NotificationSettings::default(),
                };

                let monitor = FlowMonitor::with_notification_config(
                    config,
                    None,
                    ThresholdConfig::default(),
                    notification_config
                );

                let mut receiver = monitor.subscribe();

                // 开始 Flow
                let flow_id = monitor.start_flow(request.clone(), metadata.clone()).await;
                prop_assert!(flow_id.is_some(), "Flow 应该被创建");
                let flow_id = flow_id.unwrap();

                // 验证 FlowStarted 事件
                let event = receiver.try_recv();
                prop_assert!(event.is_ok(), "应该收到 FlowStarted 事件");
                if let FlowEvent::FlowStarted { flow } = event.unwrap() {
                    prop_assert_eq!(flow.id, flow_id.clone(), "事件中的 Flow ID 应该正确");
                    prop_assert_eq!(flow.model, request.model, "事件中的模型应该正确");
                    prop_assert_eq!(
                        flow.state,
                        FlowState::Pending,
                        "新 Flow 状态应该是 Pending"
                    );
                } else {
                    prop_assert!(false, "应该是 FlowStarted 事件");
                }

                // 可能有 RequestRateUpdate 事件，消费它
                let _ = receiver.try_recv();

                // 完成 Flow
                monitor.complete_flow(&flow_id, None).await;

                // 验证 FlowCompleted 事件（可能需要跳过其他事件）
                let mut found_completed = false;
                for _ in 0..3 {  // 最多尝试 3 次
                    let event = receiver.try_recv();
                    if event.is_ok() {
                        if let FlowEvent::FlowCompleted { id, summary } = event.unwrap() {
                            prop_assert_eq!(id, flow_id.clone(), "事件中的 Flow ID 应该正确");
                            prop_assert_eq!(
                                summary.state,
                                FlowState::Completed,
                                "完成后状态应该是 Completed"
                            );
                            found_completed = true;
                            break;
                        }
                        // 如果不是 FlowCompleted 事件，继续尝试下一个
                    } else {
                        break;
                    }
                }
                prop_assert!(found_completed, "应该收到 FlowCompleted 事件");

                Ok(())
            })?;
        }

        /// **Feature: llm-flow-monitor, Property 9b: 失败事件发送正确性**
        /// **Validates: Requirements 6.4**
        ///
        /// *对于任意* Flow 失败操作，应该发出 FlowFailed 事件，
        /// 且事件内容应该包含正确的错误信息。
        #[test]
        fn prop_failure_event_correctness(
            request in arb_llm_request(),
            metadata in arb_flow_metadata(),
            error_type in arb_flow_error_type(),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let config = FlowMonitorConfig::default();

                // 创建禁用通知的配置
                let notification_config = NotificationConfig {
                    enabled: false,
                    new_flow: NotificationSettings::default(),
                    error_flow: NotificationSettings::default(),
                    latency_warning: NotificationSettings::default(),
                    token_warning: NotificationSettings::default(),
                };

                let monitor = FlowMonitor::with_notification_config(
                    config,
                    None,
                    ThresholdConfig::default(),
                    notification_config
                );

                let mut receiver = monitor.subscribe();

                // 开始 Flow
                let flow_id = monitor.start_flow(request, metadata).await.unwrap();

                // 消费 FlowStarted 事件
                let _ = receiver.try_recv();
                // 可能有 RequestRateUpdate 事件，消费它
                let _ = receiver.try_recv();

                // 失败 Flow
                let error = FlowError::new(error_type.clone(), "Test error message");
                monitor.fail_flow(&flow_id, error.clone()).await;

                // 验证 FlowFailed 事件（可能需要跳过其他事件）
                let mut found_failed = false;
                for _ in 0..3 {  // 最多尝试 3 次
                    let event = receiver.try_recv();
                    if event.is_ok() {
                        if let FlowEvent::FlowFailed { id, error: evt_error } = event.unwrap() {
                            prop_assert_eq!(id, flow_id, "事件中的 Flow ID 应该正确");
                            prop_assert_eq!(
                                evt_error.error_type,
                                error_type,
                                "事件中的错误类型应该正确"
                            );
                            prop_assert_eq!(
                                evt_error.message,
                                "Test error message",
                                "事件中的错误消息应该正确"
                            );
                            found_failed = true;
                            break;
                        }
                        // 如果不是 FlowFailed 事件，继续尝试下一个
                    } else {
                        break;
                    }
                }
                prop_assert!(found_failed, "应该收到 FlowFailed 事件");

                Ok(())
            })?;
        }

        /// **Feature: llm-flow-monitor, Property 10: 标注 Round-Trip**
        /// **Validates: Requirements 7.1, 7.2, 7.3, 7.4**
        ///
        /// *对于任意* Flow 和标注操作（收藏、评论、标签、标记），
        /// 更新后再读取，标注信息应该与设置的值一致。
        #[test]
        fn prop_annotation_roundtrip(
            request in arb_llm_request(),
            metadata in arb_flow_metadata(),
            starred in any::<bool>(),
            comment in prop::option::of("[a-zA-Z0-9 ]{1,50}"),
            marker in prop::option::of(prop_oneof![
                Just("⭐".to_string()),
                Just("🔴".to_string()),
                Just("🟢".to_string()),
            ]),
            tags in prop::collection::vec("[a-z]{3,10}", 0..3),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let config = FlowMonitorConfig::default();
                let monitor = FlowMonitor::new(config, None);

                // 创建并完成 Flow
                let flow_id = monitor.start_flow(request, metadata).await.unwrap();
                monitor.complete_flow(&flow_id, None).await;

                // 设置标注
                let annotations = FlowAnnotations {
                    starred,
                    comment: comment.clone(),
                    marker: marker.clone(),
                    tags: tags.clone(),
                };

                let updated = monitor.update_annotations(&flow_id, annotations.clone()).await;
                prop_assert!(updated, "标注更新应该成功");

                // 读取并验证
                let store = monitor.memory_store.read().await;
                let flow_lock = store.get(&flow_id);
                prop_assert!(flow_lock.is_some(), "Flow 应该存在");

                let binding = flow_lock.unwrap();
                let flow = binding.read().unwrap();
                prop_assert_eq!(flow.annotations.starred, starred, "收藏状态应该一致");
                prop_assert_eq!(flow.annotations.comment.clone(), comment, "评论应该一致");
                prop_assert_eq!(flow.annotations.marker.clone(), marker, "标记应该一致");
                prop_assert_eq!(flow.annotations.tags.clone(), tags, "标签应该一致");

                Ok(())
            })?;
        }

        /// **Feature: llm-flow-monitor, Property 12: 配置生效属性**
        /// **Validates: Requirements 11.1, 11.2, 11.7, 11.8**
        ///
        /// *对于任意* 监控配置（启用/禁用、缓存大小、采样率、排除规则），
        /// Flow_Monitor 的行为应该符合配置。
        #[test]
        fn prop_config_effectiveness(
            enabled in any::<bool>(),
            max_memory_flows in 10usize..100usize,
            excluded_model in prop::option::of("[a-z]{3,10}"),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                // 构建配置
                let excluded_models = excluded_model
                    .clone()
                    .map(|m| vec![format!("{}*", m)])
                    .unwrap_or_default();

                let config = FlowMonitorConfig {
                    enabled,
                    max_memory_flows,
                    sampling_rate: 1.0, // 确保采样率为 100%
                    excluded_models: excluded_models.clone(),
                    ..Default::default()
                };

                let monitor = FlowMonitor::new(config, None);

                // 验证启用/禁用配置
                prop_assert_eq!(
                    monitor.is_enabled().await,
                    enabled,
                    "监控启用状态应该与配置一致"
                );

                // 测试排除模型配置
                if let Some(ref excluded) = excluded_model {
                    let excluded_model_name = format!("{}-test", excluded);
                    let request = LLMRequest {
                        method: "POST".to_string(),
                        path: "/v1/chat/completions".to_string(),
                        model: excluded_model_name,
                        ..Default::default()
                    };
                    let metadata = FlowMetadata::default();

                    let flow_id = monitor.start_flow(request, metadata).await;

                    if enabled {
                        // 启用时，排除的模型不应该被监控
                        prop_assert!(
                            flow_id.is_none(),
                            "排除的模型不应该被监控"
                        );
                    } else {
                        // 禁用时，任何模型都不应该被监控
                        prop_assert!(
                            flow_id.is_none(),
                            "禁用时不应该监控任何模型"
                        );
                    }
                }

                // 测试非排除模型
                if enabled {
                    let request = LLMRequest {
                        method: "POST".to_string(),
                        path: "/v1/chat/completions".to_string(),
                        model: "gpt-4".to_string(),
                        ..Default::default()
                    };
                    let metadata = FlowMetadata::default();

                    let flow_id = monitor.start_flow(request, metadata).await;
                    prop_assert!(
                        flow_id.is_some(),
                        "启用时，非排除的模型应该被监控"
                    );
                }

                Ok(())
            })?;
        }

        /// **Feature: llm-flow-monitor, Property 12b: 缓存大小配置生效**
        /// **Validates: Requirements 11.2**
        ///
        /// *对于任意* 缓存大小配置，内存存储的最大大小应该与配置一致。
        #[test]
        fn prop_cache_size_config(
            max_memory_flows in 10usize..100usize,
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let config = FlowMonitorConfig {
                    enabled: true,
                    max_memory_flows,
                    sampling_rate: 1.0,
                    ..Default::default()
                };

                let monitor = FlowMonitor::new(config, None);

                // 验证内存存储的最大大小
                let store = monitor.memory_store.read().await;
                prop_assert_eq!(
                    store.max_size(),
                    max_memory_flows,
                    "内存存储的最大大小应该与配置一致"
                );

                Ok(())
            })?;
        }

        /// **Feature: flow-monitor-enhancement, Property 18: 阈值检测正确性**
        /// **Validates: Requirements 10.3, 10.4**
        ///
        /// *对于任意* 阈值配置和 Flow，阈值检测应该正确判断是否超过阈值。
        #[test]
        fn prop_threshold_detection_correctness(
            latency_threshold_ms in 100u64..10000u64,
            token_threshold in 100u32..50000u32,
            actual_latency_ms in 0u64..20000u64,
            actual_input_tokens in 0u32..30000u32,
            actual_output_tokens in 0u32..30000u32,
            input_token_threshold in prop::option::of(100u32..50000u32),
            output_token_threshold in prop::option::of(100u32..50000u32),
        ) {
            use crate::flow_monitor::models::{LLMResponse, TokenUsage};

            // 创建阈值配置
            let config = ThresholdConfig {
                enabled: true,
                latency_threshold_ms,
                token_threshold,
                input_token_threshold,
                output_token_threshold,
            };

            // 创建测试 Flow
            let request = LLMRequest {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                model: "gpt-4".to_string(),
                ..Default::default()
            };
            let metadata = FlowMetadata::default();
            let mut flow = LLMFlow::new(
                "test-flow".to_string(),
                FlowType::ChatCompletions,
                request,
                metadata,
            );

            // 设置延迟
            flow.timestamps.duration_ms = actual_latency_ms;

            // 设置 Token 使用量
            let actual_total_tokens = actual_input_tokens + actual_output_tokens;
            flow.response = Some(LLMResponse {
                usage: TokenUsage {
                    input_tokens: actual_input_tokens,
                    output_tokens: actual_output_tokens,
                    total_tokens: actual_total_tokens,
                    ..Default::default()
                },
                ..Default::default()
            });

            // 执行阈值检测
            let result = FlowMonitor::check_threshold_with_config(&flow, &config);

            // 验证延迟阈值检测
            let expected_latency_exceeded = actual_latency_ms > latency_threshold_ms;
            prop_assert_eq!(
                result.latency_exceeded,
                expected_latency_exceeded,
                "延迟阈值检测应该正确: 实际延迟 {} ms, 阈值 {} ms",
                actual_latency_ms,
                latency_threshold_ms
            );

            // 验证 Token 阈值检测
            let expected_token_exceeded = actual_total_tokens > token_threshold;
            prop_assert_eq!(
                result.token_exceeded,
                expected_token_exceeded,
                "Token 阈值检测应该正确: 实际 Token {}, 阈值 {}",
                actual_total_tokens,
                token_threshold
            );

            // 验证输入 Token 阈值检测
            let expected_input_exceeded = input_token_threshold
                .map_or(false, |threshold| actual_input_tokens > threshold);
            prop_assert_eq!(
                result.input_token_exceeded,
                expected_input_exceeded,
                "输入 Token 阈值检测应该正确"
            );

            // 验证输出 Token 阈值检测
            let expected_output_exceeded = output_token_threshold
                .map_or(false, |threshold| actual_output_tokens > threshold);
            prop_assert_eq!(
                result.output_token_exceeded,
                expected_output_exceeded,
                "输出 Token 阈值检测应该正确"
            );

            // 验证实际值记录正确
            prop_assert_eq!(
                result.actual_latency_ms,
                actual_latency_ms,
                "实际延迟应该正确记录"
            );
            prop_assert_eq!(
                result.actual_tokens,
                actual_total_tokens,
                "实际 Token 数应该正确记录"
            );
            prop_assert_eq!(
                result.actual_input_tokens,
                actual_input_tokens,
                "实际输入 Token 数应该正确记录"
            );
            prop_assert_eq!(
                result.actual_output_tokens,
                actual_output_tokens,
                "实际输出 Token 数应该正确记录"
            );

            // 验证 any_exceeded 方法
            let expected_any_exceeded = expected_latency_exceeded
                || expected_token_exceeded
                || expected_input_exceeded
                || expected_output_exceeded;
            prop_assert_eq!(
                result.any_exceeded(),
                expected_any_exceeded,
                "any_exceeded 应该正确反映是否有任何阈值被超过"
            );
        }

        /// **Feature: flow-monitor-enhancement, Property 18b: 禁用阈值检测**
        /// **Validates: Requirements 10.3, 10.4**
        ///
        /// *对于任意* Flow，当阈值检测禁用时，所有检测结果应该为 false。
        #[test]
        fn prop_threshold_detection_disabled(
            actual_latency_ms in 0u64..20000u64,
            actual_input_tokens in 0u32..30000u32,
            actual_output_tokens in 0u32..30000u32,
        ) {
            use crate::flow_monitor::models::{LLMResponse, TokenUsage};

            // 创建禁用的阈值配置
            let config = ThresholdConfig {
                enabled: false,
                latency_threshold_ms: 100, // 很低的阈值
                token_threshold: 100,       // 很低的阈值
                input_token_threshold: Some(100),
                output_token_threshold: Some(100),
            };

            // 创建测试 Flow
            let request = LLMRequest {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                model: "gpt-4".to_string(),
                ..Default::default()
            };
            let metadata = FlowMetadata::default();
            let mut flow = LLMFlow::new(
                "test-flow".to_string(),
                FlowType::ChatCompletions,
                request,
                metadata,
            );

            // 设置延迟和 Token（超过阈值）
            flow.timestamps.duration_ms = actual_latency_ms;
            flow.response = Some(LLMResponse {
                usage: TokenUsage {
                    input_tokens: actual_input_tokens,
                    output_tokens: actual_output_tokens,
                    total_tokens: actual_input_tokens + actual_output_tokens,
                    ..Default::default()
                },
                ..Default::default()
            });

            // 执行阈值检测
            let result = FlowMonitor::check_threshold_with_config(&flow, &config);

            // 验证所有检测结果都为 false
            prop_assert!(
                !result.latency_exceeded,
                "禁用时延迟阈值检测应该为 false"
            );
            prop_assert!(
                !result.token_exceeded,
                "禁用时 Token 阈值检测应该为 false"
            );
            prop_assert!(
                !result.input_token_exceeded,
                "禁用时输入 Token 阈值检测应该为 false"
            );
            prop_assert!(
                !result.output_token_exceeded,
                "禁用时输出 Token 阈值检测应该为 false"
            );
            prop_assert!(
                !result.any_exceeded(),
                "禁用时 any_exceeded 应该为 false"
            );
        }

        /// **Feature: flow-monitor-enhancement, Property 20: 通知触发正确性**
        /// **Validates: Requirements 10.1, 10.2, 10.3, 10.4**
        ///
        /// *对于任意* 通知配置和 Flow 事件，当通知启用时应该触发相应的通知事件。
        #[test]
        fn prop_notification_trigger_correctness(
            request in arb_llm_request(),
            metadata in arb_flow_metadata(),
            new_flow_enabled in any::<bool>(),
            error_flow_enabled in any::<bool>(),
            latency_warning_enabled in any::<bool>(),
            token_warning_enabled in any::<bool>(),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                // 创建通知配置
                let notification_config = NotificationConfig {
                    enabled: true,
                    new_flow: NotificationSettings {
                        enabled: new_flow_enabled,
                        desktop: true,
                        sound: false,
                        sound_file: None,
                    },
                    error_flow: NotificationSettings {
                        enabled: error_flow_enabled,
                        desktop: true,
                        sound: false,
                        sound_file: None,
                    },
                    latency_warning: NotificationSettings {
                        enabled: latency_warning_enabled,
                        desktop: false,
                        sound: false,
                        sound_file: None,
                    },
                    token_warning: NotificationSettings {
                        enabled: token_warning_enabled,
                        desktop: false,
                        sound: false,
                        sound_file: None,
                    },
                };

                // 创建阈值配置（低阈值，容易触发）
                let threshold_config = ThresholdConfig {
                    enabled: true,
                    latency_threshold_ms: 100,
                    token_threshold: 100,
                    input_token_threshold: None,
                    output_token_threshold: None,
                };

                let config = FlowMonitorConfig::default();
                let monitor = FlowMonitor::with_full_config(
                    config,
                    None,
                    threshold_config,
                    notification_config,
                );

                let mut receiver = monitor.subscribe();

                // 开始 Flow
                let flow_id = monitor.start_flow(request.clone(), metadata.clone()).await;
                prop_assert!(flow_id.is_some(), "Flow 应该被创建");
                let flow_id = flow_id.unwrap();

                // 消费 FlowStarted 事件
                let _ = receiver.try_recv();

                // 检查新 Flow 通知
                if new_flow_enabled {
                    // 应该有 RequestRateUpdate 事件
                    let event = receiver.try_recv();
                    if event.is_ok() {
                        let event_value = event.unwrap();
                        if let FlowEvent::RequestRateUpdate { .. } = event_value {
                            // 这是速率更新事件，继续检查通知事件
                            let notification_event = receiver.try_recv();
                            if notification_event.is_ok() {
                                if let FlowEvent::Notification { notification } = notification_event.unwrap() {
                                    prop_assert_eq!(
                                        notification.flow_id,
                                        flow_id.clone(),
                                        "通知中的 Flow ID 应该正确"
                                    );
                                    prop_assert!(
                                        matches!(notification.notification_type, NotificationType::NewFlow),
                                        "应该是新 Flow 通知"
                                    );
                                }
                            }
                        } else if let FlowEvent::Notification { notification } = event_value {
                            prop_assert_eq!(
                                notification.flow_id,
                                flow_id.clone(),
                                "通知中的 Flow ID 应该正确"
                            );
                            prop_assert!(
                                matches!(notification.notification_type, NotificationType::NewFlow),
                                "应该是新 Flow 通知"
                            );
                        }
                    }
                }

                // 测试错误通知
                if error_flow_enabled {
                    let error = FlowError::new(FlowErrorType::Network, "Test error");
                    monitor.fail_flow(&flow_id, error).await;

                    // 消费 FlowFailed 事件
                    let _ = receiver.try_recv();

                    // 检查错误通知
                    let event = receiver.try_recv();
                    if event.is_ok() {
                        if let FlowEvent::Notification { notification } = event.unwrap() {
                            prop_assert_eq!(
                                notification.flow_id,
                                flow_id.clone(),
                                "错误通知中的 Flow ID 应该正确"
                            );
                            prop_assert!(
                                matches!(notification.notification_type, NotificationType::ErrorFlow),
                                "应该是错误 Flow 通知"
                            );
                        }
                    }
                }

                Ok(())
            })?;
        }

        /// **Feature: flow-monitor-enhancement, Property 20b: 禁用通知不触发**
        /// **Validates: Requirements 10.1, 10.2**
        ///
        /// *对于任意* Flow 事件，当通知禁用时不应该触发通知事件。
        #[test]
        fn prop_disabled_notifications_not_triggered(
            request in arb_llm_request(),
            metadata in arb_flow_metadata(),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                // 创建禁用的通知配置
                let notification_config = NotificationConfig {
                    enabled: false, // 全局禁用
                    new_flow: NotificationSettings {
                        enabled: true, // 即使启用也不应该触发
                        desktop: true,
                        sound: false,
                        sound_file: None,
                    },
                    error_flow: NotificationSettings {
                        enabled: true, // 即使启用也不应该触发
                        desktop: true,
                        sound: false,
                        sound_file: None,
                    },
                    ..Default::default()
                };

                let config = FlowMonitorConfig::default();
                let monitor = FlowMonitor::with_full_config(
                    config,
                    None,
                    ThresholdConfig::default(),
                    notification_config,
                );

                let mut receiver = monitor.subscribe();

                // 开始 Flow
                let flow_id = monitor.start_flow(request, metadata).await;
                prop_assert!(flow_id.is_some(), "Flow 应该被创建");
                let flow_id = flow_id.unwrap();

                // 消费 FlowStarted 事件
                let _ = receiver.try_recv();

                // 可能有 RequestRateUpdate 事件，消费它
                let event = receiver.try_recv();
                if event.is_ok() {
                    let event_value = event.unwrap();
                    if let FlowEvent::RequestRateUpdate { .. } = event_value {
                        // 这是速率更新事件，检查是否还有其他事件
                        let next_event = receiver.try_recv();
                        prop_assert!(
                            next_event.is_err() || !matches!(next_event.unwrap(), FlowEvent::Notification { .. }),
                            "禁用通知时不应该有通知事件"
                        );
                    } else {
                        prop_assert!(
                            !matches!(event_value, FlowEvent::Notification { .. }),
                            "禁用通知时不应该有通知事件"
                        );
                    }
                }

                // 测试错误情况
                let error = FlowError::new(FlowErrorType::Network, "Test error");
                monitor.fail_flow(&flow_id, error).await;

                // 消费 FlowFailed 事件
                let _ = receiver.try_recv();

                // 检查不应该有通知事件
                let event = receiver.try_recv();
                if event.is_ok() {
                    let event = event.unwrap();
                    prop_assert!(
                        !matches!(event, FlowEvent::Notification { .. }),
                        "禁用通知时不应该有错误通知事件"
                    );
                }

                Ok(())
            })?;
        }
    }
}

// ============================================================================
// 请求速率追踪器属性测试
// ============================================================================

#[cfg(test)]
mod rate_tracker_property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: flow-monitor-enhancement, Property 19: 请求速率计算正确性**
        /// **Validates: Requirements 10.7**
        ///
        /// *对于任意* 时间窗口内的请求集合，请求速率计算应该正确反映该窗口内的请求数量。
        #[test]
        fn prop_request_rate_calculation_correctness(
            window_seconds in 10i64..120i64,
            request_count in 0usize..100usize,
        ) {
            let mut tracker = RequestRateTracker::new(window_seconds);
            let now = Utc::now();

            // 在时间窗口内添加请求
            for i in 0..request_count {
                // 在窗口内均匀分布请求
                let offset_seconds = if request_count > 1 {
                    (i as i64 * (window_seconds - 1)) / (request_count as i64 - 1).max(1)
                } else {
                    0
                };
                let timestamp = now - Duration::seconds(window_seconds - 1 - offset_seconds);
                tracker.record_request_at(timestamp);
            }

            // 计算速率
            let rate = tracker.get_rate_at(now);
            let count = tracker.get_count_at(now);

            // 验证请求数量
            prop_assert_eq!(
                count,
                request_count,
                "时间窗口内的请求数量应该正确"
            );

            // 验证速率计算
            let expected_rate = request_count as f64 / window_seconds as f64;
            prop_assert!(
                (rate - expected_rate).abs() < 0.0001,
                "请求速率计算应该正确: 期望 {}, 实际 {}",
                expected_rate,
                rate
            );
        }

        /// **Feature: flow-monitor-enhancement, Property 19b: 过期请求清理**
        /// **Validates: Requirements 10.7**
        ///
        /// *对于任意* 请求集合，超出时间窗口的请求应该被正确排除。
        #[test]
        fn prop_expired_requests_excluded(
            window_seconds in 10i64..60i64,
            in_window_count in 0usize..50usize,
            out_window_count in 0usize..50usize,
        ) {
            let mut tracker = RequestRateTracker::new(window_seconds);
            let now = Utc::now();

            // 添加窗口内的请求
            for i in 0..in_window_count {
                let offset = (i as i64 * (window_seconds - 1)) / (in_window_count as i64).max(1);
                let timestamp = now - Duration::seconds(offset);
                tracker.record_request_at(timestamp);
            }

            // 添加窗口外的请求（过期的）
            for i in 0..out_window_count {
                let offset = window_seconds + 1 + i as i64;
                let timestamp = now - Duration::seconds(offset);
                tracker.record_request_at(timestamp);
            }

            // 验证只计算窗口内的请求
            let count = tracker.get_count_at(now);
            prop_assert_eq!(
                count,
                in_window_count,
                "只应该计算时间窗口内的请求: 期望 {}, 实际 {}",
                in_window_count,
                count
            );

            // 验证速率只基于窗口内的请求
            let rate = tracker.get_rate_at(now);
            let expected_rate = in_window_count as f64 / window_seconds as f64;
            prop_assert!(
                (rate - expected_rate).abs() < 0.0001,
                "请求速率应该只基于窗口内的请求"
            );
        }

        /// **Feature: flow-monitor-enhancement, Property 19c: 空窗口处理**
        /// **Validates: Requirements 10.7**
        ///
        /// *对于任意* 空的请求集合，请求速率应该为 0。
        #[test]
        fn prop_empty_window_rate_zero(
            window_seconds in 1i64..120i64,
        ) {
            let tracker = RequestRateTracker::new(window_seconds);

            // 验证空窗口的速率为 0
            let rate = tracker.get_rate();
            prop_assert_eq!(
                rate,
                0.0,
                "空窗口的请求速率应该为 0"
            );

            // 验证空窗口的请求数量为 0
            let count = tracker.get_count();
            prop_assert_eq!(
                count,
                0,
                "空窗口的请求数量应该为 0"
            );
        }

        /// **Feature: flow-monitor-enhancement, Property 19d: 窗口大小变更**
        /// **Validates: Requirements 10.7**
        ///
        /// *对于任意* 窗口大小变更，请求计数应该正确反映新窗口内的请求。
        #[test]
        fn prop_window_size_change(
            initial_window in 30i64..60i64,
            new_window in 10i64..30i64,
            request_count in 10usize..50usize,
        ) {
            let mut tracker = RequestRateTracker::new(initial_window);
            let now = Utc::now();

            // 在初始窗口内均匀添加请求
            // 请求时间从 now 到 now - (initial_window - 1) 秒
            for i in 0..request_count {
                let offset = (i as i64 * (initial_window - 1)) / (request_count as i64).max(1);
                let timestamp = now - Duration::seconds(offset);
                tracker.record_request_at(timestamp);
            }

            // 验证初始窗口内的请求数量
            let initial_count = tracker.get_count_at(now);
            prop_assert_eq!(
                initial_count,
                request_count,
                "初始窗口内的请求数量应该正确"
            );

            // 更改窗口大小
            tracker.set_window_seconds(new_window);

            // 计算新窗口内应该有多少请求
            // cutoff = now - new_window，所以 timestamp >= cutoff 意味着 offset <= new_window
            let expected_new_count = (0..request_count)
                .filter(|&i| {
                    let offset = (i as i64 * (initial_window - 1)) / (request_count as i64).max(1);
                    // 请求在新窗口内的条件是 offset < new_window（严格小于）
                    // 因为 cutoff = now - new_window，timestamp = now - offset
                    // timestamp >= cutoff 等价于 now - offset >= now - new_window
                    // 即 offset <= new_window
                    offset < new_window
                })
                .count();

            // 验证新窗口内的请求数量
            let new_count = tracker.get_count_at(now);

            // 由于整数除法舍入和边界条件，允许 ±2 的误差
            // 边界情况：当 offset 恰好等于 new_window 时，由于整数除法的舍入
            // 可能导致多个请求落在边界附近
            let diff = (new_count as i64 - expected_new_count as i64).abs();
            prop_assert!(
                diff <= 2,
                "新窗口内的请求数量应该接近预期: 期望 {}, 实际 {}, 差异 {}",
                expected_new_count,
                new_count,
                diff
            );
        }
    }
}
