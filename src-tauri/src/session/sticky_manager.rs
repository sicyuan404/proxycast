//! 会话粘性管理器
//!
//! 实现会话与账号的映射，支持：
//! - 会话绑定到特定账号
//! - 60 秒全局锁定窗口
//! - 订阅等级排序

use super::rate_limit::RateLimitTracker;
use super::sticky_config::{SchedulingMode, StickySessionConfig};
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// 账号信息
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// 账号 ID
    pub account_id: String,
    /// 邮箱
    pub email: String,
    /// 订阅等级 (ULTRA, PRO, FREE)
    pub subscription_tier: Option<String>,
    /// 是否被禁用
    pub disabled: bool,
}

impl AccountInfo {
    /// 获取订阅等级优先级（数字越小优先级越高）
    pub fn tier_priority(&self) -> u8 {
        match self.subscription_tier.as_deref() {
            Some("ULTRA") => 0,
            Some("PRO") => 1,
            Some("FREE") => 2,
            _ => 3,
        }
    }
}

/// 会话粘性管理器
pub struct StickySessionManager {
    /// 会话与账号映射 (session_id -> account_id)
    session_accounts: DashMap<String, String>,
    /// 最后使用的账号 (account_id, timestamp)
    last_used_account: Arc<tokio::sync::Mutex<Option<(String, Instant)>>>,
    /// 当前轮询索引
    current_index: AtomicUsize,
    /// 限流追踪器
    rate_limit_tracker: Arc<RateLimitTracker>,
    /// 粘性配置
    sticky_config: Arc<RwLock<StickySessionConfig>>,
}

impl Default for StickySessionManager {
    fn default() -> Self {
        Self::new(Arc::new(RateLimitTracker::default()))
    }
}

impl StickySessionManager {
    /// 创建新的会话粘性管理器
    pub fn new(rate_limit_tracker: Arc<RateLimitTracker>) -> Self {
        Self {
            session_accounts: DashMap::new(),
            last_used_account: Arc::new(tokio::sync::Mutex::new(None)),
            current_index: AtomicUsize::new(0),
            rate_limit_tracker,
            sticky_config: Arc::new(RwLock::new(StickySessionConfig::default())),
        }
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> StickySessionConfig {
        self.sticky_config.read().await.clone()
    }

    /// 设置配置
    pub async fn set_config(&self, config: StickySessionConfig) {
        *self.sticky_config.write().await = config;
    }

    /// 绑定会话到账号
    pub fn bind_session(&self, session_id: &str, account_id: &str) {
        self.session_accounts
            .insert(session_id.to_string(), account_id.to_string());
        tracing::debug!(
            "[StickySession] 绑定会话 {} 到账号 {}",
            session_id,
            account_id
        );
    }

    /// 解绑会话
    pub fn unbind_session(&self, session_id: &str) {
        if let Some((_, account_id)) = self.session_accounts.remove(session_id) {
            tracing::debug!(
                "[StickySession] 解绑会话 {} (原账号: {})",
                session_id,
                account_id
            );
        }
    }

    /// 获取会话绑定的账号
    pub fn get_bound_account(&self, session_id: &str) -> Option<String> {
        self.session_accounts.get(session_id).map(|v| v.clone())
    }

    /// 选择账号（支持粘性会话和智能调度）
    ///
    /// # 参数
    /// - `accounts`: 可用账号列表
    /// - `session_id`: 会话 ID（可选）
    /// - `force_rotate`: 是否强制轮换
    /// - `quota_group`: 配额组（如 "claude", "gemini", "image_gen"）
    ///
    /// # 返回
    /// 选中的账号，如果没有可用账号返回 None
    pub async fn select_account(
        &self,
        accounts: &[AccountInfo],
        session_id: Option<&str>,
        force_rotate: bool,
        quota_group: &str,
    ) -> Option<AccountInfo> {
        if accounts.is_empty() {
            return None;
        }

        // 按订阅等级排序（ULTRA > PRO > FREE）
        let mut sorted_accounts = accounts.to_vec();
        sorted_accounts.sort_by_key(|a| a.tier_priority());

        let config = self.sticky_config.read().await.clone();
        let total = sorted_accounts.len();

        // 模式 A: 粘性会话处理
        if !force_rotate && session_id.is_some() && config.mode != SchedulingMode::PerformanceFirst
        {
            let sid = session_id.unwrap();

            // 检查会话是否已绑定账号
            if let Some(bound_id) = self.get_bound_account(sid) {
                // 找到绑定的账号
                if let Some(bound_account) =
                    sorted_accounts.iter().find(|a| a.account_id == bound_id)
                {
                    // 检查是否被限流
                    if !self
                        .rate_limit_tracker
                        .is_rate_limited(&bound_account.email)
                    {
                        tracing::debug!(
                            "[StickySession] 复用绑定账号 {} (会话: {})",
                            bound_account.email,
                            sid
                        );
                        return Some(bound_account.clone());
                    } else {
                        // 账号被限流，解绑并切换
                        tracing::warn!(
                            "[StickySession] 绑定账号 {} 被限流，解绑会话 {}",
                            bound_account.email,
                            sid
                        );
                        self.unbind_session(sid);
                    }
                } else {
                    // 绑定的账号不存在，解绑
                    self.unbind_session(sid);
                }
            }
        }

        // 模式 B: 60 秒全局锁定（针对无 session_id 情况）
        if !force_rotate && quota_group != "image_gen" && config.global_lock_window_seconds > 0 {
            let last_used = self.last_used_account.lock().await;
            if let Some((account_id, last_time)) = &*last_used {
                if last_time.elapsed().as_secs() < config.global_lock_window_seconds {
                    // 找到最后使用的账号
                    if let Some(account) =
                        sorted_accounts.iter().find(|a| &a.account_id == account_id)
                    {
                        if !self.rate_limit_tracker.is_rate_limited(&account.email) {
                            tracing::debug!("[StickySession] 60s 窗口内复用账号 {}", account.email);
                            return Some(account.clone());
                        }
                    }
                }
            }
            drop(last_used);
        }

        // 模式 C: 轮询选择
        let start_idx = self.current_index.fetch_add(1, Ordering::SeqCst) % total;
        for offset in 0..total {
            let idx = (start_idx + offset) % total;
            let candidate = &sorted_accounts[idx];

            // 跳过被禁用的账号
            if candidate.disabled {
                continue;
            }

            // 跳过被限流的账号
            if self.rate_limit_tracker.is_rate_limited(&candidate.email) {
                continue;
            }

            // 找到可用账号
            tracing::debug!(
                "[StickySession] 轮询选择账号 {} (索引: {})",
                candidate.email,
                idx
            );

            // 更新最后使用的账号
            {
                let mut last_used = self.last_used_account.lock().await;
                *last_used = Some((candidate.account_id.clone(), Instant::now()));
            }

            // 如果有会话 ID 且启用粘性，绑定会话
            if let Some(sid) = session_id {
                if config.mode != SchedulingMode::PerformanceFirst {
                    self.bind_session(sid, &candidate.account_id);
                }
            }

            return Some(candidate.clone());
        }

        // 没有可用账号
        tracing::warn!("[StickySession] 没有可用账号");
        None
    }

    /// 标记账号请求成功（清除限流状态）
    pub fn mark_success(&self, account_id: &str) {
        self.rate_limit_tracker.clear_rate_limit(account_id);
    }

    /// 获取限流追踪器
    pub fn rate_limit_tracker(&self) -> &Arc<RateLimitTracker> {
        &self.rate_limit_tracker
    }

    /// 清理过期的会话绑定
    pub fn cleanup_expired_sessions(&self, max_age_seconds: u64) {
        // 这里可以添加会话过期清理逻辑
        // 目前简单实现，不做过期清理
        let _ = max_age_seconds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_binding() {
        let manager = StickySessionManager::default();

        // 绑定会话
        manager.bind_session("session1", "account1");
        assert_eq!(
            manager.get_bound_account("session1"),
            Some("account1".to_string())
        );

        // 解绑会话
        manager.unbind_session("session1");
        assert_eq!(manager.get_bound_account("session1"), None);
    }

    #[tokio::test]
    async fn test_account_selection() {
        let manager = StickySessionManager::default();

        let accounts = vec![
            AccountInfo {
                account_id: "acc1".to_string(),
                email: "user1@example.com".to_string(),
                subscription_tier: Some("FREE".to_string()),
                disabled: false,
            },
            AccountInfo {
                account_id: "acc2".to_string(),
                email: "user2@example.com".to_string(),
                subscription_tier: Some("PRO".to_string()),
                disabled: false,
            },
            AccountInfo {
                account_id: "acc3".to_string(),
                email: "user3@example.com".to_string(),
                subscription_tier: Some("ULTRA".to_string()),
                disabled: false,
            },
        ];

        // 应该优先选择 ULTRA 账号
        let selected = manager
            .select_account(&accounts, None, false, "claude")
            .await;
        assert!(selected.is_some());
        assert_eq!(
            selected.unwrap().subscription_tier,
            Some("ULTRA".to_string())
        );
    }

    #[tokio::test]
    async fn test_sticky_session() {
        let manager = StickySessionManager::default();

        let accounts = vec![
            AccountInfo {
                account_id: "acc1".to_string(),
                email: "user1@example.com".to_string(),
                subscription_tier: Some("PRO".to_string()),
                disabled: false,
            },
            AccountInfo {
                account_id: "acc2".to_string(),
                email: "user2@example.com".to_string(),
                subscription_tier: Some("PRO".to_string()),
                disabled: false,
            },
        ];

        // 第一次选择，应该绑定会话
        let selected1 = manager
            .select_account(&accounts, Some("session1"), false, "claude")
            .await;
        assert!(selected1.is_some());
        let account_id = selected1.unwrap().account_id;

        // 第二次选择同一会话，应该返回相同账号
        let selected2 = manager
            .select_account(&accounts, Some("session1"), false, "claude")
            .await;
        assert!(selected2.is_some());
        assert_eq!(selected2.unwrap().account_id, account_id);
    }
}
