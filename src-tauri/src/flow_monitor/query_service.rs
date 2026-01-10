//! Flow 查询服务
//!
//! 该模块实现 LLM Flow 的查询服务，支持多维度过滤、排序、分页和全文搜索。
//! 查询时先检查内存缓存，再检查文件存储。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

use super::file_store::{FileStoreError, FlowFileStore};
use super::filter_parser::{FilterParseError, FilterParser};
use super::memory_store::{FlowFilter, FlowMemoryStore};
use super::models::{FlowState, LLMFlow};

// ============================================================================
// 错误类型
// ============================================================================

/// 使用过滤表达式查询时的错误
#[derive(Debug, Error)]
pub enum QueryWithExpressionError {
    /// 过滤表达式解析错误
    #[error("过滤表达式解析错误: {0}")]
    ParseError(#[from] FilterParseError),
    /// 文件存储错误
    #[error("文件存储错误: {0}")]
    FileStoreError(#[from] FileStoreError),
}

// ============================================================================
// 排序选项
// ============================================================================

/// 排序字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowSortBy {
    /// 按创建时间排序
    CreatedAt,
    /// 按耗时排序
    Duration,
    /// 按总 Token 数排序
    TotalTokens,
    /// 按响应内容长度排序
    ContentLength,
    /// 按模型名称排序
    Model,
}

impl Default for FlowSortBy {
    fn default() -> Self {
        FlowSortBy::CreatedAt
    }
}

// ============================================================================
// 查询结果
// ============================================================================

/// 查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowQueryResult {
    /// 匹配的 Flow 列表
    pub flows: Vec<LLMFlow>,
    /// 总数（不含分页）
    pub total: usize,
    /// 当前页码
    pub page: usize,
    /// 每页大小
    pub page_size: usize,
    /// 总页数
    pub total_pages: usize,
    /// 是否有下一页
    pub has_next: bool,
    /// 是否有上一页
    pub has_prev: bool,
}

impl FlowQueryResult {
    /// 创建空结果
    pub fn empty(page: usize, page_size: usize) -> Self {
        Self {
            flows: Vec::new(),
            total: 0,
            page,
            page_size,
            total_pages: 0,
            has_next: false,
            has_prev: false,
        }
    }
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSearchResult {
    /// Flow ID
    pub id: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 模型名称
    pub model: String,
    /// 提供商
    pub provider: String,
    /// 匹配的内容片段
    pub snippet: String,
    /// 匹配分数
    pub score: f64,
}

// ============================================================================
// 统计信息
// ============================================================================

/// Flow 统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlowStats {
    /// 总请求数
    pub total_requests: usize,
    /// 成功请求数
    pub successful_requests: usize,
    /// 失败请求数
    pub failed_requests: usize,
    /// 成功率
    pub success_rate: f64,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,
    /// 最小延迟（毫秒）
    pub min_latency_ms: u64,
    /// 最大延迟（毫秒）
    pub max_latency_ms: u64,
    /// 总输入 Token 数
    pub total_input_tokens: u64,
    /// 总输出 Token 数
    pub total_output_tokens: u64,
    /// 平均输入 Token 数
    pub avg_input_tokens: f64,
    /// 平均输出 Token 数
    pub avg_output_tokens: f64,
    /// 按提供商统计
    pub by_provider: Vec<ProviderStats>,
    /// 按模型统计
    pub by_model: Vec<ModelStats>,
    /// 按状态统计
    pub by_state: Vec<StateStats>,
}

/// 按提供商统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStats {
    pub provider: String,
    pub count: usize,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
}

/// 按模型统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub model: String,
    pub count: usize,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
}

/// 按状态统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateStats {
    pub state: String,
    pub count: usize,
}

// ============================================================================
// 查询服务
// ============================================================================

/// Flow 查询服务
///
/// 提供统一的查询接口，先查内存缓存，再查文件存储。
pub struct FlowQueryService {
    /// 内存存储
    memory_store: Arc<RwLock<FlowMemoryStore>>,
    /// 文件存储
    file_store: Arc<FlowFileStore>,
}

impl FlowQueryService {
    /// 创建新的查询服务
    pub fn new(memory_store: Arc<RwLock<FlowMemoryStore>>, file_store: Arc<FlowFileStore>) -> Self {
        Self {
            memory_store,
            file_store,
        }
    }

    /// 查询 Flow
    ///
    /// # 参数
    /// - `filter`: 过滤条件
    /// - `sort_by`: 排序字段
    /// - `sort_desc`: 是否降序
    /// - `page`: 页码（从 1 开始）
    /// - `page_size`: 每页大小
    pub async fn query(
        &self,
        filter: FlowFilter,
        sort_by: FlowSortBy,
        sort_desc: bool,
        page: usize,
        page_size: usize,
    ) -> Result<FlowQueryResult, FileStoreError> {
        eprintln!(
            "[QUERY_SERVICE] 开始查询: filter={:?}, sort_by={:?}, page={}, page_size={}",
            filter, sort_by, page, page_size
        );

        // 先从内存获取
        let memory_flows = {
            let store = self.memory_store.read().await;
            let flows = store.query(&filter);
            eprintln!("[QUERY_SERVICE] 内存中查询到 {} 条记录", flows.len());
            flows
        };

        // 再从文件获取（如果需要更多数据）
        // 这里简化处理：如果内存数据足够，就不查文件
        // 实际应用中可能需要更复杂的合并逻辑
        let mut all_flows = memory_flows;

        // 如果内存数据不足，从文件补充
        let memory_count = all_flows.len();
        let needed = page * page_size;

        if memory_count < needed {
            eprintln!(
                "[QUERY_SERVICE] 内存数据不足，从文件补充: memory_count={}, needed={}",
                memory_count, needed
            );

            // 从文件存储获取更多数据
            let file_flows = self.file_store.query(&filter, needed * 2, 0)?;
            eprintln!("[QUERY_SERVICE] 文件中查询到 {} 条记录", file_flows.len());

            // 合并并去重（以 ID 为准）
            let memory_ids: std::collections::HashSet<_> =
                all_flows.iter().map(|f| f.id.clone()).collect();

            for flow in file_flows {
                if !memory_ids.contains(&flow.id) {
                    all_flows.push(flow);
                }
            }
        }

        eprintln!("[QUERY_SERVICE] 合并后总记录数: {}", all_flows.len());

        // 排序
        Self::sort_flows(&mut all_flows, sort_by, sort_desc);

        // 计算分页
        let total = all_flows.len();
        let total_pages = if page_size > 0 {
            (total + page_size - 1) / page_size
        } else {
            0
        };

        // 应用分页
        let page = page.max(1);
        let start = (page - 1) * page_size;
        let end = (start + page_size).min(total);

        let flows = if start < total {
            all_flows[start..end].to_vec()
        } else {
            Vec::new()
        };

        Ok(FlowQueryResult {
            flows,
            total,
            page,
            page_size,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        })
    }

    /// 使用过滤表达式查询 Flow
    ///
    /// 支持类似 mitmproxy 的过滤表达式语法，如：
    /// - `~m claude` - 模型名称包含 "claude"
    /// - `~p kiro & ~m claude` - 提供商为 kiro 且模型包含 claude
    /// - `~e | ~latency >5s` - 有错误或延迟超过 5 秒
    ///
    /// # 参数
    /// - `filter_expr`: 过滤表达式字符串
    /// - `sort_by`: 排序字段
    /// - `sort_desc`: 是否降序
    /// - `page`: 页码（从 1 开始）
    /// - `page_size`: 每页大小
    ///
    /// # 返回
    /// - `Ok(FlowQueryResult)` - 查询结果
    /// - `Err(QueryWithExpressionError)` - 解析或查询错误
    pub async fn query_with_expression(
        &self,
        filter_expr: &str,
        sort_by: FlowSortBy,
        sort_desc: bool,
        page: usize,
        page_size: usize,
    ) -> Result<FlowQueryResult, QueryWithExpressionError> {
        // 解析过滤表达式
        let expr = FilterParser::parse(filter_expr)?;

        // 编译为过滤函数
        let filter_fn = FilterParser::compile(&expr);

        // 从内存获取所有 Flow 并应用过滤
        let memory_flows = {
            let store = self.memory_store.read().await;
            let all_flows = store.query(&FlowFilter::default());
            all_flows
                .into_iter()
                .filter(|f| filter_fn(f))
                .collect::<Vec<_>>()
        };

        let mut all_flows = memory_flows;

        // 如果内存数据不足，从文件补充
        let memory_count = all_flows.len();
        let needed = page * page_size;

        if memory_count < needed {
            // 从文件存储获取更多数据
            let file_flows = self
                .file_store
                .query(&FlowFilter::default(), needed * 2, 0)?;

            // 合并并去重（以 ID 为准），同时应用过滤
            let memory_ids: std::collections::HashSet<_> =
                all_flows.iter().map(|f| f.id.clone()).collect();

            for flow in file_flows {
                if !memory_ids.contains(&flow.id) && filter_fn(&flow) {
                    all_flows.push(flow);
                }
            }
        }

        // 排序
        Self::sort_flows(&mut all_flows, sort_by, sort_desc);

        // 计算分页
        let total = all_flows.len();
        let total_pages = if page_size > 0 {
            (total + page_size - 1) / page_size
        } else {
            0
        };

        // 应用分页
        let page = page.max(1);
        let start = (page - 1) * page_size;
        let end = (start + page_size).min(total);

        let flows = if start < total {
            all_flows[start..end].to_vec()
        } else {
            Vec::new()
        };

        Ok(FlowQueryResult {
            flows,
            total,
            page,
            page_size,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        })
    }

    /// 排序 Flow 列表
    fn sort_flows(flows: &mut [LLMFlow], sort_by: FlowSortBy, desc: bool) {
        flows.sort_by(|a, b| {
            let cmp = match sort_by {
                FlowSortBy::CreatedAt => a.timestamps.created.cmp(&b.timestamps.created),
                FlowSortBy::Duration => a.timestamps.duration_ms.cmp(&b.timestamps.duration_ms),
                FlowSortBy::TotalTokens => {
                    let a_tokens = a.response.as_ref().map_or(0, |r| r.usage.total_tokens);
                    let b_tokens = b.response.as_ref().map_or(0, |r| r.usage.total_tokens);
                    a_tokens.cmp(&b_tokens)
                }
                FlowSortBy::ContentLength => {
                    let a_len = a.response.as_ref().map_or(0, |r| r.content.len());
                    let b_len = b.response.as_ref().map_or(0, |r| r.content.len());
                    a_len.cmp(&b_len)
                }
                FlowSortBy::Model => a.request.model.cmp(&b.request.model),
            };

            if desc {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }

    /// 全文搜索
    ///
    /// 使用 SQLite FTS5 进行全文搜索
    ///
    /// # 参数
    /// - `query`: 搜索关键词
    /// - `limit`: 最大返回数量
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FlowSearchResult>, FileStoreError> {
        // 先在内存中搜索
        let memory_results = self.search_in_memory(query, limit).await;

        // 如果内存结果不足，在文件中搜索
        if memory_results.len() < limit {
            let file_results = self.search_in_file(query, limit - memory_results.len())?;

            // 合并结果
            let mut all_results = memory_results;
            let existing_ids: std::collections::HashSet<_> =
                all_results.iter().map(|r| r.id.clone()).collect();

            for result in file_results {
                if !existing_ids.contains(&result.id) {
                    all_results.push(result);
                }
            }

            // 按分数排序
            all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

            Ok(all_results)
        } else {
            Ok(memory_results)
        }
    }

    /// 在内存中搜索
    async fn search_in_memory(&self, query: &str, limit: usize) -> Vec<FlowSearchResult> {
        let store = self.memory_store.read().await;
        let query_lower = query.to_lowercase();

        let mut results = Vec::new();

        // 获取所有 Flow 并手动搜索
        let all_flows = store.get_recent(10000); // 获取足够多的 Flow

        for flow in all_flows {
            // 检查是否匹配搜索条件
            let mut matches = false;
            let mut match_text = String::new();

            // 搜索 Flow ID
            if flow.id.to_lowercase().contains(&query_lower) {
                matches = true;
                match_text = flow.id.clone();
            }

            // 搜索模型名称
            if !matches && flow.request.model.to_lowercase().contains(&query_lower) {
                matches = true;
                match_text = flow.request.model.clone();
            }

            // 搜索响应内容
            if !matches {
                if let Some(ref response) = flow.response {
                    if response.content.to_lowercase().contains(&query_lower) {
                        matches = true;
                        match_text = response.content.clone();
                    }
                }
            }

            // 搜索请求消息
            if !matches {
                for message in &flow.request.messages {
                    let message_text = message.content.get_all_text();
                    if message_text.to_lowercase().contains(&query_lower) {
                        matches = true;
                        match_text = message_text;
                        break;
                    }
                }
            }

            if matches {
                let snippet = Self::extract_snippet(&match_text, &query_lower, 100);
                let score = Self::calculate_score(&match_text, &query_lower);

                results.push(FlowSearchResult {
                    id: flow.id,
                    created_at: flow.timestamps.created,
                    model: flow.request.model,
                    provider: format!("{:?}", flow.metadata.provider),
                    snippet,
                    score,
                });

                if results.len() >= limit {
                    break;
                }
            }
        }

        // 按分数排序
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// 在文件中搜索（使用 SQLite FTS5）
    fn search_in_file(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FlowSearchResult>, FileStoreError> {
        let fts_results = self.file_store.search(query, limit)?;

        let results: Vec<FlowSearchResult> = fts_results
            .into_iter()
            .filter_map(|r| {
                // 解析创建时间
                let created_at = chrono::DateTime::parse_from_rfc3339(&r.created_at)
                    .ok()?
                    .with_timezone(&Utc);

                Some(FlowSearchResult {
                    id: r.id,
                    created_at,
                    model: r.model,
                    provider: r.provider,
                    snippet: r.snippet,
                    score: 1.0, // FTS5 已经按 rank 排序
                })
            })
            .collect();

        Ok(results)
    }

    /// 提取匹配片段
    fn extract_snippet(content: &str, query: &str, max_len: usize) -> String {
        let content_lower = content.to_lowercase();

        if let Some(pos) = content_lower.find(query) {
            let start = pos.saturating_sub(max_len / 2);
            let end = (pos + query.len() + max_len / 2).min(content.len());

            let mut snippet = String::new();
            if start > 0 {
                snippet.push_str("...");
            }
            snippet.push_str(&content[start..end]);
            if end < content.len() {
                snippet.push_str("...");
            }
            snippet
        } else {
            content.chars().take(max_len).collect()
        }
    }

    /// 计算匹配分数
    fn calculate_score(content: &str, query: &str) -> f64 {
        let content_lower = content.to_lowercase();
        let count = content_lower.matches(query).count();

        // 简单的 TF 分数
        if content.is_empty() {
            0.0
        } else {
            (count as f64) / (content.len() as f64) * 1000.0
        }
    }

    /// 获取统计信息
    ///
    /// # 参数
    /// - `filter`: 过滤条件（可选）
    pub async fn get_stats(&self, filter: &FlowFilter) -> FlowStats {
        // 从内存获取 Flow
        let flows = {
            let store = self.memory_store.read().await;
            store.query(filter)
        };

        Self::calculate_stats(&flows)
    }

    /// 计算统计信息
    fn calculate_stats(flows: &[LLMFlow]) -> FlowStats {
        if flows.is_empty() {
            return FlowStats::default();
        }

        let total = flows.len();
        let mut successful = 0;
        let mut failed = 0;
        let mut total_latency: u64 = 0;
        let mut min_latency = u64::MAX;
        let mut max_latency = 0u64;
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;

        // 按提供商和模型分组
        let mut provider_map: std::collections::HashMap<String, (usize, usize, u64)> =
            std::collections::HashMap::new();
        let mut model_map: std::collections::HashMap<String, (usize, usize, u64)> =
            std::collections::HashMap::new();
        let mut state_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for flow in flows {
            // 状态统计
            let state_str = format!("{:?}", flow.state);
            *state_map.entry(state_str).or_insert(0) += 1;

            // 成功/失败统计
            match flow.state {
                FlowState::Completed => successful += 1,
                FlowState::Failed => failed += 1,
                _ => {}
            }

            // 延迟统计
            let latency = flow.timestamps.duration_ms;
            total_latency += latency;
            min_latency = min_latency.min(latency);
            max_latency = max_latency.max(latency);

            // Token 统计
            if let Some(ref response) = flow.response {
                total_input_tokens += response.usage.input_tokens as u64;
                total_output_tokens += response.usage.output_tokens as u64;
            }

            // 按提供商分组
            let provider_str = format!("{:?}", flow.metadata.provider);
            let provider_entry = provider_map.entry(provider_str).or_insert((0, 0, 0));
            provider_entry.0 += 1;
            if flow.state == FlowState::Completed {
                provider_entry.1 += 1;
            }
            provider_entry.2 += latency;

            // 按模型分组
            let model_entry = model_map
                .entry(flow.request.model.clone())
                .or_insert((0, 0, 0));
            model_entry.0 += 1;
            if flow.state == FlowState::Completed {
                model_entry.1 += 1;
            }
            model_entry.2 += latency;
        }

        // 构建统计结果
        let by_provider: Vec<ProviderStats> = provider_map
            .into_iter()
            .map(|(provider, (count, success, latency))| ProviderStats {
                provider,
                count,
                success_rate: if count > 0 {
                    success as f64 / count as f64
                } else {
                    0.0
                },
                avg_latency_ms: if count > 0 {
                    latency as f64 / count as f64
                } else {
                    0.0
                },
            })
            .collect();

        let by_model: Vec<ModelStats> = model_map
            .into_iter()
            .map(|(model, (count, success, latency))| ModelStats {
                model,
                count,
                success_rate: if count > 0 {
                    success as f64 / count as f64
                } else {
                    0.0
                },
                avg_latency_ms: if count > 0 {
                    latency as f64 / count as f64
                } else {
                    0.0
                },
            })
            .collect();

        let by_state: Vec<StateStats> = state_map
            .into_iter()
            .map(|(state, count)| StateStats { state, count })
            .collect();

        FlowStats {
            total_requests: total,
            successful_requests: successful,
            failed_requests: failed,
            success_rate: if total > 0 {
                successful as f64 / total as f64
            } else {
                0.0
            },
            avg_latency_ms: if total > 0 {
                total_latency as f64 / total as f64
            } else {
                0.0
            },
            min_latency_ms: if min_latency == u64::MAX {
                0
            } else {
                min_latency
            },
            max_latency_ms: max_latency,
            total_input_tokens,
            total_output_tokens,
            avg_input_tokens: if total > 0 {
                total_input_tokens as f64 / total as f64
            } else {
                0.0
            },
            avg_output_tokens: if total > 0 {
                total_output_tokens as f64 / total as f64
            } else {
                0.0
            },
            by_provider,
            by_model,
            by_state,
        }
    }

    /// 根据 ID 获取单个 Flow
    pub async fn get_flow(&self, id: &str) -> Result<Option<LLMFlow>, FileStoreError> {
        // 先从内存查找
        {
            let store = self.memory_store.read().await;
            if let Some(flow_lock) = store.get(id) {
                if let Ok(flow) = flow_lock.read() {
                    return Ok(Some(flow.clone()));
                }
            }
        }

        // 从文件查找
        self.file_store.get(id)
    }

    /// 获取最近的 Flow
    pub async fn get_recent(&self, limit: usize) -> Vec<LLMFlow> {
        let store = self.memory_store.read().await;
        store.get_recent(limit)
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_monitor::models::{
        FlowMetadata, FlowType, LLMRequest, LLMResponse, RequestParameters, TokenUsage,
    };
    use crate::ProviderType;

    /// 创建测试用的 Flow
    fn create_test_flow(
        id: &str,
        model: &str,
        provider: ProviderType,
        state: FlowState,
    ) -> LLMFlow {
        let request = LLMRequest {
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            model: model.to_string(),
            parameters: RequestParameters {
                stream: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let metadata = FlowMetadata {
            provider,
            ..Default::default()
        };

        let mut flow = LLMFlow::new(id.to_string(), FlowType::ChatCompletions, request, metadata);
        flow.state = state;
        flow
    }

    #[test]
    fn test_flow_sort_by_created_at() {
        let mut flows = vec![
            create_test_flow(
                "flow-1",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
            create_test_flow(
                "flow-2",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
            create_test_flow(
                "flow-3",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
        ];

        // 设置不同的创建时间
        flows[0].timestamps.created = Utc::now() - chrono::Duration::hours(2);
        flows[1].timestamps.created = Utc::now() - chrono::Duration::hours(1);
        flows[2].timestamps.created = Utc::now();

        // 升序排序
        FlowQueryService::sort_flows(&mut flows, FlowSortBy::CreatedAt, false);
        assert_eq!(flows[0].id, "flow-1");
        assert_eq!(flows[1].id, "flow-2");
        assert_eq!(flows[2].id, "flow-3");

        // 降序排序
        FlowQueryService::sort_flows(&mut flows, FlowSortBy::CreatedAt, true);
        assert_eq!(flows[0].id, "flow-3");
        assert_eq!(flows[1].id, "flow-2");
        assert_eq!(flows[2].id, "flow-1");
    }

    #[test]
    fn test_flow_sort_by_duration() {
        let mut flows = vec![
            create_test_flow(
                "flow-1",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
            create_test_flow(
                "flow-2",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
            create_test_flow(
                "flow-3",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
        ];

        flows[0].timestamps.duration_ms = 100;
        flows[1].timestamps.duration_ms = 300;
        flows[2].timestamps.duration_ms = 200;

        // 升序排序
        FlowQueryService::sort_flows(&mut flows, FlowSortBy::Duration, false);
        assert_eq!(flows[0].timestamps.duration_ms, 100);
        assert_eq!(flows[1].timestamps.duration_ms, 200);
        assert_eq!(flows[2].timestamps.duration_ms, 300);
    }

    #[test]
    fn test_flow_sort_by_model() {
        let mut flows = vec![
            create_test_flow(
                "flow-1",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
            create_test_flow(
                "flow-2",
                "claude-3",
                ProviderType::Claude,
                FlowState::Completed,
            ),
            create_test_flow(
                "flow-3",
                "gemini-pro",
                ProviderType::Gemini,
                FlowState::Completed,
            ),
        ];

        // 升序排序
        FlowQueryService::sort_flows(&mut flows, FlowSortBy::Model, false);
        assert_eq!(flows[0].request.model, "claude-3");
        assert_eq!(flows[1].request.model, "gemini-pro");
        assert_eq!(flows[2].request.model, "gpt-4");
    }

    #[test]
    fn test_calculate_stats() {
        let mut flows = vec![
            create_test_flow(
                "flow-1",
                "gpt-4",
                ProviderType::OpenAI,
                FlowState::Completed,
            ),
            create_test_flow("flow-2", "gpt-4", ProviderType::OpenAI, FlowState::Failed),
            create_test_flow(
                "flow-3",
                "claude-3",
                ProviderType::Claude,
                FlowState::Completed,
            ),
        ];

        // 设置延迟
        flows[0].timestamps.duration_ms = 100;
        flows[1].timestamps.duration_ms = 200;
        flows[2].timestamps.duration_ms = 150;

        // 设置响应
        flows[0].response = Some(LLMResponse {
            usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                ..Default::default()
            },
            ..Default::default()
        });
        flows[2].response = Some(LLMResponse {
            usage: TokenUsage {
                input_tokens: 200,
                output_tokens: 100,
                total_tokens: 300,
                ..Default::default()
            },
            ..Default::default()
        });

        let stats = FlowQueryService::calculate_stats(&flows);

        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 1);
        assert!((stats.success_rate - 2.0 / 3.0).abs() < 0.001);
        assert_eq!(stats.min_latency_ms, 100);
        assert_eq!(stats.max_latency_ms, 200);
        assert_eq!(stats.total_input_tokens, 300);
        assert_eq!(stats.total_output_tokens, 150);
    }

    #[test]
    fn test_extract_snippet() {
        let content = "This is a test content with some keywords for searching.";

        let snippet = FlowQueryService::extract_snippet(content, "keywords", 20);
        assert!(snippet.contains("keywords"));

        let snippet = FlowQueryService::extract_snippet(content, "notfound", 20);
        assert_eq!(snippet, "This is a test conte");
    }

    #[test]
    fn test_calculate_score() {
        let content = "hello world hello";
        let score = FlowQueryService::calculate_score(content, "hello");
        assert!(score > 0.0);

        let score_empty = FlowQueryService::calculate_score("", "hello");
        assert_eq!(score_empty, 0.0);
    }

    #[test]
    fn test_flow_query_result_empty() {
        let result = FlowQueryResult::empty(1, 10);
        assert!(result.flows.is_empty());
        assert_eq!(result.total, 0);
        assert_eq!(result.page, 1);
        assert_eq!(result.page_size, 10);
        assert!(!result.has_next);
        assert!(!result.has_prev);
    }
}

// ============================================================================
// 属性测试模块
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::flow_monitor::models::{
        FlowMetadata, FlowType, LLMRequest, LLMResponse, RequestParameters, TokenUsage,
    };
    use crate::ProviderType;
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
            Just(ProviderType::Antigravity),
        ]
    }

    /// 生成随机的 FlowState
    fn arb_flow_state() -> impl Strategy<Value = FlowState> {
        prop_oneof![
            Just(FlowState::Pending),
            Just(FlowState::Streaming),
            Just(FlowState::Completed),
            Just(FlowState::Failed),
            Just(FlowState::Cancelled),
        ]
    }

    /// 生成随机的 FlowSortBy
    fn arb_sort_by() -> impl Strategy<Value = FlowSortBy> {
        prop_oneof![
            Just(FlowSortBy::CreatedAt),
            Just(FlowSortBy::Duration),
            Just(FlowSortBy::TotalTokens),
            Just(FlowSortBy::ContentLength),
            Just(FlowSortBy::Model),
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

    /// 生成随机的 LLMFlow
    fn arb_llm_flow() -> impl Strategy<Value = LLMFlow> {
        (
            "[a-f0-9]{8}",
            arb_model_name(),
            arb_provider_type(),
            arb_flow_state(),
            0u64..10000u64,
            0u32..1000u32,
            0u32..500u32,
        )
            .prop_map(
                |(id, model, provider, state, duration, input_tokens, output_tokens)| {
                    let request = LLMRequest {
                        method: "POST".to_string(),
                        path: "/v1/chat/completions".to_string(),
                        model,
                        parameters: RequestParameters {
                            stream: false,
                            ..Default::default()
                        },
                        ..Default::default()
                    };

                    let metadata = FlowMetadata {
                        provider,
                        ..Default::default()
                    };

                    let mut flow = LLMFlow::new(id, FlowType::ChatCompletions, request, metadata);
                    flow.state = state;
                    flow.timestamps.duration_ms = duration;

                    if flow.state == FlowState::Completed {
                        flow.response = Some(LLMResponse {
                            usage: TokenUsage {
                                input_tokens,
                                output_tokens,
                                total_tokens: input_tokens + output_tokens,
                                ..Default::default()
                            },
                            ..Default::default()
                        });
                    }

                    flow
                },
            )
    }

    // ========================================================================
    // 属性测试
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: llm-flow-monitor, Property 5: 过滤正确性**
        /// **Validates: Requirements 4.1-4.9**
        ///
        /// *对于任意* 过滤条件和 Flow 集合，查询返回的所有 Flow 都应该满足该过滤条件。
        #[test]
        fn prop_filter_correctness(
            provider in arb_provider_type(),
        ) {
            // 创建不同 Provider 的 Flow
            let providers = vec![
                ProviderType::OpenAI,
                ProviderType::Claude,
                ProviderType::Gemini,
                ProviderType::Kiro,
            ];

            let mut flows: Vec<LLMFlow> = Vec::new();
            for (i, p) in providers.iter().enumerate() {
                let request = LLMRequest {
                    method: "POST".to_string(),
                    path: "/v1/chat/completions".to_string(),
                    model: "gpt-4".to_string(),
                    ..Default::default()
                };
                let metadata = FlowMetadata {
                    provider: p.clone(),
                    ..Default::default()
                };
                let flow = LLMFlow::new(format!("flow-{}", i), FlowType::ChatCompletions, request, metadata);
                flows.push(flow);
            }

            // 按 Provider 过滤
            let filter = FlowFilter {
                providers: Some(vec![provider.clone()]),
                ..Default::default()
            };

            let filtered: Vec<&LLMFlow> = flows.iter().filter(|f| filter.matches(f)).collect();

            // 验证所有结果都匹配过滤条件
            for flow in &filtered {
                prop_assert_eq!(
                    flow.metadata.provider,
                    provider,
                    "查询结果的 Provider 应该匹配过滤条件"
                );
            }
        }

        /// **Feature: llm-flow-monitor, Property 6: 排序正确性**
        /// **Validates: Requirements 4.10**
        ///
        /// *对于任意* 排序选项和 Flow 集合，查询返回的 Flow 列表应该按指定字段正确排序。
        #[test]
        fn prop_sort_correctness(
            sort_by in arb_sort_by(),
            desc in any::<bool>(),
        ) {
            // 创建多个 Flow
            let mut flows: Vec<LLMFlow> = Vec::new();
            for i in 0..10 {
                let request = LLMRequest {
                    method: "POST".to_string(),
                    path: "/v1/chat/completions".to_string(),
                    model: format!("model-{}", i % 3),
                    ..Default::default()
                };
                let metadata = FlowMetadata::default();
                let mut flow = LLMFlow::new(format!("flow-{}", i), FlowType::ChatCompletions, request, metadata);
                flow.timestamps.duration_ms = (i * 100) as u64;
                flow.timestamps.created = Utc::now() - chrono::Duration::minutes(i as i64);

                if i % 2 == 0 {
                    flow.response = Some(LLMResponse {
                        content: "x".repeat(i * 10),
                        usage: TokenUsage {
                            input_tokens: (i * 10) as u32,
                            output_tokens: (i * 5) as u32,
                            total_tokens: (i * 15) as u32,
                            ..Default::default()
                        },
                        ..Default::default()
                    });
                }

                flows.push(flow);
            }

            // 排序
            FlowQueryService::sort_flows(&mut flows, sort_by, desc);

            // 验证排序正确性
            for i in 1..flows.len() {
                let cmp = match sort_by {
                    FlowSortBy::CreatedAt => flows[i-1].timestamps.created.cmp(&flows[i].timestamps.created),
                    FlowSortBy::Duration => flows[i-1].timestamps.duration_ms.cmp(&flows[i].timestamps.duration_ms),
                    FlowSortBy::TotalTokens => {
                        let a = flows[i-1].response.as_ref().map_or(0, |r| r.usage.total_tokens);
                        let b = flows[i].response.as_ref().map_or(0, |r| r.usage.total_tokens);
                        a.cmp(&b)
                    }
                    FlowSortBy::ContentLength => {
                        let a = flows[i-1].response.as_ref().map_or(0, |r| r.content.len());
                        let b = flows[i].response.as_ref().map_or(0, |r| r.content.len());
                        a.cmp(&b)
                    }
                    FlowSortBy::Model => flows[i-1].request.model.cmp(&flows[i].request.model),
                };

                let expected = if desc {
                    cmp != std::cmp::Ordering::Less
                } else {
                    cmp != std::cmp::Ordering::Greater
                };

                prop_assert!(
                    expected,
                    "排序不正确: {:?} vs {:?} (sort_by={:?}, desc={})",
                    flows[i-1].id,
                    flows[i].id,
                    sort_by,
                    desc
                );
            }
        }

        /// **Feature: llm-flow-monitor, Property 7: 分页正确性**
        /// **Validates: Requirements 4.11**
        ///
        /// *对于任意* 分页参数（page, page_size）和 Flow 集合，
        /// 返回的结果应该是正确的分页切片，且总数应该正确。
        #[test]
        fn prop_pagination_correctness(
            total_count in 1usize..=100usize,
            page_size in 1usize..=20usize,
            page in 1usize..=10usize,
        ) {
            // 创建 Flow 列表
            let mut all_flows: Vec<LLMFlow> = Vec::new();
            for i in 0..total_count {
                let request = LLMRequest {
                    method: "POST".to_string(),
                    path: "/v1/chat/completions".to_string(),
                    model: "gpt-4".to_string(),
                    ..Default::default()
                };
                let metadata = FlowMetadata::default();
                let flow = LLMFlow::new(format!("flow-{:04}", i), FlowType::ChatCompletions, request, metadata);
                all_flows.push(flow);
            }

            // 计算分页
            let total = all_flows.len();
            let total_pages = if page_size > 0 {
                (total + page_size - 1) / page_size
            } else {
                0
            };

            let start = (page - 1) * page_size;
            let end = (start + page_size).min(total);

            let page_flows = if start < total {
                all_flows[start..end].to_vec()
            } else {
                Vec::new()
            };

            // 验证分页结果
            let expected_count = if start < total {
                (end - start).min(page_size)
            } else {
                0
            };

            prop_assert_eq!(
                page_flows.len(),
                expected_count,
                "分页结果数量不正确"
            );

            // 验证 has_next 和 has_prev
            let has_next = page < total_pages;
            let has_prev = page > 1;

            prop_assert_eq!(
                has_next,
                page < total_pages,
                "has_next 不正确"
            );

            prop_assert_eq!(
                has_prev,
                page > 1,
                "has_prev 不正确"
            );

            // 验证分页内容正确
            for (i, flow) in page_flows.iter().enumerate() {
                let expected_id = format!("flow-{:04}", start + i);
                prop_assert_eq!(
                    &flow.id,
                    &expected_id,
                    "分页内容不正确"
                );
            }
        }
    }
}
