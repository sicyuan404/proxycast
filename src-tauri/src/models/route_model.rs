//! 路由模型
//!
//! 用于多供应商路由功能的数据结构定义。

use serde::{Deserialize, Serialize};

/// 单个路由信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    /// 路由选择器 (provider 类型或凭证名称)
    pub selector: String,
    /// Provider 类型
    pub provider_type: String,
    /// 关联的凭证数量
    pub credential_count: usize,
    /// 可用的端点列表
    pub endpoints: Vec<RouteEndpoint>,
    /// 标签 (如 "突破限制", "官方API/三方")
    pub tags: Vec<String>,
    /// 是否启用
    pub enabled: bool,
}

/// 路由端点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEndpoint {
    /// 端点路径
    pub path: String,
    /// 协议类型
    pub protocol: String, // "openai" 或 "claude"
    /// 完整 URL
    pub url: String,
}

/// 路由列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteListResponse {
    /// 服务器基础 URL
    pub base_url: String,
    /// 默认 Provider
    pub default_provider: String,
    /// 所有可用路由
    pub routes: Vec<RouteInfo>,
}

/// curl 示例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurlExample {
    /// 描述
    pub description: String,
    /// curl 命令
    pub command: String,
}

impl RouteInfo {
    /// 创建新的路由信息
    pub fn new(selector: String, provider_type: String) -> Self {
        Self {
            selector,
            provider_type,
            credential_count: 0,
            endpoints: Vec::new(),
            tags: Vec::new(),
            enabled: true,
        }
    }

    /// 添加端点
    pub fn add_endpoint(&mut self, base_url: &str, protocol: &str) {
        let path = match protocol {
            "claude" => format!("/{}/v1/messages", self.selector),
            "openai" => format!("/{}/v1/chat/completions", self.selector),
            _ => return,
        };
        let url = format!("{}{}", base_url, path);
        self.endpoints.push(RouteEndpoint {
            path,
            protocol: protocol.to_string(),
            url,
        });
    }

    /// 生成 curl 示例
    pub fn generate_curl_examples(&self, api_key: &str) -> Vec<CurlExample> {
        let mut examples = Vec::new();

        for endpoint in &self.endpoints {
            let (_model, body) = match endpoint.protocol.as_str() {
                "claude" => {
                    let model = match self.provider_type.as_str() {
                        "kiro" | "claude" => "claude-sonnet-4-5",
                        "gemini" => "gemini-2.5-flash",
                        "qwen" => "qwen3-coder-plus",
                        "openai" => "gpt-4",
                        _ => "claude-sonnet-4-5",
                    };
                    (
                        model,
                        format!(
                            r#"{{
  "model": "{}",
  "max_tokens": 1024,
  "messages": [{{"role": "user", "content": "Hello!"}}]
}}"#,
                            model
                        ),
                    )
                }
                "openai" => {
                    let model = match self.provider_type.as_str() {
                        "kiro" | "claude" => "claude-sonnet-4-5",
                        "gemini" => "gemini-2.5-flash",
                        "qwen" => "qwen3-coder-plus",
                        "openai" => "gpt-4",
                        _ => "claude-sonnet-4-5",
                    };
                    (
                        model,
                        format!(
                            r#"{{
  "model": "{}",
  "messages": [{{"role": "user", "content": "Hello!"}}]
}}"#,
                            model
                        ),
                    )
                }
                _ => continue,
            };

            let command = format!(
                r#"curl {} \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer {}" \
  -d '{}'"#,
                endpoint.url, api_key, body
            );

            examples.push(CurlExample {
                description: format!("{} 协议", endpoint.protocol.to_uppercase()),
                command,
            });
        }

        examples
    }
}
