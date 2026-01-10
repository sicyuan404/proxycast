//! Flow Monitor Enhancement 端到端功能验证测试
//!
//! 验证 Flow Monitor 的基础功能，包括：
//! - Flow 数据结构创建和操作
//! - 内存存储功能
//! - 查询服务功能
//! - 导出功能
//!
//! **Validates: Requirements 8.2**

use std::sync::Arc;
use tempfile::TempDir;

use chrono::Utc;
use proxycast_lib::flow_monitor::{
    ClientInfo, ExportFormat, ExportOptions, FlowAnnotations, FlowExporter, FlowFileStore,
    FlowFilter, FlowMetadata, FlowMonitor, FlowMonitorConfig, FlowQueryService, FlowSortBy,
    FlowState, FlowTimestamps, FlowType, LLMFlow, LLMRequest, LLMResponse, Message, MessageContent,
    MessageRole, ProviderType, RequestParameters, RotationConfig, RoutingInfo, TokenUsage,
};
use std::collections::HashMap;

/// 端到端测试上下文
struct E2ETestContext {
    pub temp_dir: TempDir,
    pub flow_monitor: Arc<FlowMonitor>,
    pub flow_query_service: Arc<FlowQueryService>,
}

impl E2ETestContext {
    /// 创建端到端测试上下文
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // 创建临时目录
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path().to_path_buf();

        // 创建 Flow 文件存储
        let flows_dir = temp_path.join("flows");
        std::fs::create_dir_all(&flows_dir)?;
        let rotation_config = RotationConfig::default();
        let flow_file_store = Arc::new(FlowFileStore::new(flows_dir, rotation_config)?);

        // 创建 Flow Monitor
        let flow_monitor_config = FlowMonitorConfig::default();
        let flow_monitor = Arc::new(FlowMonitor::new(
            flow_monitor_config,
            Some(flow_file_store.clone()),
        ));

        // 创建 Flow Query Service
        let flow_query_service = Arc::new(FlowQueryService::new(
            flow_monitor.memory_store(),
            flow_file_store,
        ));

        Ok(Self {
            temp_dir,
            flow_monitor,
            flow_query_service,
        })
    }

    /// 创建测试用的 Flow
    pub fn create_test_flow(&self, id: &str, provider: ProviderType, model: &str) -> LLMFlow {
        let now = Utc::now();

        LLMFlow {
            id: id.to_string(),
            flow_type: FlowType::ChatCompletions,
            request: LLMRequest {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                headers: HashMap::new(),
                body: serde_json::json!({
                    "model": model,
                    "messages": [{"role": "user", "content": "Hello, world!"}]
                }),
                timestamp: now,
                system_prompt: None,
                messages: vec![Message {
                    role: MessageRole::User,
                    content: MessageContent::Text("Hello, world!".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_result: None,
                }],
                parameters: RequestParameters {
                    temperature: None,
                    top_p: None,
                    max_tokens: None,
                    stop: None,
                    stream: false,
                    extra: HashMap::new(),
                },
                model: model.to_string(),
                original_model: Some(model.to_string()),
                size_bytes: 100,
                tools: None,
            },
            response: Some(LLMResponse {
                status_code: 200,
                status_text: "OK".to_string(),
                headers: HashMap::new(),
                body: serde_json::json!({
                    "id": "chatcmpl-test",
                    "object": "chat.completion",
                    "created": now.timestamp(),
                    "model": model,
                    "choices": []
                }),
                content: "Hello! How can I help you today?".to_string(),
                stop_reason: None,
                usage: TokenUsage {
                    input_tokens: 10,
                    output_tokens: 20,
                    total_tokens: 30,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    thinking_tokens: None,
                },
                stream_info: None,
                thinking: None,
                tool_calls: vec![],
                size_bytes: 200,
                timestamp_start: now,
                timestamp_end: now,
            }),
            error: None,
            metadata: FlowMetadata {
                provider: provider,
                provider_id: None,
                credential_name: Some("test-cred".to_string()),
                credential_id: Some("test-cred-id".to_string()),
                retry_count: 0,
                injected_params: Some(HashMap::new()),
                context_usage_percentage: None,
                client_info: ClientInfo::default(),
                routing_info: RoutingInfo::default(),
            },
            timestamps: FlowTimestamps {
                created: now,
                request_start: now,
                request_end: Some(now),
                response_start: Some(now),
                response_end: Some(now),
                duration_ms: 500,
                ttfb_ms: Some(100),
            },
            state: FlowState::Completed,
            annotations: FlowAnnotations::default(),
        }
    }

    /// 设置测试数据
    pub async fn setup_test_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        // 创建多样化的测试 Flow
        let test_flows = vec![
            ("flow-kiro-claude", ProviderType::Kiro, "claude-3-5-sonnet"),
            ("flow-openai-gpt4", ProviderType::OpenAI, "gpt-4"),
            ("flow-gemini-pro", ProviderType::Gemini, "gemini-pro"),
            (
                "flow-kiro-claude-2",
                ProviderType::Kiro,
                "claude-3-5-sonnet",
            ),
            ("flow-openai-gpt35", ProviderType::OpenAI, "gpt-3.5-turbo"),
        ];

        for (id, provider, model) in test_flows {
            let flow = self.create_test_flow(id, provider, model);
            // 直接添加到内存存储
            let memory_store = self.flow_monitor.memory_store();
            let mut store = memory_store.write().await;
            store.add(flow);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 端到端测试：基础 Flow 操作
    #[tokio::test]
    async fn test_e2e_basic_flow_operations() {
        let ctx = E2ETestContext::new().await.unwrap();
        ctx.setup_test_data().await.unwrap();

        // 1. 验证 Flow 已添加到内存存储
        let memory_store = ctx.flow_monitor.memory_store();
        let store = memory_store.read().await;
        let recent_flows = store.get_recent(10);
        assert_eq!(recent_flows.len(), 5);

        // 2. 测试按 ID 获取 Flow
        let flow_lock = store.get("flow-kiro-claude");
        assert!(flow_lock.is_some());
        let flow_lock = flow_lock.unwrap();
        let flow = flow_lock.read().unwrap();
        assert_eq!(flow.id, "flow-kiro-claude");

        // 3. 测试过滤功能
        let filter = FlowFilter {
            providers: Some(vec![ProviderType::Kiro]),
            ..Default::default()
        };
        let filtered_flows = store.query(&filter);
        assert_eq!(filtered_flows.len(), 2); // 应该有 2 个 Kiro 的 Flow

        // 4. 测试状态过滤
        let state_filter = FlowFilter {
            states: Some(vec![FlowState::Completed]),
            ..Default::default()
        };
        let completed_flows = store.query(&state_filter);
        assert_eq!(completed_flows.len(), 5); // 所有 Flow 都是 Completed 状态

        println!("✅ 基础 Flow 操作端到端测试通过");
    }

    /// 端到端测试：Flow 查询服务
    #[tokio::test]
    async fn test_e2e_flow_query_service() {
        let ctx = E2ETestContext::new().await.unwrap();
        ctx.setup_test_data().await.unwrap();

        // 1. 测试查询功能
        let result = ctx
            .flow_query_service
            .query(FlowFilter::default(), FlowSortBy::CreatedAt, true, 1, 20)
            .await
            .unwrap();

        assert_eq!(result.flows.len(), 5);
        assert_eq!(result.total, 5);
        assert_eq!(result.page, 1);
        assert_eq!(result.page_size, 20);

        // 2. 测试分页
        let page_result = ctx
            .flow_query_service
            .query(FlowFilter::default(), FlowSortBy::CreatedAt, true, 1, 3)
            .await
            .unwrap();

        assert_eq!(page_result.flows.len(), 3);
        assert_eq!(page_result.total, 5);

        // 3. 测试按 ID 获取
        let flow = ctx
            .flow_query_service
            .get_flow("flow-kiro-claude")
            .await
            .unwrap();
        assert!(flow.is_some());
        let flow = flow.unwrap();
        assert_eq!(flow.id, "flow-kiro-claude");

        // 4. 测试搜索功能
        let search_results = ctx.flow_query_service.search("claude", 10).await.unwrap();
        assert_eq!(search_results.len(), 2); // 应该找到 2 个包含 claude 的 Flow

        // 5. 测试统计功能
        let stats = ctx
            .flow_query_service
            .get_stats(&FlowFilter::default())
            .await;
        assert_eq!(stats.total_requests, 5);

        println!("✅ Flow 查询服务端到端测试通过");
    }

    /// 端到端测试：Flow 导出功能
    #[tokio::test]
    async fn test_e2e_flow_export() {
        let ctx = E2ETestContext::new().await.unwrap();
        ctx.setup_test_data().await.unwrap();

        // 获取所有 Flow
        let memory_store = ctx.flow_monitor.memory_store();
        let store = memory_store.read().await;
        let all_flows = store.get_recent(10);

        // 1. 测试 JSON 导出
        let json_options = ExportOptions {
            format: ExportFormat::JSON,
            filter: None,
            include_raw: true,
            include_stream_chunks: false,
            redact_sensitive: false,
            redaction_rules: Vec::new(),
            compress: false,
        };
        let json_exporter = FlowExporter::new(json_options);
        let json_data = json_exporter.export_json(&all_flows);
        // json_data 应该是一个数组，包含所有的 Flow
        assert!(json_data.is_array());
        let flows_array = json_data.as_array().unwrap();
        assert_eq!(flows_array.len(), all_flows.len());

        // 2. 测试 JSONL 导出
        let jsonl_options = ExportOptions {
            format: ExportFormat::JSONL,
            filter: None,
            include_raw: true,
            include_stream_chunks: false,
            redact_sensitive: false,
            redaction_rules: Vec::new(),
            compress: false,
        };
        let jsonl_exporter = FlowExporter::new(jsonl_options);
        let jsonl_data = jsonl_exporter.export_jsonl(&all_flows);
        let lines: Vec<&str> = jsonl_data.lines().collect();
        assert_eq!(lines.len(), 5); // 应该有 5 行

        // 3. 测试 HAR 导出
        let har_options = ExportOptions {
            format: ExportFormat::HAR,
            filter: None,
            include_raw: true,
            include_stream_chunks: false,
            redact_sensitive: false,
            redaction_rules: Vec::new(),
            compress: false,
        };
        let har_exporter = FlowExporter::new(har_options);
        let har_archive = har_exporter.export_har(&all_flows);
        assert_eq!(har_archive.log.entries.len(), 5);

        // 4. 测试 Markdown 导出
        let md_options = ExportOptions {
            format: ExportFormat::Markdown,
            filter: None,
            include_raw: false,
            include_stream_chunks: false,
            redact_sensitive: true,
            redaction_rules: Vec::new(),
            compress: false,
        };
        let md_exporter = FlowExporter::new(md_options);
        let md_data = md_exporter.export_markdown_multiple(&all_flows);
        assert!(md_data.contains("#")); // Markdown 应该包含标题

        // 5. 测试 CSV 导出
        let csv_options = ExportOptions {
            format: ExportFormat::CSV,
            filter: None,
            include_raw: false,
            include_stream_chunks: false,
            redact_sensitive: false,
            redaction_rules: Vec::new(),
            compress: false,
        };
        let csv_exporter = FlowExporter::new(csv_options);
        let csv_data = csv_exporter.export_csv(&all_flows);
        let lines: Vec<&str> = csv_data.lines().collect();
        assert!(lines.len() > 1); // 应该有标题行和数据行

        println!("✅ Flow 导出端到端测试通过");
    }

    /// 端到端测试：Flow 标注功能
    #[tokio::test]
    async fn test_e2e_flow_annotations() {
        let ctx = E2ETestContext::new().await.unwrap();
        ctx.setup_test_data().await.unwrap();

        let flow_id = "flow-kiro-claude";

        // 1. 测试切换收藏状态
        let updated = ctx.flow_monitor.toggle_starred(flow_id).await;
        assert!(updated);

        // 2. 测试添加评论
        let comment_added = ctx
            .flow_monitor
            .add_comment(flow_id, "这是一个测试评论".to_string())
            .await;
        assert!(comment_added);

        // 3. 测试添加标签
        let tag_added = ctx.flow_monitor.add_tag(flow_id, "重要".to_string()).await;
        assert!(tag_added);

        // 4. 测试设置标记
        let marker_set = ctx
            .flow_monitor
            .set_marker(flow_id, Some("⭐".to_string()))
            .await;
        assert!(marker_set);

        // 5. 验证标注已更新
        let memory_store = ctx.flow_monitor.memory_store();
        {
            let store = memory_store.read().await;
            let flow_lock = store.get(flow_id);
            assert!(flow_lock.is_some());
            let flow_lock = flow_lock.unwrap();
            let flow = flow_lock.read().unwrap();
            assert!(flow.annotations.starred);
            assert!(flow.annotations.comment.is_some());
            assert!(flow.annotations.tags.contains(&"重要".to_string()));
            assert_eq!(flow.annotations.marker, Some("⭐".to_string()));
        } // 确保 store 锁在这里被释放

        // 6. 测试移除标签
        let tag_removed = ctx.flow_monitor.remove_tag(flow_id, "重要").await;
        assert!(tag_removed);

        // 7. 测试清除标记
        let marker_cleared = ctx.flow_monitor.set_marker(flow_id, None).await;
        assert!(marker_cleared);

        println!("✅ Flow 标注端到端测试通过");
    }

    /// 端到端测试：Flow 过滤和排序
    #[tokio::test]
    async fn test_e2e_flow_filtering_and_sorting() {
        let ctx = E2ETestContext::new().await.unwrap();
        ctx.setup_test_data().await.unwrap();

        // 1. 测试按提供商过滤
        let provider_filter = FlowFilter {
            providers: Some(vec![ProviderType::Kiro]),
            ..Default::default()
        };
        let kiro_result = ctx
            .flow_query_service
            .query(provider_filter, FlowSortBy::CreatedAt, true, 1, 20)
            .await
            .unwrap();
        assert_eq!(kiro_result.flows.len(), 2);

        // 2. 测试按状态过滤
        let state_filter = FlowFilter {
            states: Some(vec![FlowState::Completed]),
            ..Default::default()
        };
        let completed_result = ctx
            .flow_query_service
            .query(state_filter, FlowSortBy::Duration, false, 1, 20)
            .await
            .unwrap();
        assert_eq!(completed_result.flows.len(), 5);

        // 3. 测试分页
        let page_result = ctx
            .flow_query_service
            .query(FlowFilter::default(), FlowSortBy::CreatedAt, true, 1, 3)
            .await
            .unwrap();
        assert_eq!(page_result.flows.len(), 3);
        assert_eq!(page_result.total, 5);

        // 4. 测试排序
        let sorted_result = ctx
            .flow_query_service
            .query(FlowFilter::default(), FlowSortBy::Duration, true, 1, 20)
            .await
            .unwrap();
        assert_eq!(sorted_result.flows.len(), 5);

        println!("✅ Flow 过滤和排序端到端测试通过");
    }
}
