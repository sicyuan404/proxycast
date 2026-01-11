//! 应用启动引导模块
//!
//! 包含配置验证、状态初始化等启动逻辑。

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent::NativeAgentState;
use crate::commands::api_key_provider_cmd::ApiKeyProviderServiceState;
use crate::commands::connect_cmd::ConnectStateWrapper;
use crate::commands::flow_monitor_cmd::{
    BatchOperationsState, BookmarkManagerState, EnhancedStatsServiceState, FlowInterceptorState,
    FlowMonitorState, FlowQueryServiceState, FlowReplayerState, QuickFilterManagerState,
    SessionManagerState,
};
use crate::commands::machine_id_cmd::MachineIdState;
use crate::commands::model_registry_cmd::ModelRegistryState;
use crate::commands::orchestrator_cmd::OrchestratorState;
use crate::commands::plugin_cmd::PluginManagerState;
use crate::commands::plugin_install_cmd::PluginInstallerState;
use crate::commands::provider_pool_cmd::{CredentialSyncServiceState, ProviderPoolServiceState};
use crate::commands::resilience_cmd::ResilienceConfigState;
use crate::commands::session_files_cmd::SessionFilesState;
use crate::commands::skill_cmd::SkillServiceState;
use crate::commands::terminal_cmd::TerminalManagerState;
use crate::commands::webview_cmd::{WebviewManagerState, WebviewManagerWrapper};
use crate::config::{self, Config, ConfigManager, GlobalConfigManager, GlobalConfigManagerState};
use crate::database::{self, DbConnection};
use crate::flow_monitor::{
    BatchOperations, BookmarkManager, EnhancedStatsService, FlowFileStore, FlowInterceptor,
    FlowMonitor, FlowMonitorConfig, FlowQueryService, FlowReplayer, InterceptConfig,
    QuickFilterManager, RotationConfig, SessionManager,
};
use crate::logger;
use crate::plugin;
use crate::server;
use crate::services::api_key_provider_service::ApiKeyProviderService;
use crate::services::provider_pool_service::ProviderPoolService;
use crate::services::skill_service::SkillService;
use crate::services::token_cache_service::TokenCacheService;
use crate::services::update_check_service::UpdateCheckServiceState;
use crate::telemetry;

use super::types::{AppState, LogState, TokenCacheServiceState};
use super::utils::{generate_api_key, is_non_local_bind, is_valid_bind_host};

/// 配置验证错误
#[derive(Debug)]
pub enum ConfigError {
    LoadFailed(String),
    SaveFailed(String),
    InvalidHost,
    DefaultApiKeyWithNonLocalBind,
    TlsNotSupported,
    RemoteManagementNotSupported,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::LoadFailed(e) => write!(f, "配置加载失败: {}", e),
            ConfigError::SaveFailed(e) => write!(f, "配置保存失败: {}", e),
            ConfigError::InvalidHost => {
                write!(
                    f,
                    "无效的监听地址。允许的地址：127.0.0.1、localhost、::1、0.0.0.0、::"
                )
            }
            ConfigError::DefaultApiKeyWithNonLocalBind => write!(
                f,
                "监听所有网络接口 (0.0.0.0 或 ::) 时，必须设置非默认的 API Key"
            ),
            ConfigError::TlsNotSupported => write!(f, "当前版本尚未支持 TLS"),
            ConfigError::RemoteManagementNotSupported => {
                write!(f, "远程管理需要 TLS 支持，当前版本未启用")
            }
        }
    }
}

/// 加载并验证配置
pub fn load_and_validate_config() -> Result<Config, ConfigError> {
    let mut config = config::load_config().map_err(|e| ConfigError::LoadFailed(e.to_string()))?;

    // 验证主机地址
    if !is_valid_bind_host(&config.server.host) {
        return Err(ConfigError::InvalidHost);
    }

    let is_non_local = is_non_local_bind(&config.server.host);

    // 如果是本地绑定且使用默认 API key，自动生成新密钥
    if !is_non_local && config.server.api_key == config::DEFAULT_API_KEY {
        let new_key = generate_api_key();
        config.server.api_key = new_key;
        config::save_config(&config).map_err(|e| ConfigError::SaveFailed(e.to_string()))?;
        tracing::info!("检测到默认 API key，已自动生成并保存新密钥");
    }

    // 如果是非本地绑定，必须使用非默认 API key
    if is_non_local && config.server.api_key == config::DEFAULT_API_KEY {
        return Err(ConfigError::DefaultApiKeyWithNonLocalBind);
    }

    // 检查 TLS 配置
    if config.server.tls.enable {
        return Err(ConfigError::TlsNotSupported);
    }

    // 检查远程管理配置
    if config.remote_management.allow_remote {
        return Err(ConfigError::RemoteManagementNotSupported);
    }

    Ok(config)
}

/// 应用状态集合
pub struct AppStates {
    pub state: AppState,
    pub logs: LogState,
    pub db: DbConnection,
    pub skill_service: SkillServiceState,
    pub provider_pool_service: ProviderPoolServiceState,
    pub api_key_provider_service: ApiKeyProviderServiceState,
    pub credential_sync_service: CredentialSyncServiceState,
    pub token_cache_service: TokenCacheServiceState,
    pub machine_id_service: MachineIdState,
    pub resilience_config: ResilienceConfigState,
    pub plugin_manager: PluginManagerState,
    pub plugin_installer: PluginInstallerState,
    pub plugin_rpc_manager: crate::commands::plugin_rpc_cmd::PluginRpcManagerState,
    pub telemetry: crate::commands::telemetry_cmd::TelemetryState,
    pub flow_monitor: FlowMonitorState,
    pub flow_query_service: FlowQueryServiceState,
    pub flow_interceptor: FlowInterceptorState,
    pub flow_replayer: FlowReplayerState,
    pub session_manager: SessionManagerState,
    pub quick_filter_manager: QuickFilterManagerState,
    pub bookmark_manager: BookmarkManagerState,
    pub enhanced_stats_service: EnhancedStatsServiceState,
    pub batch_operations: BatchOperationsState,
    pub native_agent: NativeAgentState,
    pub oauth_plugin_manager: crate::commands::oauth_plugin_cmd::OAuthPluginManagerState,
    pub orchestrator: OrchestratorState,
    pub connect_state: ConnectStateWrapper,
    pub model_registry: ModelRegistryState,
    pub global_config_manager: GlobalConfigManagerState,
    pub terminal_manager: TerminalManagerState,
    pub webview_manager: WebviewManagerWrapper,
    pub update_check_service: UpdateCheckServiceState,
    pub session_files: SessionFilesState,
    // 用于 setup hook 的共享实例
    pub shared_stats: Arc<parking_lot::RwLock<telemetry::StatsAggregator>>,
    pub shared_tokens: Arc<parking_lot::RwLock<telemetry::TokenTracker>>,
    pub shared_logger: Arc<telemetry::RequestLogger>,
    pub flow_monitor_arc: Arc<FlowMonitor>,
    pub flow_interceptor_arc: Arc<FlowInterceptor>,
}

/// 初始化所有应用状态
pub fn init_states(config: &Config) -> Result<AppStates, String> {
    // 核心状态
    let state: AppState = Arc::new(RwLock::new(server::ServerState::new(config.clone())));
    let logs: LogState = Arc::new(RwLock::new(logger::LogStore::with_config(&config.logging)));

    // 数据库
    let db = database::init_database().map_err(|e| format!("数据库初始化失败: {}", e))?;

    // 服务状态
    let skill_service =
        SkillService::new().map_err(|e| format!("SkillService 初始化失败: {}", e))?;
    let skill_service_state = SkillServiceState(Arc::new(skill_service));

    let provider_pool_service = ProviderPoolService::new();
    let provider_pool_service_state = ProviderPoolServiceState(Arc::new(provider_pool_service));

    let api_key_provider_service = ApiKeyProviderService::new();
    let api_key_provider_service_state =
        ApiKeyProviderServiceState(Arc::new(api_key_provider_service));

    let credential_sync_service_state = CredentialSyncServiceState(None);

    let token_cache_service = TokenCacheService::new();
    let token_cache_service_state = TokenCacheServiceState(Arc::new(token_cache_service));

    let machine_id_service = crate::services::machine_id_service::MachineIdService::new()
        .map_err(|e| format!("MachineIdService 初始化失败: {}", e))?;
    let machine_id_service_state: MachineIdState = Arc::new(RwLock::new(machine_id_service));

    let resilience_config_state = ResilienceConfigState::default();

    // 插件管理器
    let plugin_manager = plugin::PluginManager::with_defaults();
    let plugin_manager_state = PluginManagerState(Arc::new(RwLock::new(plugin_manager)));

    // 插件安装器
    let plugin_installer_state = init_plugin_installer()?;

    // 插件 RPC 管理器
    let plugin_rpc_manager_state = crate::commands::plugin_rpc_cmd::PluginRpcManagerState::new();

    // 遥测系统
    let (telemetry_state, shared_stats, shared_tokens, shared_logger) = init_telemetry(config)?;

    // Flow Monitor 系统（根据插件安装状态启用/禁用）
    let (
        flow_monitor_state,
        flow_query_service_state,
        flow_interceptor_state,
        flow_replayer_state,
        session_manager_state,
        quick_filter_manager_state,
        bookmark_manager_state,
        enhanced_stats_service_state,
        batch_operations_state,
        flow_monitor_arc,
        flow_interceptor_arc,
    ) = init_flow_monitor(&provider_pool_service_state, &db, &plugin_installer_state)?;

    // 其他状态
    let native_agent_state = NativeAgentState::new();
    let oauth_plugin_manager_state =
        crate::commands::oauth_plugin_cmd::OAuthPluginManagerState::with_defaults();
    let orchestrator_state = OrchestratorState::new();

    // 初始化 Connect 状态（延迟初始化，在 setup hook 中完成）
    let connect_state = ConnectStateWrapper(Arc::new(RwLock::new(None)));

    // 初始化 Model Registry 状态（延迟初始化，在 setup hook 中完成）
    let model_registry_state: ModelRegistryState = Arc::new(RwLock::new(None));

    // 初始化终端管理器状态（延迟初始化，在 setup hook 中完成）
    let terminal_manager_state = TerminalManagerState(Arc::new(RwLock::new(None)));

    // 初始化 Webview 管理器状态
    let webview_manager_state =
        WebviewManagerWrapper(Arc::new(RwLock::new(WebviewManagerState::new())));

    // 初始化更新检查服务
    let update_check_service_state = UpdateCheckServiceState::new();

    // 初始化会话文件存储
    let session_files_storage = crate::session_files::SessionFileStorage::new()
        .map_err(|e| format!("SessionFileStorage 初始化失败: {}", e))?;
    let session_files_state = SessionFilesState(std::sync::Mutex::new(session_files_storage));

    // 初始化全局配置管理器
    let config_path = ConfigManager::default_config_path();
    let global_config_manager = GlobalConfigManager::new(config.clone(), config_path);
    let global_config_manager_state = GlobalConfigManagerState::new(global_config_manager);

    // 初始化默认技能仓库
    {
        let conn = db.lock().expect("Failed to lock database");
        database::dao::skills::SkillDao::init_default_skill_repos(&conn)
            .map_err(|e| format!("初始化默认技能仓库失败: {}", e))?;
    }

    Ok(AppStates {
        state,
        logs,
        db,
        skill_service: skill_service_state,
        provider_pool_service: provider_pool_service_state,
        api_key_provider_service: api_key_provider_service_state,
        credential_sync_service: credential_sync_service_state,
        token_cache_service: token_cache_service_state,
        machine_id_service: machine_id_service_state,
        resilience_config: resilience_config_state,
        plugin_manager: plugin_manager_state,
        plugin_installer: plugin_installer_state,
        plugin_rpc_manager: plugin_rpc_manager_state,
        telemetry: telemetry_state,
        flow_monitor: flow_monitor_state,
        flow_query_service: flow_query_service_state,
        flow_interceptor: flow_interceptor_state,
        flow_replayer: flow_replayer_state,
        session_manager: session_manager_state,
        quick_filter_manager: quick_filter_manager_state,
        bookmark_manager: bookmark_manager_state,
        enhanced_stats_service: enhanced_stats_service_state,
        batch_operations: batch_operations_state,
        native_agent: native_agent_state,
        oauth_plugin_manager: oauth_plugin_manager_state,
        orchestrator: orchestrator_state,
        connect_state,
        model_registry: model_registry_state,
        global_config_manager: global_config_manager_state,
        terminal_manager: terminal_manager_state,
        webview_manager: webview_manager_state,
        update_check_service: update_check_service_state,
        session_files: session_files_state,
        shared_stats,
        shared_tokens,
        shared_logger,
        flow_monitor_arc,
        flow_interceptor_arc,
    })
}

/// 初始化插件安装器
fn init_plugin_installer() -> Result<PluginInstallerState, String> {
    let db_path = database::get_db_path().map_err(|e| format!("获取数据库路径失败: {}", e))?;
    let plugins_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("proxycast")
        .join("plugins");
    let temp_dir = std::env::temp_dir().join("proxycast_plugin_install");

    let _ = std::fs::create_dir_all(&plugins_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    match plugin::installer::PluginInstaller::from_paths(
        plugins_dir.clone(),
        temp_dir.clone(),
        &db_path,
    ) {
        Ok(installer) => {
            tracing::info!("[启动] 插件安装器初始化成功");
            Ok(PluginInstallerState(Arc::new(RwLock::new(installer))))
        }
        Err(e) => {
            tracing::error!("[启动] 插件安装器初始化失败: {}", e);
            // 使用临时目录作为后备
            let fallback_plugins_dir = std::env::temp_dir().join("proxycast_plugins_fallback");
            let fallback_temp_dir = std::env::temp_dir().join("proxycast_plugin_install_fallback");
            let _ = std::fs::create_dir_all(&fallback_plugins_dir);
            let _ = std::fs::create_dir_all(&fallback_temp_dir);
            let installer = plugin::installer::PluginInstaller::from_paths(
                fallback_plugins_dir,
                fallback_temp_dir,
                &db_path,
            )
            .map_err(|e| format!("后备插件安装器初始化失败: {}", e))?;
            Ok(PluginInstallerState(Arc::new(RwLock::new(installer))))
        }
    }
}

/// 初始化遥测系统
fn init_telemetry(
    config: &Config,
) -> Result<
    (
        crate::commands::telemetry_cmd::TelemetryState,
        Arc<parking_lot::RwLock<telemetry::StatsAggregator>>,
        Arc<parking_lot::RwLock<telemetry::TokenTracker>>,
        Arc<telemetry::RequestLogger>,
    ),
    String,
> {
    let shared_stats = Arc::new(parking_lot::RwLock::new(
        telemetry::StatsAggregator::with_defaults(),
    ));
    let shared_tokens = Arc::new(parking_lot::RwLock::new(
        telemetry::TokenTracker::with_defaults(),
    ));
    let log_rotation = telemetry::LogRotationConfig {
        max_memory_logs: 10000,
        retention_days: config.logging.retention_days,
        max_file_size: 10 * 1024 * 1024,
        enable_file_logging: config.logging.enabled,
    };
    let shared_logger = Arc::new(
        telemetry::RequestLogger::new(log_rotation)
            .map_err(|e| format!("RequestLogger 初始化失败: {}", e))?,
    );

    let telemetry_state = crate::commands::telemetry_cmd::TelemetryState::with_shared(
        shared_stats.clone(),
        shared_tokens.clone(),
        Some(shared_logger.clone()),
    )
    .map_err(|e| format!("TelemetryState 初始化失败: {}", e))?;

    Ok((telemetry_state, shared_stats, shared_tokens, shared_logger))
}

/// 初始化 Flow Monitor 系统
///
/// 如果 flow-monitor 插件已安装，则启用监控功能；否则禁用。
#[allow(clippy::type_complexity)]
fn init_flow_monitor(
    provider_pool_service_state: &ProviderPoolServiceState,
    db: &DbConnection,
    plugin_installer_state: &PluginInstallerState,
) -> Result<
    (
        FlowMonitorState,
        FlowQueryServiceState,
        FlowInterceptorState,
        FlowReplayerState,
        SessionManagerState,
        QuickFilterManagerState,
        BookmarkManagerState,
        EnhancedStatsServiceState,
        BatchOperationsState,
        Arc<FlowMonitor>,
        Arc<FlowInterceptor>,
    ),
    String,
> {
    // 检查 flow-monitor 插件是否已安装
    let is_plugin_installed = {
        let installer = plugin_installer_state.0.blocking_read();
        installer.is_installed("flow-monitor").unwrap_or(false)
    };

    // 根据插件安装状态设置 enabled
    let mut flow_monitor_config = FlowMonitorConfig::default();
    flow_monitor_config.enabled = is_plugin_installed;

    if is_plugin_installed {
        tracing::info!("[启动] flow-monitor 插件已安装，启用 Flow 监控");
    } else {
        tracing::info!("[启动] flow-monitor 插件未安装，禁用 Flow 监控");
    }

    // 初始化文件存储
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("proxycast")
        .join("flows");
    let _ = std::fs::create_dir_all(&data_dir);

    let rotation_config = RotationConfig::default();
    let flow_file_store = match FlowFileStore::new(data_dir, rotation_config.clone()) {
        Ok(store) => Some(Arc::new(store)),
        Err(e) => {
            tracing::warn!("无法初始化 Flow 文件存储: {}", e);
            None
        }
    };

    let flow_monitor = Arc::new(FlowMonitor::new(
        flow_monitor_config,
        flow_file_store.clone(),
    ));
    let flow_monitor_state = FlowMonitorState(flow_monitor.clone());

    let flow_interceptor = Arc::new(FlowInterceptor::new(InterceptConfig::default()));
    let flow_interceptor_state = FlowInterceptorState(flow_interceptor.clone());

    let flow_replayer = Arc::new(FlowReplayer::new(
        flow_monitor.clone(),
        provider_pool_service_state.0.clone(),
        db.clone(),
    ));
    let flow_replayer_state = FlowReplayerState(flow_replayer);

    let db_path = database::get_db_path().map_err(|e| format!("获取数据库路径失败: {}", e))?;

    let session_manager = Arc::new(
        SessionManager::new(db_path.clone())
            .map_err(|e| format!("SessionManager 初始化失败: {}", e))?,
    );
    let session_manager_state = SessionManagerState(session_manager.clone());

    let quick_filter_manager = Arc::new(
        QuickFilterManager::new(db_path.clone())
            .map_err(|e| format!("QuickFilterManager 初始化失败: {}", e))?,
    );
    let quick_filter_manager_state = QuickFilterManagerState(quick_filter_manager);

    let bookmark_manager = Arc::new(
        BookmarkManager::new(db_path).map_err(|e| format!("BookmarkManager 初始化失败: {}", e))?,
    );
    let bookmark_manager_state = BookmarkManagerState(bookmark_manager);

    let enhanced_stats_service = Arc::new(EnhancedStatsService::new(flow_monitor.memory_store()));
    let enhanced_stats_service_state = EnhancedStatsServiceState(enhanced_stats_service);

    let batch_operations = Arc::new(BatchOperations::new(
        flow_monitor.clone(),
        Some(session_manager_state.0.clone()),
    ));
    let batch_operations_state = BatchOperationsState(batch_operations);

    // FlowQueryService
    let flow_query_service_state = if let Some(file_store) = flow_file_store {
        let query_service = FlowQueryService::new(flow_monitor.memory_store(), file_store);
        FlowQueryServiceState(Arc::new(query_service))
    } else {
        let temp_dir = std::env::temp_dir().join("proxycast_flows");
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_store = FlowFileStore::new(temp_dir, rotation_config)
            .map_err(|e| format!("临时 FlowFileStore 初始化失败: {}", e))?;
        let query_service =
            FlowQueryService::new(flow_monitor.memory_store(), Arc::new(temp_store));
        FlowQueryServiceState(Arc::new(query_service))
    };

    Ok((
        flow_monitor_state,
        flow_query_service_state,
        flow_interceptor_state,
        flow_replayer_state,
        session_manager_state,
        quick_filter_manager_state,
        bookmark_manager_state,
        enhanced_stats_service_state,
        batch_operations_state,
        flow_monitor,
        flow_interceptor,
    ))
}
