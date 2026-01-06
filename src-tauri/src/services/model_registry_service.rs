//! 模型注册服务
//!
//! 负责从 aiclientproxy/models 仓库获取模型数据、管理本地缓存、提供模型搜索等功能

use crate::database::DbConnection;
use crate::models::model_registry::{
    EnhancedModelMetadata, ModelCapabilities, ModelLimits, ModelPricing, ModelSource, ModelStatus,
    ModelSyncState, ModelTier, UserModelPreference,
};
use rusqlite::params;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// GitHub 仓库 raw 文件基础 URL
const MODELS_REPO_BASE_URL: &str = "https://raw.githubusercontent.com/aiclientproxy/models/main";
const CACHE_DURATION_SECS: i64 = 3600; // 1 小时

/// 仓库索引文件结构
#[derive(Debug, Deserialize)]
struct RepoIndex {
    providers: Vec<String>,
    #[allow(dead_code)]
    total_models: u32,
}

/// 仓库中的 Provider 数据结构
#[derive(Debug, Deserialize)]
struct RepoProviderData {
    provider: RepoProvider,
    models: Vec<RepoModel>,
}

#[derive(Debug, Deserialize)]
struct RepoProvider {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct RepoModel {
    id: String,
    name: String,
    family: Option<String>,
    tier: Option<String>,
    capabilities: Option<RepoCapabilities>,
    pricing: Option<RepoPricing>,
    limits: Option<RepoLimits>,
    status: Option<String>,
    release_date: Option<String>,
    is_latest: Option<bool>,
    description: Option<String>,
    #[serde(default)]
    description_zh: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RepoCapabilities {
    #[serde(default)]
    vision: bool,
    #[serde(default)]
    tools: bool,
    #[serde(default)]
    streaming: bool,
    #[serde(default)]
    json_mode: bool,
    #[serde(default)]
    function_calling: bool,
    #[serde(default)]
    reasoning: bool,
}

#[derive(Debug, Deserialize)]
struct RepoPricing {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
    currency: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoLimits {
    context: Option<u32>,
    max_output: Option<u32>,
}

/// 模型注册服务
pub struct ModelRegistryService {
    /// 数据库连接
    db: DbConnection,
    /// 内存缓存的模型数据
    models_cache: Arc<RwLock<Vec<EnhancedModelMetadata>>>,
    /// 同步状态
    sync_state: Arc<RwLock<ModelSyncState>>,
}

impl ModelRegistryService {
    /// 创建新的模型注册服务
    pub fn new(db: DbConnection) -> Self {
        Self {
            db,
            models_cache: Arc::new(RwLock::new(Vec::new())),
            sync_state: Arc::new(RwLock::new(ModelSyncState::default())),
        }
    }

    /// 初始化服务
    pub async fn initialize(&self) -> Result<(), String> {
        tracing::info!("[ModelRegistry] 初始化模型注册服务");

        // 1. 尝试从数据库加载缓存
        match self.load_from_db().await {
            Ok(models) if !models.is_empty() => {
                tracing::info!("[ModelRegistry] 从数据库加载了 {} 个模型", models.len());
                let mut cache = self.models_cache.write().await;
                *cache = models;

                // 检查是否需要后台刷新
                if self.should_refresh().await {
                    tracing::info!("[ModelRegistry] 缓存已过期，启动后台刷新");
                    self.spawn_background_refresh();
                }
                return Ok(());
            }
            Ok(_) => {
                tracing::info!("[ModelRegistry] 数据库中没有缓存数据");
            }
            Err(e) => {
                tracing::warn!("[ModelRegistry] 从数据库加载失败: {}", e);
            }
        }

        // 2. 后台获取 models 仓库数据
        self.spawn_background_refresh();

        Ok(())
    }

    /// 检查是否需要刷新
    async fn should_refresh(&self) -> bool {
        let state = self.sync_state.read().await;
        match state.last_sync_at {
            Some(last_sync) => {
                let now = chrono::Utc::now().timestamp();
                now - last_sync > CACHE_DURATION_SECS
            }
            None => true,
        }
    }

    /// 启动后台刷新任务
    fn spawn_background_refresh(&self) {
        let db = self.db.clone();
        let models_cache = self.models_cache.clone();
        let sync_state = self.sync_state.clone();

        tokio::spawn(async move {
            let service = ModelRegistryService {
                db,
                models_cache,
                sync_state,
            };
            if let Err(e) = service.refresh_from_repo().await {
                tracing::error!("[ModelRegistry] 后台刷新失败: {}", e);
            }
        });
    }

    /// 从 aiclientproxy/models 仓库刷新数据
    pub async fn refresh_from_repo(&self) -> Result<(), String> {
        tracing::info!("[ModelRegistry] 开始从 models 仓库获取数据");

        // 设置同步状态
        {
            let mut state = self.sync_state.write().await;
            state.is_syncing = true;
            state.last_error = None;
        }

        // 获取数据
        let result = self.fetch_models_from_repo().await;

        match result {
            Ok(models) => {
                tracing::info!("[ModelRegistry] 获取了 {} 个模型", models.len());

                // 更新缓存
                {
                    let mut cache = self.models_cache.write().await;
                    *cache = models.clone();
                }

                // 保存到数据库
                self.save_models_to_db(&models).await?;

                // 更新同步状态
                {
                    let mut state = self.sync_state.write().await;
                    state.is_syncing = false;
                    state.last_sync_at = Some(chrono::Utc::now().timestamp());
                    state.model_count = models.len() as u32;
                    state.last_error = None;
                }

                // 保存同步状态到数据库
                self.save_sync_state().await?;

                Ok(())
            }
            Err(e) => {
                tracing::error!("[ModelRegistry] 从 models 仓库获取数据失败: {}", e);

                // 更新同步状态
                {
                    let mut state = self.sync_state.write().await;
                    state.is_syncing = false;
                    state.last_error = Some(e.clone());
                }

                Err(e)
            }
        }
    }

    /// 从 models 仓库获取数据
    async fn fetch_models_from_repo(&self) -> Result<Vec<EnhancedModelMetadata>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        // 1. 获取索引文件
        let index_url = format!("{}/index.json", MODELS_REPO_BASE_URL);
        let index: RepoIndex = client
            .get(&index_url)
            .header("User-Agent", "ProxyCast/1.0")
            .send()
            .await
            .map_err(|e| format!("请求 index.json 失败: {}", e))?
            .json()
            .await
            .map_err(|e| format!("解析 index.json 失败: {}", e))?;

        tracing::info!(
            "[ModelRegistry] 索引包含 {} 个 providers",
            index.providers.len()
        );

        // 2. 并发获取所有 provider 数据
        let mut models = Vec::new();
        let now = chrono::Utc::now().timestamp();

        for provider_id in &index.providers {
            let provider_url = format!("{}/providers/{}.json", MODELS_REPO_BASE_URL, provider_id);

            match client
                .get(&provider_url)
                .header("User-Agent", "ProxyCast/1.0")
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<RepoProviderData>().await {
                            Ok(provider_data) => {
                                for model in provider_data.models {
                                    let enhanced = self.convert_repo_model(
                                        model,
                                        &provider_data.provider.id,
                                        &provider_data.provider.name,
                                        now,
                                    );
                                    models.push(enhanced);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("[ModelRegistry] 解析 {} 失败: {}", provider_id, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("[ModelRegistry] 获取 {} 失败: {}", provider_id, e);
                }
            }
        }

        // 按 provider_id 和 display_name 排序
        models.sort_by(|a, b| {
            a.provider_id
                .cmp(&b.provider_id)
                .then(a.display_name.cmp(&b.display_name))
        });

        tracing::info!(
            "[ModelRegistry] 从 models 仓库获取了 {} 个模型",
            models.len()
        );

        Ok(models)
    }

    /// 转换仓库模型格式为内部格式
    fn convert_repo_model(
        &self,
        model: RepoModel,
        provider_id: &str,
        provider_name: &str,
        now: i64,
    ) -> EnhancedModelMetadata {
        let caps = model.capabilities.unwrap_or_default();

        EnhancedModelMetadata {
            id: model.id,
            display_name: model.name,
            provider_id: provider_id.to_string(),
            provider_name: provider_name.to_string(),
            family: model.family,
            tier: model
                .tier
                .and_then(|t| t.parse().ok())
                .unwrap_or(ModelTier::Pro),
            capabilities: ModelCapabilities {
                vision: caps.vision,
                tools: caps.tools,
                streaming: caps.streaming,
                json_mode: caps.json_mode,
                function_calling: caps.function_calling,
                reasoning: caps.reasoning,
            },
            pricing: model.pricing.map(|p| ModelPricing {
                input_per_million: p.input,
                output_per_million: p.output,
                cache_read_per_million: p.cache_read,
                cache_write_per_million: p.cache_write,
                currency: p.currency.unwrap_or_else(|| "USD".to_string()),
            }),
            limits: ModelLimits {
                context_length: model.limits.as_ref().and_then(|l| l.context),
                max_output_tokens: model.limits.as_ref().and_then(|l| l.max_output),
                requests_per_minute: None,
                tokens_per_minute: None,
            },
            status: model
                .status
                .and_then(|s| s.parse().ok())
                .unwrap_or(ModelStatus::Active),
            release_date: model.release_date,
            is_latest: model.is_latest.unwrap_or(false),
            description: model.description_zh.or(model.description),
            source: ModelSource::ModelsDev,
            created_at: now,
            updated_at: now,
        }
    }

    /// 从数据库加载模型
    async fn load_from_db(&self) -> Result<Vec<EnhancedModelMetadata>, String> {
        let (models, sync_rows) = {
            let conn = self.db.lock().map_err(|e| e.to_string())?;

            let mut stmt = conn
                .prepare(
                    "SELECT id, display_name, provider_id, provider_name, family, tier,
                            capabilities, pricing, limits, status, release_date, is_latest,
                            description, source, created_at, updated_at
                     FROM model_registry",
                )
                .map_err(|e| e.to_string())?;

            let models = stmt
                .query_map([], |row| {
                    let capabilities_json: String = row.get(6)?;
                    let pricing_json: Option<String> = row.get(7)?;
                    let limits_json: String = row.get(8)?;
                    let status_str: String = row.get(9)?;
                    let tier_str: String = row.get(5)?;
                    let source_str: String = row.get(13)?;

                    Ok(EnhancedModelMetadata {
                        id: row.get(0)?,
                        display_name: row.get(1)?,
                        provider_id: row.get(2)?,
                        provider_name: row.get(3)?,
                        family: row.get(4)?,
                        tier: tier_str.parse().unwrap_or(ModelTier::Pro),
                        capabilities: serde_json::from_str(&capabilities_json).unwrap_or_default(),
                        pricing: pricing_json.and_then(|s| serde_json::from_str(&s).ok()),
                        limits: serde_json::from_str(&limits_json).unwrap_or_default(),
                        status: status_str.parse().unwrap_or(ModelStatus::Active),
                        release_date: row.get(10)?,
                        is_latest: row.get::<_, i32>(11)? != 0,
                        description: row.get(12)?,
                        source: source_str.parse().unwrap_or(ModelSource::Local),
                        created_at: row.get(14)?,
                        updated_at: row.get(15)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            // 加载同步状态数据
            let mut sync_stmt = conn
                .prepare("SELECT key, value FROM model_sync_state")
                .map_err(|e| e.to_string())?;

            let sync_rows: Vec<(String, String)> = sync_stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            (models, sync_rows)
        }; // conn 锁在这里释放

        // 更新同步状态（在锁释放后）
        {
            let mut state = self.sync_state.write().await;
            for (key, value) in sync_rows {
                match key.as_str() {
                    "last_sync_at" => {
                        state.last_sync_at = value.parse().ok();
                    }
                    "model_count" => {
                        state.model_count = value.parse().unwrap_or(0);
                    }
                    "last_error" => {
                        state.last_error = if value.is_empty() { None } else { Some(value) };
                    }
                    _ => {}
                }
            }
        }

        Ok(models)
    }

    /// 保存模型到数据库
    async fn save_models_to_db(&self, models: &[EnhancedModelMetadata]) -> Result<(), String> {
        let conn = self.db.lock().map_err(|e| e.to_string())?;

        // 开始事务
        conn.execute("BEGIN TRANSACTION", [])
            .map_err(|e| e.to_string())?;

        // 清空现有数据
        conn.execute("DELETE FROM model_registry", [])
            .map_err(|e| e.to_string())?;

        // 插入新数据
        let mut stmt = conn
            .prepare(
                "INSERT INTO model_registry (
                    id, display_name, provider_id, provider_name, family, tier,
                    capabilities, pricing, limits, status, release_date, is_latest,
                    description, source, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .map_err(|e| e.to_string())?;

        for model in models {
            let capabilities_json = serde_json::to_string(&model.capabilities).unwrap_or_default();
            let pricing_json = model
                .pricing
                .as_ref()
                .map(|p| serde_json::to_string(p).unwrap_or_default());
            let limits_json = serde_json::to_string(&model.limits).unwrap_or_default();

            stmt.execute(params![
                model.id,
                model.display_name,
                model.provider_id,
                model.provider_name,
                model.family,
                model.tier.to_string(),
                capabilities_json,
                pricing_json,
                limits_json,
                model.status.to_string(),
                model.release_date,
                model.is_latest as i32,
                model.description,
                model.source.to_string(),
                model.created_at,
                model.updated_at,
            ])
            .map_err(|e| e.to_string())?;
        }

        // 提交事务
        conn.execute("COMMIT", []).map_err(|e| e.to_string())?;

        tracing::info!("[ModelRegistry] 保存了 {} 个模型到数据库", models.len());

        Ok(())
    }

    /// 保存同步状态
    async fn save_sync_state(&self) -> Result<(), String> {
        let (last_sync_at, model_count, last_error) = {
            let state = self.sync_state.read().await;
            (
                state.last_sync_at,
                state.model_count,
                state.last_error.clone(),
            )
        };

        let conn = self.db.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn
            .prepare(
                "INSERT OR REPLACE INTO model_sync_state (key, value, updated_at)
                 VALUES (?, ?, ?)",
            )
            .map_err(|e| e.to_string())?;

        if let Some(last_sync) = last_sync_at {
            stmt.execute(params!["last_sync_at", last_sync.to_string(), now])
                .map_err(|e| e.to_string())?;
        }

        stmt.execute(params!["model_count", model_count.to_string(), now])
            .map_err(|e| e.to_string())?;

        if let Some(ref error) = last_error {
            stmt.execute(params!["last_error", error, now])
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// 获取所有模型
    pub async fn get_all_models(&self) -> Vec<EnhancedModelMetadata> {
        self.models_cache.read().await.clone()
    }

    /// 获取同步状态
    pub async fn get_sync_state(&self) -> ModelSyncState {
        self.sync_state.read().await.clone()
    }

    /// 按 Provider 获取模型
    pub async fn get_models_by_provider(&self, provider_id: &str) -> Vec<EnhancedModelMetadata> {
        self.models_cache
            .read()
            .await
            .iter()
            .filter(|m| m.provider_id == provider_id)
            .cloned()
            .collect()
    }

    /// 按服务等级获取模型
    pub async fn get_models_by_tier(&self, tier: ModelTier) -> Vec<EnhancedModelMetadata> {
        self.models_cache
            .read()
            .await
            .iter()
            .filter(|m| m.tier == tier)
            .cloned()
            .collect()
    }

    /// 搜索模型（简单的模糊匹配）
    pub async fn search_models(&self, query: &str, limit: usize) -> Vec<EnhancedModelMetadata> {
        let models = self.models_cache.read().await;

        if query.is_empty() {
            return models.iter().take(limit).cloned().collect();
        }

        let query_lower = query.to_lowercase();
        let mut scored: Vec<(f64, &EnhancedModelMetadata)> = models
            .iter()
            .filter_map(|m| {
                let score = self.calculate_search_score(m, &query_lower);
                if score > 0.0 {
                    Some((score, m))
                } else {
                    None
                }
            })
            .collect();

        // 按分数降序排序
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(limit)
            .map(|(_, m)| m.clone())
            .collect()
    }

    /// 计算搜索匹配分数
    fn calculate_search_score(&self, model: &EnhancedModelMetadata, query: &str) -> f64 {
        let mut score = 0.0;

        // 精确匹配 ID
        if model.id.to_lowercase() == query {
            score += 100.0;
        } else if model.id.to_lowercase().contains(query) {
            score += 50.0;
        }

        // 显示名称匹配
        if model.display_name.to_lowercase().contains(query) {
            score += 30.0;
        }

        // Provider 匹配
        if model.provider_name.to_lowercase().contains(query) {
            score += 20.0;
        }

        // 家族匹配
        if let Some(family) = &model.family {
            if family.to_lowercase().contains(query) {
                score += 15.0;
            }
        }

        // 最新版本加分
        if model.is_latest {
            score += 5.0;
        }

        // 活跃状态加分
        if model.status == ModelStatus::Active {
            score += 3.0;
        }

        score
    }

    // ========== 用户偏好相关方法 ==========

    /// 获取所有用户偏好
    pub async fn get_all_preferences(&self) -> Result<Vec<UserModelPreference>, String> {
        let conn = self.db.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT model_id, is_favorite, is_hidden, custom_alias,
                        usage_count, last_used_at, created_at, updated_at
                 FROM user_model_preferences",
            )
            .map_err(|e| e.to_string())?;

        let prefs = stmt
            .query_map([], |row| {
                Ok(UserModelPreference {
                    model_id: row.get(0)?,
                    is_favorite: row.get::<_, i32>(1)? != 0,
                    is_hidden: row.get::<_, i32>(2)? != 0,
                    custom_alias: row.get(3)?,
                    usage_count: row.get::<_, i32>(4)? as u32,
                    last_used_at: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(prefs)
    }

    /// 切换收藏状态
    pub async fn toggle_favorite(&self, model_id: &str) -> Result<bool, String> {
        let conn = self.db.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().timestamp();

        // 检查是否存在
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM user_model_preferences WHERE model_id = ?",
                params![model_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if exists {
            // 切换状态
            conn.execute(
                "UPDATE user_model_preferences
                 SET is_favorite = NOT is_favorite, updated_at = ?
                 WHERE model_id = ?",
                params![now, model_id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            // 创建新记录
            conn.execute(
                "INSERT INTO user_model_preferences
                 (model_id, is_favorite, is_hidden, usage_count, created_at, updated_at)
                 VALUES (?, 1, 0, 0, ?, ?)",
                params![model_id, now, now],
            )
            .map_err(|e| e.to_string())?;
        }

        // 返回新状态
        let new_state: bool = conn
            .query_row(
                "SELECT is_favorite FROM user_model_preferences WHERE model_id = ?",
                params![model_id],
                |row| Ok(row.get::<_, i32>(0)? != 0),
            )
            .unwrap_or(false);

        Ok(new_state)
    }

    /// 隐藏模型
    pub async fn hide_model(&self, model_id: &str) -> Result<(), String> {
        let conn = self.db.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO user_model_preferences
             (model_id, is_favorite, is_hidden, usage_count, created_at, updated_at)
             VALUES (?, 0, 1, 0, ?, ?)
             ON CONFLICT(model_id) DO UPDATE SET is_hidden = 1, updated_at = ?",
            params![model_id, now, now, now],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// 记录模型使用
    pub async fn record_usage(&self, model_id: &str) -> Result<(), String> {
        let conn = self.db.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO user_model_preferences
             (model_id, is_favorite, is_hidden, usage_count, last_used_at, created_at, updated_at)
             VALUES (?, 0, 0, 1, ?, ?, ?)
             ON CONFLICT(model_id) DO UPDATE SET
                usage_count = usage_count + 1,
                last_used_at = ?,
                updated_at = ?",
            params![model_id, now, now, now, now, now],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}
