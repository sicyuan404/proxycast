//! 会话粘性配置
//!
//! 提供调度模式配置，用于控制账号选择策略。

use serde::{Deserialize, Serialize};

/// 调度模式枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulingMode {
    /// 缓存优先 (Cache-first): 尽可能锁定同一账号，限流时优先等待，极大提升 Prompt Caching 命中率
    CacheFirst,
    /// 平衡模式 (Balance): 锁定同一账号，限流时立即切换到备选账号，兼顾成功率和性能
    Balance,
    /// 性能优先 (Performance-first): 纯轮询模式 (Round-robin)，账号负载最均衡，但不利用缓存
    PerformanceFirst,
}

impl Default for SchedulingMode {
    fn default() -> Self {
        Self::Balance
    }
}

impl std::fmt::Display for SchedulingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CacheFirst => write!(f, "CacheFirst"),
            Self::Balance => write!(f, "Balance"),
            Self::PerformanceFirst => write!(f, "PerformanceFirst"),
        }
    }
}

/// 粘性会话配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickySessionConfig {
    /// 当前调度模式
    pub mode: SchedulingMode,
    /// 缓存优先模式下的最大等待时间 (秒)
    pub max_wait_seconds: u64,
    /// 60 秒全局锁定窗口（用于无 session_id 情况的默认保护）
    pub global_lock_window_seconds: u64,
}

impl Default for StickySessionConfig {
    fn default() -> Self {
        Self {
            mode: SchedulingMode::Balance,
            max_wait_seconds: 60,
            global_lock_window_seconds: 60,
        }
    }
}

impl StickySessionConfig {
    /// 创建缓存优先配置
    pub fn cache_first() -> Self {
        Self {
            mode: SchedulingMode::CacheFirst,
            max_wait_seconds: 120,
            global_lock_window_seconds: 60,
        }
    }

    /// 创建性能优先配置
    pub fn performance_first() -> Self {
        Self {
            mode: SchedulingMode::PerformanceFirst,
            max_wait_seconds: 0,
            global_lock_window_seconds: 0,
        }
    }

    /// 是否启用会话粘性
    pub fn is_sticky_enabled(&self) -> bool {
        self.mode != SchedulingMode::PerformanceFirst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = StickySessionConfig::default();
        assert_eq!(config.mode, SchedulingMode::Balance);
        assert_eq!(config.max_wait_seconds, 60);
        assert!(config.is_sticky_enabled());
    }

    #[test]
    fn test_cache_first_config() {
        let config = StickySessionConfig::cache_first();
        assert_eq!(config.mode, SchedulingMode::CacheFirst);
        assert!(config.is_sticky_enabled());
    }

    #[test]
    fn test_performance_first_config() {
        let config = StickySessionConfig::performance_first();
        assert_eq!(config.mode, SchedulingMode::PerformanceFirst);
        assert!(!config.is_sticky_enabled());
    }
}
