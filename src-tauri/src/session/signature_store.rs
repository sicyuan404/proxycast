//! thoughtSignature 全局存储
//!
//! 用于在流式响应中捕获 thoughtSignature，并在后续请求中注入。
//! 这对于 Gemini 3 Pro 的 Tool Use 功能至关重要。

use std::sync::RwLock;

/// 最小有效签名长度
const MIN_SIGNATURE_LENGTH: usize = 50;

/// 全局 thoughtSignature 存储
static THOUGHT_SIGNATURE: RwLock<Option<String>> = RwLock::new(None);

/// 存储 thoughtSignature 到全局存储
///
/// 只有当新签名长度大于等于最小长度时才会存储。
/// 如果已有签名，只有当新签名更长时才会替换。
///
/// # 参数
/// - `sig`: 要存储的签名
pub fn store_thought_signature(sig: &str) {
    if sig.len() < MIN_SIGNATURE_LENGTH {
        tracing::debug!(
            "[SignatureStore] Ignoring short signature (length: {} < {})",
            sig.len(),
            MIN_SIGNATURE_LENGTH
        );
        return;
    }

    let mut store = THOUGHT_SIGNATURE.write().unwrap();

    // 只有当新签名更长时才替换
    let should_replace = match &*store {
        Some(existing) => sig.len() > existing.len(),
        None => true,
    };

    if should_replace {
        tracing::debug!(
            "[SignatureStore] Storing thought_signature (length: {})",
            sig.len()
        );
        *store = Some(sig.to_string());
    }
}

/// 获取存储的 thoughtSignature（不清除）
///
/// # 返回
/// 存储的签名，如果没有则返回 None
pub fn get_thought_signature() -> Option<String> {
    let store = THOUGHT_SIGNATURE.read().unwrap();
    store.clone()
}

/// 获取并清除存储的 thoughtSignature
///
/// # 返回
/// 存储的签名，如果没有则返回 None
pub fn take_thought_signature() -> Option<String> {
    let mut store = THOUGHT_SIGNATURE.write().unwrap();
    store.take()
}

/// 清除存储的 thoughtSignature
pub fn clear_thought_signature() {
    let mut store = THOUGHT_SIGNATURE.write().unwrap();
    *store = None;
    tracing::debug!("[SignatureStore] Cleared thought_signature");
}

/// 检查是否有有效的 thoughtSignature
pub fn has_valid_signature() -> bool {
    let store = THOUGHT_SIGNATURE.read().unwrap();
    store
        .as_ref()
        .map(|s| s.len() >= MIN_SIGNATURE_LENGTH)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_store() {
        // 清除之前的状态
        clear_thought_signature();

        // 初始状态应该为空
        assert!(get_thought_signature().is_none());

        // 存储短签名应该被忽略
        store_thought_signature("short");
        assert!(get_thought_signature().is_none());

        // 存储有效签名
        let valid_sig = "a".repeat(MIN_SIGNATURE_LENGTH);
        store_thought_signature(&valid_sig);
        assert_eq!(get_thought_signature(), Some(valid_sig.clone()));

        // 存储更长的签名应该替换
        let longer_sig = "b".repeat(MIN_SIGNATURE_LENGTH + 10);
        store_thought_signature(&longer_sig);
        assert_eq!(get_thought_signature(), Some(longer_sig.clone()));

        // 存储更短的签名不应该替换
        store_thought_signature(&valid_sig);
        assert_eq!(get_thought_signature(), Some(longer_sig.clone()));

        // take 应该返回并清除
        let taken = take_thought_signature();
        assert_eq!(taken, Some(longer_sig));
        assert!(get_thought_signature().is_none());
    }
}
