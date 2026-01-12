//! 增强的限流处理模块
//!
//! 提供以下功能：
//! - Duration 字符串解析（如 "1.5s", "1h16m0.667s"）
//! - 指数退避策略
//! - 账号级别和模型级别限流
//! - 连续失败计数

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

/// 限流原因类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitReason {
    /// 配额耗尽
    QuotaExhausted,
    /// 速率限制
    RateLimitExceeded,
    /// 模型容量耗尽
    ModelCapacityExhausted,
    /// 服务器错误 (5xx)
    ServerError,
    /// 未知原因
    Unknown,
}

impl std::fmt::Display for RateLimitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QuotaExhausted => write!(f, "QuotaExhausted"),
            Self::RateLimitExceeded => write!(f, "RateLimitExceeded"),
            Self::ModelCapacityExhausted => write!(f, "ModelCapacityExhausted"),
            Self::ServerError => write!(f, "ServerError"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// 限流记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRecord {
    /// 账号/凭证 ID
    pub account_id: String,
    /// 限流原因
    pub reason: RateLimitReason,
    /// 限流开始时间
    pub started_at: DateTime<Utc>,
    /// 限流结束时间（预计）
    pub reset_at: DateTime<Utc>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 模型名称（如果是模型级别限流）
    pub model: Option<String>,
}

/// 限流追踪器
#[derive(Debug)]
pub struct RateLimitTracker {
    /// 账号级别限流记录
    account_limits: DashMap<String, RateLimitRecord>,
    /// 模型级别限流记录 (account_id:model -> record)
    model_limits: DashMap<String, RateLimitRecord>,
    /// 连续失败计数 (account_id -> count)
    failure_counts: DashMap<String, AtomicU32>,
    /// 基础退避时间（秒）
    base_backoff_seconds: u64,
    /// 最大退避时间（秒）
    max_backoff_seconds: u64,
}

impl Default for RateLimitTracker {
    fn default() -> Self {
        Self::new(5, 300) // 默认 5 秒基础退避，最大 5 分钟
    }
}

impl RateLimitTracker {
    /// 创建新的限流追踪器
    pub fn new(base_backoff_seconds: u64, max_backoff_seconds: u64) -> Self {
        Self {
            account_limits: DashMap::new(),
            model_limits: DashMap::new(),
            failure_counts: DashMap::new(),
            base_backoff_seconds,
            max_backoff_seconds,
        }
    }

    /// 标记账号限流
    pub fn mark_rate_limited(
        &self,
        account_id: &str,
        reason: RateLimitReason,
        retry_after: Option<Duration>,
        model: Option<&str>,
    ) -> RateLimitRecord {
        let now = Utc::now();

        // 增加连续失败计数
        let failures = self
            .failure_counts
            .entry(account_id.to_string())
            .or_insert_with(|| AtomicU32::new(0));
        let failure_count = failures.fetch_add(1, Ordering::SeqCst) + 1;

        // 计算退避时间
        let backoff = if let Some(retry) = retry_after {
            retry
        } else {
            self.calculate_exponential_backoff(failure_count)
        };

        let reset_at = now + backoff;

        let record = RateLimitRecord {
            account_id: account_id.to_string(),
            reason,
            started_at: now,
            reset_at,
            consecutive_failures: failure_count,
            model: model.map(|s| s.to_string()),
        };

        // 根据是否有模型信息决定存储位置
        if let Some(m) = model {
            let key = format!("{}:{}", account_id, m);
            self.model_limits.insert(key, record.clone());
        } else {
            self.account_limits
                .insert(account_id.to_string(), record.clone());
        }

        tracing::warn!(
            account_id = %account_id,
            reason = %reason,
            reset_at = %reset_at,
            failures = failure_count,
            model = ?model,
            "账号被限流"
        );

        record
    }

    /// 计算指数退避时间
    fn calculate_exponential_backoff(&self, failure_count: u32) -> Duration {
        // 指数退避: base * 2^(failures-1)，但不超过最大值
        let exponent = (failure_count - 1).min(10); // 防止溢出
        let backoff_secs = self.base_backoff_seconds * (1 << exponent);
        let capped_secs = backoff_secs.min(self.max_backoff_seconds);
        Duration::seconds(capped_secs as i64)
    }

    /// 检查账号是否被限流
    pub fn is_rate_limited(&self, account_id: &str) -> bool {
        self.get_remaining_wait(account_id) > 0
    }

    /// 检查特定模型是否被限流
    pub fn is_model_rate_limited(&self, account_id: &str, model: &str) -> bool {
        let key = format!("{}:{}", account_id, model);
        if let Some(record) = self.model_limits.get(&key) {
            return Utc::now() < record.reset_at;
        }
        false
    }

    /// 获取剩余等待时间（秒）
    pub fn get_remaining_wait(&self, account_id: &str) -> i64 {
        if let Some(record) = self.account_limits.get(account_id) {
            let remaining = (record.reset_at - Utc::now()).num_seconds();
            if remaining > 0 {
                return remaining;
            }
        }
        0
    }

    /// 获取模型的剩余等待时间（秒）
    pub fn get_model_remaining_wait(&self, account_id: &str, model: &str) -> i64 {
        let key = format!("{}:{}", account_id, model);
        if let Some(record) = self.model_limits.get(&key) {
            let remaining = (record.reset_at - Utc::now()).num_seconds();
            if remaining > 0 {
                return remaining;
            }
        }
        0
    }

    /// 清除账号的限流状态（成功请求后调用）
    pub fn clear_rate_limit(&self, account_id: &str) {
        self.account_limits.remove(account_id);
        // 重置连续失败计数
        if let Some(counter) = self.failure_counts.get(account_id) {
            counter.store(0, Ordering::SeqCst);
        }
    }

    /// 清除模型的限流状态
    pub fn clear_model_rate_limit(&self, account_id: &str, model: &str) {
        let key = format!("{}:{}", account_id, model);
        self.model_limits.remove(&key);
    }

    /// 清理过期的限流记录
    pub fn cleanup_expired(&self) {
        let now = Utc::now();

        // 清理账号级别限流
        self.account_limits
            .retain(|_, record| record.reset_at > now);

        // 清理模型级别限流
        self.model_limits.retain(|_, record| record.reset_at > now);
    }

    /// 获取所有被限流的账号
    pub fn get_rate_limited_accounts(&self) -> Vec<String> {
        let now = Utc::now();
        self.account_limits
            .iter()
            .filter(|entry| entry.value().reset_at > now)
            .map(|entry| entry.key().clone())
            .collect()
    }
}

/// 解析 Duration 字符串
///
/// 支持格式：
/// - "1.5s" -> 1.5 秒
/// - "1h16m0.667s" -> 1 小时 16 分钟 0.667 秒
/// - "30m" -> 30 分钟
/// - "2h" -> 2 小时
///
/// # 参数
/// - `s`: Duration 字符串
///
/// # 返回
/// 解析后的 Duration，如果解析失败返回 None
pub fn parse_duration_string(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let mut total_millis: i64 = 0;
    let mut current_num = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '.' {
            current_num.push(c);
        } else {
            if current_num.is_empty() {
                continue;
            }

            let num: f64 = current_num.parse().ok()?;
            current_num.clear();

            match c {
                'h' => total_millis += (num * 3600.0 * 1000.0) as i64,
                'm' => {
                    // 检查是否是 "ms"
                    if chars.peek() == Some(&'s') {
                        chars.next();
                        total_millis += num as i64;
                    } else {
                        total_millis += (num * 60.0 * 1000.0) as i64;
                    }
                }
                's' => total_millis += (num * 1000.0) as i64,
                _ => return None,
            }
        }
    }

    // 处理末尾没有单位的数字（默认为秒）
    if !current_num.is_empty() {
        let num: f64 = current_num.parse().ok()?;
        total_millis += (num * 1000.0) as i64;
    }

    if total_millis > 0 {
        Some(Duration::milliseconds(total_millis))
    } else {
        None
    }
}

/// 从 429 响应中提取重试延迟
///
/// 尝试从以下位置提取：
/// 1. Retry-After 头
/// 2. 响应体中的 retryDelay 字段
/// 3. 响应体中的 quotaResetDelay 字段
///
/// # 参数
/// - `headers`: HTTP 响应头
/// - `body`: 响应体 JSON
///
/// # 返回
/// 解析后的 Duration，如果无法提取返回 None
pub fn extract_retry_delay(
    headers: Option<&reqwest::header::HeaderMap>,
    body: Option<&serde_json::Value>,
) -> Option<Duration> {
    // 1. 尝试从 Retry-After 头提取
    if let Some(hdrs) = headers {
        if let Some(retry_after) = hdrs.get("retry-after").and_then(|v| v.to_str().ok()) {
            // Retry-After 可以是秒数或 HTTP 日期
            if let Ok(secs) = retry_after.parse::<i64>() {
                return Some(Duration::seconds(secs));
            }
            // 尝试解析为 Duration 字符串
            if let Some(d) = parse_duration_string(retry_after) {
                return d.into();
            }
        }
    }

    // 2. 尝试从响应体提取
    if let Some(json) = body {
        // 尝试 error.details[].retryDelay
        if let Some(details) = json
            .get("error")
            .and_then(|e| e.get("details"))
            .and_then(|d| d.as_array())
        {
            for detail in details {
                if let Some(retry_delay) = detail.get("retryDelay").and_then(|r| r.as_str()) {
                    if let Some(d) = parse_duration_string(retry_delay) {
                        return Some(d);
                    }
                }
                if let Some(quota_reset) = detail.get("quotaResetDelay").and_then(|r| r.as_str()) {
                    if let Some(d) = parse_duration_string(quota_reset) {
                        return Some(d);
                    }
                }
            }
        }

        // 尝试顶层 retryDelay
        if let Some(retry_delay) = json.get("retryDelay").and_then(|r| r.as_str()) {
            if let Some(d) = parse_duration_string(retry_delay) {
                return Some(d);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_string() {
        // 秒
        assert_eq!(
            parse_duration_string("1.5s"),
            Some(Duration::milliseconds(1500))
        );
        assert_eq!(
            parse_duration_string("30s"),
            Some(Duration::milliseconds(30000))
        );

        // 分钟
        assert_eq!(
            parse_duration_string("5m"),
            Some(Duration::milliseconds(300000))
        );

        // 小时
        assert_eq!(
            parse_duration_string("2h"),
            Some(Duration::milliseconds(7200000))
        );

        // 复合格式
        assert_eq!(
            parse_duration_string("1h16m0.667s"),
            Some(Duration::milliseconds(4560667))
        );

        // 毫秒
        assert_eq!(
            parse_duration_string("500ms"),
            Some(Duration::milliseconds(500))
        );

        // 无效输入
        assert_eq!(parse_duration_string(""), None);
        assert_eq!(parse_duration_string("invalid"), None);
    }

    #[test]
    fn test_exponential_backoff() {
        let tracker = RateLimitTracker::new(5, 300);

        // 第一次失败: 5 秒
        assert_eq!(
            tracker.calculate_exponential_backoff(1),
            Duration::seconds(5)
        );

        // 第二次失败: 10 秒
        assert_eq!(
            tracker.calculate_exponential_backoff(2),
            Duration::seconds(10)
        );

        // 第三次失败: 20 秒
        assert_eq!(
            tracker.calculate_exponential_backoff(3),
            Duration::seconds(20)
        );

        // 第七次失败: 320 秒，但被限制为 300 秒
        assert_eq!(
            tracker.calculate_exponential_backoff(7),
            Duration::seconds(300)
        );
    }

    #[test]
    fn test_rate_limit_tracker() {
        let tracker = RateLimitTracker::new(5, 300);

        // 初始状态不应该被限流
        assert!(!tracker.is_rate_limited("account1"));

        // 标记限流
        tracker.mark_rate_limited("account1", RateLimitReason::QuotaExhausted, None, None);

        // 应该被限流
        assert!(tracker.is_rate_limited("account1"));

        // 清除限流
        tracker.clear_rate_limit("account1");
        assert!(!tracker.is_rate_limited("account1"));
    }
}
