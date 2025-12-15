//! 协议选择器 - 智能选择最优协议转换路径
//!
//! 根据源协议、目标 Provider 和请求特征，选择最优的协议转换路径。

use crate::models::provider_pool_model::PoolProviderType;

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI Chat Completions API
    OpenAI,
    /// Anthropic Messages API (Claude)
    Anthropic,
    /// CodeWhisperer API (Kiro)
    CodeWhisperer,
    /// Gemini API (Google)
    Gemini,
    /// Antigravity API (Google Internal)
    Antigravity,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::OpenAI => "openai",
            Protocol::Anthropic => "anthropic",
            Protocol::CodeWhisperer => "codewhisperer",
            Protocol::Gemini => "gemini",
            Protocol::Antigravity => "antigravity",
        }
    }
}

/// 转换路径
#[derive(Debug, Clone)]
pub struct ConversionPath {
    /// 源协议
    pub source: Protocol,
    /// 目标协议
    pub target: Protocol,
    /// 是否需要转换
    pub needs_conversion: bool,
    /// 转换复杂度 (0-10, 越低越好)
    pub complexity: u8,
}

/// 协议选择器
pub struct ProtocolSelector;

impl ProtocolSelector {
    /// 获取 Provider 的原生协议
    pub fn native_protocol(provider: PoolProviderType) -> Protocol {
        match provider {
            PoolProviderType::Kiro => Protocol::CodeWhisperer,
            PoolProviderType::Gemini => Protocol::Gemini,
            PoolProviderType::Qwen => Protocol::OpenAI,
            PoolProviderType::OpenAI => Protocol::OpenAI,
            PoolProviderType::Claude => Protocol::Anthropic,
            PoolProviderType::Antigravity => Protocol::Antigravity,
        }
    }

    /// 选择最优转换路径
    pub fn select_path(
        source_protocol: Protocol,
        target_provider: PoolProviderType,
    ) -> ConversionPath {
        let target_protocol = Self::native_protocol(target_provider);

        // 如果源和目标协议相同，无需转换
        if source_protocol == target_protocol {
            return ConversionPath {
                source: source_protocol,
                target: target_protocol,
                needs_conversion: false,
                complexity: 0,
            };
        }

        // 计算转换复杂度
        let complexity = Self::calculate_complexity(source_protocol, target_protocol);

        ConversionPath {
            source: source_protocol,
            target: target_protocol,
            needs_conversion: true,
            complexity,
        }
    }

    /// 计算转换复杂度
    fn calculate_complexity(source: Protocol, target: Protocol) -> u8 {
        match (source, target) {
            // OpenAI <-> Anthropic: 中等复杂度
            (Protocol::OpenAI, Protocol::Anthropic) => 3,
            (Protocol::Anthropic, Protocol::OpenAI) => 3,

            // OpenAI <-> CodeWhisperer: 较高复杂度（需要处理历史格式）
            (Protocol::OpenAI, Protocol::CodeWhisperer) => 5,
            (Protocol::CodeWhisperer, Protocol::OpenAI) => 5,

            // OpenAI <-> Gemini/Antigravity: 中等复杂度
            (Protocol::OpenAI, Protocol::Gemini) => 4,
            (Protocol::OpenAI, Protocol::Antigravity) => 4,
            (Protocol::Gemini, Protocol::OpenAI) => 4,
            (Protocol::Antigravity, Protocol::OpenAI) => 4,

            // Anthropic <-> CodeWhisperer: 较高复杂度
            (Protocol::Anthropic, Protocol::CodeWhisperer) => 6,
            (Protocol::CodeWhisperer, Protocol::Anthropic) => 6,

            // Anthropic <-> Gemini/Antigravity: 中等复杂度
            (Protocol::Anthropic, Protocol::Gemini) => 5,
            (Protocol::Anthropic, Protocol::Antigravity) => 5,
            (Protocol::Gemini, Protocol::Anthropic) => 5,
            (Protocol::Antigravity, Protocol::Anthropic) => 5,

            // Gemini <-> Antigravity: 低复杂度（格式相似）
            (Protocol::Gemini, Protocol::Antigravity) => 1,
            (Protocol::Antigravity, Protocol::Gemini) => 1,

            // 其他情况
            _ => 7,
        }
    }

    /// 检查是否支持直接转换
    pub fn supports_direct_conversion(source: Protocol, target: Protocol) -> bool {
        matches!(
            (source, target),
            (Protocol::OpenAI, Protocol::Anthropic)
                | (Protocol::Anthropic, Protocol::OpenAI)
                | (Protocol::OpenAI, Protocol::CodeWhisperer)
                | (Protocol::CodeWhisperer, Protocol::OpenAI)
                | (Protocol::OpenAI, Protocol::Gemini)
                | (Protocol::OpenAI, Protocol::Antigravity)
                | (Protocol::Gemini, Protocol::OpenAI)
                | (Protocol::Antigravity, Protocol::OpenAI)
                | (Protocol::Gemini, Protocol::Antigravity)
                | (Protocol::Antigravity, Protocol::Gemini)
        )
    }

    /// 获取推荐的中间协议（用于不支持直接转换的情况）
    pub fn intermediate_protocol(source: Protocol, target: Protocol) -> Option<Protocol> {
        // 大多数情况下，OpenAI 是最好的中间协议
        if !Self::supports_direct_conversion(source, target) {
            if source != Protocol::OpenAI && target != Protocol::OpenAI {
                return Some(Protocol::OpenAI);
            }
        }
        None
    }

    /// 获取 Provider 支持的输入协议列表
    pub fn supported_input_protocols(_provider: PoolProviderType) -> Vec<Protocol> {
        // 所有 Provider 都支持 OpenAI 和 Anthropic 协议输入
        vec![Protocol::OpenAI, Protocol::Anthropic]
    }

    /// 检查请求是否需要特殊处理
    pub fn needs_special_handling(
        source: Protocol,
        target_provider: PoolProviderType,
        has_tools: bool,
        has_images: bool,
    ) -> bool {
        // 工具调用在某些转换中需要特殊处理
        if has_tools {
            match (source, target_provider) {
                (Protocol::OpenAI, PoolProviderType::Kiro) => true,
                (Protocol::Anthropic, PoolProviderType::Kiro) => true,
                _ => false,
            }
        } else if has_images {
            // 图片在某些 Provider 中需要特殊处理
            match target_provider {
                PoolProviderType::Kiro => true, // Kiro 不支持图片
                _ => false,
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_protocol() {
        assert_eq!(
            ProtocolSelector::native_protocol(PoolProviderType::Kiro),
            Protocol::CodeWhisperer
        );
        assert_eq!(
            ProtocolSelector::native_protocol(PoolProviderType::OpenAI),
            Protocol::OpenAI
        );
        assert_eq!(
            ProtocolSelector::native_protocol(PoolProviderType::Claude),
            Protocol::Anthropic
        );
    }

    #[test]
    fn test_select_path_no_conversion() {
        let path = ProtocolSelector::select_path(Protocol::OpenAI, PoolProviderType::OpenAI);
        assert!(!path.needs_conversion);
        assert_eq!(path.complexity, 0);
    }

    #[test]
    fn test_select_path_with_conversion() {
        let path = ProtocolSelector::select_path(Protocol::OpenAI, PoolProviderType::Kiro);
        assert!(path.needs_conversion);
        assert_eq!(path.target, Protocol::CodeWhisperer);
    }
}
