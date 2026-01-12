//! 会话管理模块
//!
//! 提供以下功能：
//! - 稳定的 SessionId 生成（基于请求内容哈希）
//! - thoughtSignature 全局缓存
//! - 会话粘性管理（会话与账号映射）
//! - 调度模式配置
//! - 增强的限流处理（Duration 解析、指数退避）

mod rate_limit;
mod session_manager;
mod signature_store;
mod sticky_config;
mod sticky_manager;

pub use rate_limit::{
    extract_retry_delay, parse_duration_string, RateLimitReason, RateLimitRecord, RateLimitTracker,
};
pub use session_manager::SessionManager;
pub use signature_store::{
    clear_thought_signature, get_thought_signature, has_valid_signature, store_thought_signature,
    take_thought_signature,
};
pub use sticky_config::{SchedulingMode, StickySessionConfig};
pub use sticky_manager::{AccountInfo, StickySessionManager};
