//! 应用运行器模块
//!
//! 包含 Tauri 应用的主入口函数和命令注册。

use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};

use crate::commands;
use crate::tray::{TrayIconStatus, TrayManager, TrayStateSnapshot};

use super::bootstrap::{self, AppStates};
use super::commands as app_commands;
use super::types::{AppState, TrayManagerState};

/// 运行 Tauri 应用
///
/// 这是应用的主入口点，负责：
/// 1. 加载和验证配置
/// 2. 初始化所有应用状态
/// 3. 配置 Tauri Builder（插件、状态管理、事件处理）
/// 4. 注册所有 Tauri 命令
/// 5. 启动应用
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 加载并验证配置
    let config = match bootstrap::load_and_validate_config() {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!("{}", err);
            eprintln!("{}", err);
            return;
        }
    };

    // 初始化所有应用状态
    let states = match bootstrap::init_states(&config) {
        Ok(s) => s,
        Err(err) => {
            tracing::error!("应用状态初始化失败: {}", err);
            eprintln!("应用状态初始化失败: {}", err);
            return;
        }
    };

    // 解构状态以便使用
    let AppStates {
        state,
        logs,
        db,
        skill_service: skill_service_state,
        provider_pool_service: provider_pool_service_state,
        api_key_provider_service: api_key_provider_service_state,
        credential_sync_service: credential_sync_service_state,
        token_cache_service: token_cache_service_state,
        machine_id_service: machine_id_service_state,
        router_config: router_config_state,
        resilience_config: resilience_config_state,
        plugin_manager: plugin_manager_state,
        plugin_installer: plugin_installer_state,
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
        connect_state: connect_state,
        model_registry: model_registry_state,
        shared_stats,
        shared_tokens,
        shared_logger,
        flow_monitor_arc: flow_monitor,
        flow_interceptor_arc: flow_interceptor,
    } = states;

    // Clone for setup hook
    let state_clone = state.clone();
    let logs_clone = logs.clone();
    let db_clone = db.clone();
    let pool_service_clone = provider_pool_service_state.0.clone();
    let token_cache_clone = token_cache_service_state.0.clone();
    let shared_stats_clone = shared_stats.clone();
    let shared_tokens_clone = shared_tokens.clone();
    let shared_logger_clone = shared_logger.clone();
    let flow_monitor_clone = flow_monitor.clone();
    let flow_interceptor_clone = flow_interceptor.clone();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ));

    // 在 macOS 上注册 Deep Link 插件
    // _Requirements: 1.4_
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_plugin_deep_link::init());
    }

    builder = builder
        // 单实例插件：当第二个实例启动时，将 URL 传递给第一个实例
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!("[单实例] 收到来自新实例的参数: {:?}", args);

            // 将窗口带到前台
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));

    builder
        .manage(state)
        .manage(logs)
        .manage(db)
        .manage(skill_service_state)
        .manage(provider_pool_service_state)
        .manage(api_key_provider_service_state)
        .manage(credential_sync_service_state)
        .manage(token_cache_service_state)
        .manage(machine_id_service_state)
        .manage(router_config_state)
        .manage(resilience_config_state)
        .manage(telemetry_state)
        .manage(plugin_manager_state)
        .manage(plugin_installer_state)
        .manage(flow_monitor_state)
        .manage(flow_query_service_state)
        .manage(flow_interceptor_state)
        .manage(flow_replayer_state)
        .manage(session_manager_state)
        .manage(quick_filter_manager_state)
        .manage(bookmark_manager_state)
        .manage(enhanced_stats_service_state)
        .manage(batch_operations_state)
        .manage(native_agent_state)
        .manage(oauth_plugin_manager_state)
        .manage(orchestrator_state)
        .manage(connect_state)
        .manage(model_registry_state)
        .on_window_event(move |window, event| {
            // 处理窗口关闭事件
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 获取配置，检查是否启用最小化到托盘
                let app_handle = window.app_handle();
                if let Some(app_state) = app_handle.try_state::<AppState>() {
                    // 使用 block_on 同步获取配置
                    let minimize_to_tray = tauri::async_runtime::block_on(async {
                        let state = app_state.read().await;
                        state.config.minimize_to_tray
                    });

                    if minimize_to_tray {
                        // 阻止默认关闭行为
                        api.prevent_close();
                        // 隐藏窗口而不是关闭
                        if let Err(e) = window.hide() {
                            tracing::error!("[窗口] 隐藏窗口失败: {}", e);
                        } else {
                            tracing::info!("[窗口] 窗口已最小化到托盘");
                        }
                    }
                }
            }
        })
        .setup(move |app| {
            // 初始化托盘管理器
            // Requirements 1.4: 应用启动时显示停止状态图标
            match TrayManager::new(app.handle()) {
                Ok(tray_manager) => {
                    tracing::info!("[启动] 托盘管理器初始化成功");
                    // 将托盘管理器存储到应用状态中
                    let tray_state: TrayManagerState<tauri::Wry> =
                        TrayManagerState(Arc::new(tokio::sync::RwLock::new(Some(tray_manager))));
                    app.manage(tray_state);
                }
                Err(e) => {
                    tracing::error!("[启动] 托盘管理器初始化失败: {}", e);
                    // 即使托盘初始化失败，应用仍然可以运行
                    let tray_state: TrayManagerState<tauri::Wry> =
                        TrayManagerState(Arc::new(tokio::sync::RwLock::new(None)));
                    app.manage(tray_state);
                }
            }

            // 初始化 Connect 状态
            // _Requirements: 1.4, 2.1_
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // 获取应用数据目录
                    let app_data_dir = dirs::data_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join("proxycast");

                    // 初始化 Connect 状态
                    match crate::commands::connect_cmd::init_connect_state(app_data_dir).await {
                        Ok(connect_state_inner) => {
                            tracing::info!("[启动] Connect 模块初始化成功");
                            // 更新状态
                            if let Some(state) = app_handle
                                .try_state::<crate::commands::connect_cmd::ConnectStateWrapper>()
                            {
                                let mut guard = state.0.write().await;
                                *guard = Some(connect_state_inner);
                            }
                        }
                        Err(e) => {
                            tracing::error!("[启动] Connect 模块初始化失败: {:?}", e);
                        }
                    }
                });
            }

            // 初始化 Model Registry 服务
            {
                let app_handle = app.handle().clone();
                let db_clone = db_clone.clone();
                tauri::async_runtime::spawn(async move {
                    // 创建 ModelRegistryService
                    let service = crate::services::model_registry_service::ModelRegistryService::new(db_clone);

                    // 初始化服务
                    match service.initialize().await {
                        Ok(()) => {
                            tracing::info!("[启动] Model Registry 服务初始化成功");
                            // 更新状态
                            if let Some(state) = app_handle
                                .try_state::<crate::commands::model_registry_cmd::ModelRegistryState>()
                            {
                                let mut guard = state.write().await;
                                *guard = Some(service);
                            }
                        }
                        Err(e) => {
                            tracing::error!("[启动] Model Registry 服务初始化失败: {}", e);
                        }
                    }
                });
            }

            // 注册 Deep Link 事件处理器（仅 macOS）
            // _Requirements: 1.4_
            #[cfg(target_os = "macos")]
            {
                let app_handle = app.handle().clone();
                app.listen("deep-link://new-url", move |event| {
                    let urls = event.payload().to_string();
                    tracing::info!("[Deep Link] 收到 URL: {}", urls);
                    // 解析 URL 并处理
                    let app_handle_clone = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                            // 尝试解析为 JSON 数组（Tauri deep-link 插件返回的格式）
                            if let Ok(url_list) = serde_json::from_str::<Vec<String>>(&urls) {
                                for url in url_list {
                                    if url.starts_with("proxycast://connect") {
                                        // 调用 handle_deep_link 命令
                                        if let Some(state) = app_handle_clone
                                            .try_state::<crate::commands::connect_cmd::ConnectStateWrapper>()
                                        {
                                            match crate::connect::parse_deep_link(&url) {
                                                Ok(payload) => {
                                                    // 查询中转商信息
                                                    let (relay_info, is_verified) = {
                                                        let state_guard = state.0.read().await;
                                                        if let Some(connect_state) = state_guard.as_ref() {
                                                            let info = connect_state.registry.get(&payload.relay);
                                                            let verified = info.is_some();
                                                            (info, verified)
                                                        } else {
                                                            (None, false)
                                                        }
                                                    };

                                                    let result = crate::commands::connect_cmd::DeepLinkResult {
                                                        payload,
                                                        relay_info,
                                                        is_verified,
                                                    };

                                                    // 发送事件到前端
                                                    if let Err(e) = app_handle_clone.emit("deep-link-connect", &result) {
                                                        tracing::error!("[Deep Link] 发送事件失败: {}", e);
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::error!("[Deep Link] 解析 URL 失败: {:?}", e);
                                                    // 发送错误事件到前端
                                                    let _ = app_handle_clone.emit("deep-link-error", &format!("{:?}", e));
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if urls.starts_with("proxycast://connect") {
                                // 直接处理单个 URL
                                if let Some(state) = app_handle_clone
                                    .try_state::<crate::commands::connect_cmd::ConnectStateWrapper>()
                                {
                                    match crate::connect::parse_deep_link(&urls) {
                                        Ok(payload) => {
                                            let (relay_info, is_verified) = {
                                                let state_guard = state.0.read().await;
                                                if let Some(connect_state) = state_guard.as_ref() {
                                                    let info = connect_state.registry.get(&payload.relay);
                                                    let verified = info.is_some();
                                                    (info, verified)
                                                } else {
                                                    (None, false)
                                                }
                                            };

                                            let result = crate::commands::connect_cmd::DeepLinkResult {
                                                payload,
                                                relay_info,
                                                is_verified,
                                            };

                                            if let Err(e) = app_handle_clone.emit("deep-link-connect", &result) {
                                                tracing::error!("[Deep Link] 发送事件失败: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("[Deep Link] 解析 URL 失败: {:?}", e);
                                            let _ = app_handle_clone.emit("deep-link-error", &format!("{:?}", e));
                                        }
                                    }
                                }
                            }
                        });
                });
            }

            // 自动启动服务器
            let state = state_clone.clone();
            let logs = logs_clone.clone();
            let db = db_clone.clone();
            let pool_service = pool_service_clone.clone();
            let token_cache = token_cache_clone.clone();
            let shared_stats = shared_stats_clone.clone();
            let shared_tokens = shared_tokens_clone.clone();
            let shared_logger = shared_logger_clone.clone();
            let shared_flow_monitor = flow_monitor_clone.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // 先加载凭证池中的凭证
                {
                    logs.write().await.add("info", "[启动] 正在加载凭证池...");

                    // 获取凭证池概览信息
                    match pool_service.get_overview(&db) {
                        Ok(overview) => {
                            let mut loaded_types = Vec::new();
                            let mut total_credentials = 0;

                            for provider_overview in overview {
                                let count = provider_overview.stats.total_count;
                                if count > 0 {
                                    total_credentials += count;
                                    let provider_name =
                                        match provider_overview.provider_type.as_str() {
                                            "kiro" => "Kiro",
                                            "gemini" => "Gemini",
                                            "qwen" => "通义千问",
                                            "antigravity" => "Antigravity",
                                            "openai" => "OpenAI",
                                            "claude" => "Claude",
                                            "codex" => "Codex",
                                            "claude_oauth" => "Claude OAuth",
                                            "iflow" => "iFlow",
                                            _ => &provider_overview.provider_type,
                                        };
                                    loaded_types.push(format!("{} ({} 个)", provider_name, count));
                                }
                            }

                            if loaded_types.is_empty() {
                                logs.write().await.add("warn", "[启动] 未找到任何可用凭证");
                            } else {
                                let message = format!(
                                    "[启动] 凭证已加载: {} (共 {} 个)",
                                    loaded_types.join(", "),
                                    total_credentials
                                );
                                logs.write().await.add("info", &message);
                            }
                        }
                        Err(e) => {
                            logs.write()
                                .await
                                .add("warn", &format!("[启动] 获取凭证池信息失败: {}", e));
                        }
                    }

                    // 兼容性：仍然尝试加载旧的 Kiro 凭证（如果存在）
                    let mut s = state.write().await;
                    if let Err(e) = s.kiro_provider.load_credentials().await {
                        logs.write()
                            .await
                            .add("debug", &format!("[启动] 旧版 Kiro 凭证加载失败: {e}"));
                    }
                }
                // 启动服务器（使用共享的遥测实例和 Flow Monitor）
                let server_started;
                let server_address;
                {
                    let mut s = state.write().await;
                    logs.write()
                        .await
                        .add("info", "[启动] 正在自动启动服务器...");
                    match s
                        .start_with_telemetry_and_flow_monitor(
                            logs.clone(),
                            pool_service,
                            token_cache,
                            Some(db),
                            Some(shared_stats),
                            Some(shared_tokens),
                            Some(shared_logger),
                            Some(shared_flow_monitor),
                            Some(flow_interceptor_clone),
                        )
                        .await
                    {
                        Ok(_) => {
                            let host = s.config.server.host.clone();
                            let port = s.config.server.port;
                            logs.write()
                                .await
                                .add("info", &format!("[启动] 服务器已启动: {host}:{port}"));
                            server_started = true;
                            server_address = format!("{}:{}", host, port);
                        }
                        Err(e) => {
                            logs.write()
                                .await
                                .add("error", &format!("[启动] 服务器启动失败: {e}"));
                            server_started = false;
                            server_address = String::new();
                        }
                    }
                }

                // 更新托盘状态
                // Requirements 7.1: API 服务器状态变化时更新托盘图标
                if let Some(tray_state) = app_handle.try_state::<TrayManagerState<tauri::Wry>>() {
                    let tray_guard = tray_state.0.read().await;
                    if let Some(tray_manager) = tray_guard.as_ref() {
                        // 计算初始图标状态
                        // 服务器刚启动时，假设凭证健康（后续会通过状态同步更新）
                        let icon_status = if server_started {
                            TrayIconStatus::Running
                        } else {
                            TrayIconStatus::Stopped
                        };

                        let snapshot = TrayStateSnapshot {
                            icon_status,
                            server_running: server_started,
                            server_address,
                            available_credentials: 0, // 初始值，后续通过状态同步更新
                            total_credentials: 0,
                            today_requests: 0,
                            auto_start_enabled: false, // 后续通过状态同步更新
                        };

                        if let Err(e) = tray_manager.update_state(snapshot).await {
                            tracing::error!("[启动] 更新托盘状态失败: {}", e);
                        } else {
                            tracing::info!("[启动] 托盘状态已更新");
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Server commands (from app::commands)
            app_commands::start_server,
            app_commands::stop_server,
            app_commands::get_server_status,
            // Config commands (from app::commands)
            app_commands::get_config,
            app_commands::save_config,
            app_commands::get_default_provider,
            app_commands::set_default_provider,
            app_commands::get_endpoint_providers,
            app_commands::set_endpoint_provider,
            // Unified OAuth commands (new)
            commands::oauth_cmd::get_oauth_credentials,
            commands::oauth_cmd::reload_oauth_credentials,
            commands::oauth_cmd::refresh_oauth_token,
            commands::oauth_cmd::get_oauth_env_variables,
            commands::oauth_cmd::get_oauth_token_file_hash,
            commands::oauth_cmd::check_and_reload_oauth_credentials,
            commands::oauth_cmd::get_all_oauth_credentials,
            // Legacy Kiro commands (from app::commands, deprecated)
            app_commands::refresh_kiro_token,
            app_commands::reload_credentials,
            app_commands::get_kiro_credentials,
            app_commands::get_env_variables,
            app_commands::get_token_file_hash,
            app_commands::check_and_reload_credentials,
            // Legacy Gemini commands (from app::commands, deprecated)
            app_commands::get_gemini_credentials,
            app_commands::reload_gemini_credentials,
            app_commands::refresh_gemini_token,
            app_commands::get_gemini_env_variables,
            app_commands::get_gemini_token_file_hash,
            app_commands::check_and_reload_gemini_credentials,
            // Legacy Qwen commands (from app::commands, deprecated)
            app_commands::get_qwen_credentials,
            app_commands::reload_qwen_credentials,
            app_commands::refresh_qwen_token,
            app_commands::get_qwen_env_variables,
            app_commands::get_qwen_token_file_hash,
            app_commands::check_and_reload_qwen_credentials,
            // OpenAI Custom commands (from app::commands)
            app_commands::get_openai_custom_status,
            app_commands::set_openai_custom_config,
            // Claude Custom commands (from app::commands)
            app_commands::get_claude_custom_status,
            app_commands::set_claude_custom_config,
            // Log commands (from app::commands)
            app_commands::get_logs,
            app_commands::clear_logs,
            // API test commands (from app::commands)
            app_commands::test_api,
            app_commands::get_available_models,
            app_commands::check_api_compatibility,
            // Switch commands
            commands::switch_cmd::get_switch_providers,
            commands::switch_cmd::get_current_switch_provider,
            commands::switch_cmd::add_switch_provider,
            commands::switch_cmd::update_switch_provider,
            commands::switch_cmd::delete_switch_provider,
            commands::switch_cmd::switch_provider,
            commands::switch_cmd::import_default_config,
            commands::switch_cmd::read_live_provider_settings,
            commands::switch_cmd::check_config_sync_status,
            commands::switch_cmd::sync_from_external_config,
            // Config commands
            commands::config_cmd::get_config_status,
            commands::config_cmd::get_config_dir_path,
            commands::config_cmd::open_config_folder,
            commands::config_cmd::get_tool_versions,
            commands::config_cmd::get_auto_launch_status,
            commands::config_cmd::set_auto_launch,
            // Config import/export commands
            commands::config_cmd::export_config,
            commands::config_cmd::validate_config_yaml,
            commands::config_cmd::import_config,
            commands::config_cmd::get_config_paths,
            // Enhanced export/import commands (using ExportService/ImportService)
            commands::config_cmd::export_bundle,
            commands::config_cmd::export_config_yaml,
            commands::config_cmd::validate_import,
            commands::config_cmd::import_bundle,
            // Path utility commands
            commands::config_cmd::expand_path,
            commands::config_cmd::open_auth_dir,
            commands::config_cmd::check_for_updates,
            commands::config_cmd::download_update,
            // MCP commands
            commands::mcp_cmd::get_mcp_servers,
            commands::mcp_cmd::add_mcp_server,
            commands::mcp_cmd::update_mcp_server,
            commands::mcp_cmd::delete_mcp_server,
            commands::mcp_cmd::toggle_mcp_server,
            commands::mcp_cmd::import_mcp_from_app,
            commands::mcp_cmd::sync_all_mcp_to_live,
            // Prompt commands
            commands::prompt_cmd::get_prompts,
            commands::prompt_cmd::upsert_prompt,
            commands::prompt_cmd::add_prompt,
            commands::prompt_cmd::update_prompt,
            commands::prompt_cmd::delete_prompt,
            commands::prompt_cmd::enable_prompt,
            commands::prompt_cmd::import_prompt_from_file,
            commands::prompt_cmd::get_current_prompt_file_content,
            commands::prompt_cmd::auto_import_prompt,
            commands::prompt_cmd::switch_prompt,
            // Skill commands
            commands::skill_cmd::get_skills,
            commands::skill_cmd::get_skills_for_app,
            commands::skill_cmd::install_skill,
            commands::skill_cmd::install_skill_for_app,
            commands::skill_cmd::uninstall_skill,
            commands::skill_cmd::uninstall_skill_for_app,
            commands::skill_cmd::get_skill_repos,
            commands::skill_cmd::add_skill_repo,
            commands::skill_cmd::remove_skill_repo,
            commands::skill_cmd::get_installed_proxycast_skills,
            // Provider Pool commands
            commands::provider_pool_cmd::get_provider_pool_overview,
            commands::provider_pool_cmd::get_provider_pool_credentials,
            commands::provider_pool_cmd::add_provider_pool_credential,
            commands::provider_pool_cmd::update_provider_pool_credential,
            commands::provider_pool_cmd::delete_provider_pool_credential,
            commands::provider_pool_cmd::toggle_provider_pool_credential,
            commands::provider_pool_cmd::reset_provider_pool_credential,
            commands::provider_pool_cmd::reset_provider_pool_health,
            commands::provider_pool_cmd::check_provider_pool_credential_health,
            commands::provider_pool_cmd::check_provider_pool_type_health,
            commands::provider_pool_cmd::add_kiro_oauth_credential,
            commands::provider_pool_cmd::add_kiro_from_json,
            commands::provider_pool_cmd::add_gemini_oauth_credential,
            commands::provider_pool_cmd::add_qwen_oauth_credential,
            commands::provider_pool_cmd::add_antigravity_oauth_credential,
            commands::provider_pool_cmd::add_openai_key_credential,
            commands::provider_pool_cmd::add_claude_key_credential,
            commands::provider_pool_cmd::add_gemini_api_key_credential,
            commands::provider_pool_cmd::add_codex_oauth_credential,
            commands::provider_pool_cmd::add_claude_oauth_credential,
            commands::provider_pool_cmd::add_iflow_oauth_credential,
            commands::provider_pool_cmd::add_iflow_cookie_credential,
            commands::provider_pool_cmd::refresh_pool_credential_token,
            commands::provider_pool_cmd::get_pool_credential_oauth_status,
            commands::provider_pool_cmd::debug_kiro_credentials,
            commands::provider_pool_cmd::test_user_credentials,
            commands::provider_pool_cmd::migrate_private_config_to_pool,
            commands::provider_pool_cmd::start_antigravity_oauth_login,
            commands::provider_pool_cmd::get_antigravity_auth_url_and_wait,
            commands::provider_pool_cmd::get_codex_auth_url_and_wait,
            commands::provider_pool_cmd::start_codex_oauth_login,
            commands::provider_pool_cmd::get_claude_oauth_auth_url_and_wait,
            commands::provider_pool_cmd::start_claude_oauth_login,
            commands::provider_pool_cmd::exchange_claude_oauth_code,
            commands::provider_pool_cmd::claude_oauth_with_cookie,
            commands::provider_pool_cmd::get_qwen_device_code_and_wait,
            commands::provider_pool_cmd::start_qwen_device_code_login,
            commands::provider_pool_cmd::get_iflow_auth_url_and_wait,
            commands::provider_pool_cmd::start_iflow_oauth_login,
            commands::provider_pool_cmd::get_gemini_auth_url_and_wait,
            commands::provider_pool_cmd::start_gemini_oauth_login,
            commands::provider_pool_cmd::exchange_gemini_code,
            commands::provider_pool_cmd::get_kiro_credential_fingerprint,
            commands::provider_pool_cmd::get_credential_health,
            commands::provider_pool_cmd::get_all_credential_health,
            // Kiro Builder ID 登录命令
            commands::provider_pool_cmd::start_kiro_builder_id_login,
            commands::provider_pool_cmd::poll_kiro_builder_id_auth,
            commands::provider_pool_cmd::cancel_kiro_builder_id_login,
            commands::provider_pool_cmd::add_kiro_from_builder_id_auth,
            // Kiro Social Auth 登录命令 (Google/GitHub)
            commands::provider_pool_cmd::start_kiro_social_auth_login,
            commands::provider_pool_cmd::exchange_kiro_social_auth_token,
            commands::provider_pool_cmd::cancel_kiro_social_auth_login,
            commands::provider_pool_cmd::start_kiro_social_auth_callback_server,
            // Playwright 指纹浏览器登录命令
            commands::provider_pool_cmd::check_playwright_available,
            commands::provider_pool_cmd::install_playwright,
            commands::provider_pool_cmd::start_kiro_playwright_login,
            commands::provider_pool_cmd::cancel_kiro_playwright_login,
            // API Key Provider commands
            commands::api_key_provider_cmd::get_api_key_providers,
            commands::api_key_provider_cmd::get_api_key_provider,
            commands::api_key_provider_cmd::add_custom_api_key_provider,
            commands::api_key_provider_cmd::update_api_key_provider,
            commands::api_key_provider_cmd::delete_custom_api_key_provider,
            commands::api_key_provider_cmd::add_api_key,
            commands::api_key_provider_cmd::delete_api_key,
            commands::api_key_provider_cmd::toggle_api_key,
            commands::api_key_provider_cmd::update_api_key_alias,
            commands::api_key_provider_cmd::get_next_api_key,
            commands::api_key_provider_cmd::record_api_key_usage,
            commands::api_key_provider_cmd::record_api_key_error,
            commands::api_key_provider_cmd::get_provider_ui_state,
            commands::api_key_provider_cmd::set_provider_ui_state,
            commands::api_key_provider_cmd::update_provider_sort_orders,
            commands::api_key_provider_cmd::export_api_key_providers,
            commands::api_key_provider_cmd::import_api_key_providers,
            // Legacy API Key migration commands
            commands::api_key_provider_cmd::get_legacy_api_key_credentials,
            commands::api_key_provider_cmd::migrate_legacy_api_key_credentials,
            commands::api_key_provider_cmd::delete_legacy_api_key_credential,
            // Route commands
            commands::route_cmd::get_available_routes,
            commands::route_cmd::get_route_curl_examples,
            // Router config commands
            commands::router_cmd::get_routing_rules,
            commands::router_cmd::add_routing_rule,
            commands::router_cmd::remove_routing_rule,
            commands::router_cmd::update_routing_rule,
            commands::router_cmd::get_exclusions,
            commands::router_cmd::add_exclusion,
            commands::router_cmd::remove_exclusion,
            commands::router_cmd::set_router_default_provider,
            commands::router_cmd::clear_all_routing_config,
            // Resilience config commands
            commands::resilience_cmd::get_retry_config,
            commands::resilience_cmd::update_retry_config,
            commands::resilience_cmd::get_failover_config,
            commands::resilience_cmd::update_failover_config,
            commands::resilience_cmd::get_switch_log,
            commands::resilience_cmd::clear_switch_log,
            // Telemetry commands
            commands::telemetry_cmd::get_request_logs,
            commands::telemetry_cmd::get_request_log_detail,
            commands::telemetry_cmd::clear_request_logs,
            commands::telemetry_cmd::get_stats_summary,
            commands::telemetry_cmd::get_stats_by_provider,
            commands::telemetry_cmd::get_stats_by_model,
            commands::telemetry_cmd::get_token_summary,
            commands::telemetry_cmd::get_token_stats_by_provider,
            commands::telemetry_cmd::get_token_stats_by_model,
            commands::telemetry_cmd::get_token_stats_by_day,
            // Injection commands
            commands::injection_cmd::get_injection_config,
            commands::injection_cmd::set_injection_enabled,
            commands::injection_cmd::get_injection_rules,
            commands::injection_cmd::add_injection_rule,
            commands::injection_cmd::remove_injection_rule,
            commands::injection_cmd::update_injection_rule,
            // Usage commands
            commands::usage_cmd::get_kiro_usage,
            // Tray commands
            commands::tray_cmd::sync_tray_state,
            commands::tray_cmd::update_tray_server_status,
            commands::tray_cmd::update_tray_credential_status,
            commands::tray_cmd::get_tray_state,
            commands::tray_cmd::refresh_tray_menu,
            commands::tray_cmd::refresh_tray_with_stats,
            // Plugin commands
            commands::plugin_cmd::get_plugin_status,
            commands::plugin_cmd::get_plugins,
            commands::plugin_cmd::get_plugin_info,
            commands::plugin_cmd::enable_plugin,
            commands::plugin_cmd::disable_plugin,
            commands::plugin_cmd::update_plugin_config,
            commands::plugin_cmd::get_plugin_config,
            commands::plugin_cmd::reload_plugins,
            commands::plugin_cmd::unload_plugin,
            commands::plugin_cmd::get_plugins_dir,
            // Plugin Install commands
            commands::plugin_install_cmd::install_plugin_from_file,
            commands::plugin_install_cmd::install_plugin_from_url,
            commands::plugin_install_cmd::uninstall_plugin,
            commands::plugin_install_cmd::list_installed_plugins,
            commands::plugin_install_cmd::get_installed_plugin,
            commands::plugin_install_cmd::is_plugin_installed,
            // Plugin UI commands
            commands::plugin_cmd::get_plugins_with_ui,
            // Flow Monitor commands
            commands::flow_monitor_cmd::query_flows,
            commands::flow_monitor_cmd::get_flow_detail,
            commands::flow_monitor_cmd::search_flows,
            commands::flow_monitor_cmd::get_flow_stats,
            commands::flow_monitor_cmd::export_flows,
            commands::flow_monitor_cmd::update_flow_annotations,
            commands::flow_monitor_cmd::toggle_flow_starred,
            commands::flow_monitor_cmd::add_flow_comment,
            commands::flow_monitor_cmd::add_flow_tag,
            commands::flow_monitor_cmd::remove_flow_tag,
            commands::flow_monitor_cmd::set_flow_marker,
            commands::flow_monitor_cmd::cleanup_flows,
            commands::flow_monitor_cmd::get_recent_flows,
            commands::flow_monitor_cmd::get_flow_monitor_status,
            commands::flow_monitor_cmd::get_flow_monitor_debug_info,
            commands::flow_monitor_cmd::create_test_flows,
            commands::flow_monitor_cmd::enable_flow_monitor,
            commands::flow_monitor_cmd::disable_flow_monitor,
            commands::flow_monitor_cmd::subscribe_flow_events,
            commands::flow_monitor_cmd::get_all_flow_tags,
            // Flow Monitor filter expression commands
            commands::flow_monitor_cmd::parse_filter,
            commands::flow_monitor_cmd::validate_filter,
            commands::flow_monitor_cmd::get_filter_help_items,
            commands::flow_monitor_cmd::get_filter_help_text,
            commands::flow_monitor_cmd::query_flows_with_expression,
            // Flow Interceptor commands
            commands::flow_monitor_cmd::intercept_config_get,
            commands::flow_monitor_cmd::intercept_config_set,
            commands::flow_monitor_cmd::intercept_continue,
            commands::flow_monitor_cmd::intercept_cancel,
            commands::flow_monitor_cmd::intercept_get_flow,
            commands::flow_monitor_cmd::intercept_list_flows,
            commands::flow_monitor_cmd::intercept_count,
            commands::flow_monitor_cmd::intercept_is_enabled,
            commands::flow_monitor_cmd::intercept_enable,
            commands::flow_monitor_cmd::intercept_disable,
            commands::flow_monitor_cmd::intercept_set_editing,
            commands::flow_monitor_cmd::subscribe_intercept_events,
            // Flow Monitor realtime enhancement commands
            commands::flow_monitor_cmd::get_threshold_config,
            commands::flow_monitor_cmd::update_threshold_config,
            commands::flow_monitor_cmd::get_request_rate,
            commands::flow_monitor_cmd::set_rate_window,
            // Flow Replayer commands
            commands::flow_monitor_cmd::replay_flow,
            commands::flow_monitor_cmd::replay_flows_batch,
            // Flow Diff commands
            commands::flow_monitor_cmd::diff_flows,
            // Session Management commands
            commands::flow_monitor_cmd::create_session,
            commands::flow_monitor_cmd::get_session,
            commands::flow_monitor_cmd::list_sessions,
            commands::flow_monitor_cmd::add_flow_to_session,
            commands::flow_monitor_cmd::remove_flow_from_session,
            commands::flow_monitor_cmd::update_session,
            commands::flow_monitor_cmd::archive_session,
            commands::flow_monitor_cmd::unarchive_session,
            commands::flow_monitor_cmd::delete_session,
            commands::flow_monitor_cmd::export_session,
            commands::flow_monitor_cmd::get_session_flow_count,
            commands::flow_monitor_cmd::is_flow_in_session,
            commands::flow_monitor_cmd::get_sessions_for_flow,
            commands::flow_monitor_cmd::get_auto_session_config,
            commands::flow_monitor_cmd::set_auto_session_config,
            commands::flow_monitor_cmd::register_active_session,
            // Quick Filter commands
            commands::flow_monitor_cmd::save_quick_filter,
            commands::flow_monitor_cmd::get_quick_filter,
            commands::flow_monitor_cmd::update_quick_filter,
            commands::flow_monitor_cmd::delete_quick_filter,
            commands::flow_monitor_cmd::list_quick_filters,
            commands::flow_monitor_cmd::list_quick_filters_by_group,
            commands::flow_monitor_cmd::list_quick_filter_groups,
            commands::flow_monitor_cmd::export_quick_filters,
            commands::flow_monitor_cmd::import_quick_filters,
            commands::flow_monitor_cmd::find_quick_filter_by_name,
            // Code Export commands
            commands::flow_monitor_cmd::export_flow_as_code,
            commands::flow_monitor_cmd::export_flows_as_code,
            commands::flow_monitor_cmd::get_code_export_formats,
            // Bookmark Management commands
            commands::flow_monitor_cmd::add_bookmark,
            commands::flow_monitor_cmd::get_bookmark,
            commands::flow_monitor_cmd::get_bookmark_by_flow_id,
            commands::flow_monitor_cmd::remove_bookmark,
            commands::flow_monitor_cmd::remove_bookmark_by_flow_id,
            commands::flow_monitor_cmd::update_bookmark,
            commands::flow_monitor_cmd::list_bookmarks,
            commands::flow_monitor_cmd::list_bookmark_groups,
            commands::flow_monitor_cmd::is_flow_bookmarked,
            commands::flow_monitor_cmd::get_bookmark_count,
            commands::flow_monitor_cmd::export_bookmarks,
            commands::flow_monitor_cmd::import_bookmarks,
            commands::flow_monitor_cmd::toggle_bookmark,
            // Enhanced Stats commands
            commands::flow_monitor_cmd::get_enhanced_stats,
            commands::flow_monitor_cmd::get_request_trend,
            commands::flow_monitor_cmd::get_token_distribution,
            commands::flow_monitor_cmd::get_latency_histogram,
            commands::flow_monitor_cmd::export_stats_report,
            // Batch Operations commands
            commands::flow_monitor_cmd::batch_star_flows,
            commands::flow_monitor_cmd::batch_unstar_flows,
            commands::flow_monitor_cmd::batch_add_tags,
            commands::flow_monitor_cmd::batch_remove_tags,
            commands::flow_monitor_cmd::batch_export_flows,
            commands::flow_monitor_cmd::batch_delete_flows,
            commands::flow_monitor_cmd::batch_add_to_session,
            // Window control commands
            commands::window_cmd::get_window_size,
            commands::window_cmd::set_window_size,
            commands::window_cmd::resize_for_flow_monitor,
            commands::window_cmd::restore_window_size,
            commands::window_cmd::toggle_window_size,
            commands::window_cmd::center_window,
            commands::window_cmd::get_window_size_options,
            commands::window_cmd::set_window_size_by_option,
            commands::window_cmd::toggle_fullscreen,
            commands::window_cmd::is_fullscreen,
            // Browser Interceptor commands
            commands::browser_interceptor_cmd::get_browser_interceptor_state,
            commands::browser_interceptor_cmd::start_browser_interceptor,
            commands::browser_interceptor_cmd::stop_browser_interceptor,
            commands::browser_interceptor_cmd::restore_normal_browser_behavior,
            commands::browser_interceptor_cmd::temporary_disable_interceptor,
            commands::browser_interceptor_cmd::get_intercepted_urls,
            commands::browser_interceptor_cmd::get_interceptor_history,
            commands::browser_interceptor_cmd::copy_intercepted_url_to_clipboard,
            commands::browser_interceptor_cmd::open_url_in_fingerprint_browser,
            commands::browser_interceptor_cmd::dismiss_intercepted_url,
            commands::browser_interceptor_cmd::update_browser_interceptor_config,
            commands::browser_interceptor_cmd::get_default_browser_interceptor_config,
            commands::browser_interceptor_cmd::validate_browser_interceptor_config,
            commands::browser_interceptor_cmd::is_browser_interceptor_running,
            commands::browser_interceptor_cmd::get_browser_interceptor_statistics,
            commands::browser_interceptor_cmd::show_notification,
            commands::browser_interceptor_cmd::show_url_intercept_notification,
            commands::browser_interceptor_cmd::show_status_notification,
            // Auto fix commands
            commands::auto_fix_cmd::auto_fix_configuration,
            // Machine ID commands
            commands::machine_id_cmd::get_current_machine_id,
            commands::machine_id_cmd::set_machine_id,
            commands::machine_id_cmd::generate_random_machine_id,
            commands::machine_id_cmd::validate_machine_id,
            commands::machine_id_cmd::check_admin_privileges,
            commands::machine_id_cmd::get_os_type,
            commands::machine_id_cmd::backup_machine_id_to_file,
            commands::machine_id_cmd::restore_machine_id_from_file,
            commands::machine_id_cmd::format_machine_id,
            commands::machine_id_cmd::detect_machine_id_format,
            commands::machine_id_cmd::convert_machine_id_format,
            commands::machine_id_cmd::get_machine_id_history,
            commands::machine_id_cmd::clear_machine_id_override,
            commands::machine_id_cmd::copy_machine_id_to_clipboard,
            commands::machine_id_cmd::paste_machine_id_from_clipboard,
            commands::machine_id_cmd::get_system_info,
            // Kiro Local commands
            commands::kiro_local::switch_kiro_to_local,
            commands::kiro_local::get_kiro_fingerprint_info,
            commands::kiro_local::get_local_kiro_credential_uuid,
            // Agent commands
            commands::agent_cmd::agent_start_process,
            commands::agent_cmd::agent_stop_process,
            commands::agent_cmd::agent_get_process_status,
            commands::agent_cmd::agent_create_session,
            commands::agent_cmd::agent_send_message,
            commands::agent_cmd::agent_list_sessions,
            commands::agent_cmd::agent_get_session,
            commands::agent_cmd::agent_delete_session,
            // Native Agent commands
            commands::native_agent_cmd::native_agent_init,
            commands::native_agent_cmd::native_agent_status,
            commands::native_agent_cmd::native_agent_reset,
            commands::native_agent_cmd::native_agent_chat,
            commands::native_agent_cmd::native_agent_chat_stream,
            commands::native_agent_cmd::native_agent_create_session,
            commands::native_agent_cmd::native_agent_get_session,
            commands::native_agent_cmd::native_agent_delete_session,
            commands::native_agent_cmd::native_agent_list_sessions,
            // Models config commands
            commands::models_cmd::get_models_config,
            commands::models_cmd::save_models_config,
            commands::models_cmd::get_provider_models,
            commands::models_cmd::get_all_provider_models,
            commands::models_cmd::add_model_to_provider,
            commands::models_cmd::remove_model_from_provider,
            commands::models_cmd::toggle_model_enabled,
            commands::models_cmd::add_provider,
            commands::models_cmd::remove_provider,
            // Network commands
            commands::network_cmd::get_network_info,
            // OAuth Plugin commands
            commands::oauth_plugin_cmd::init_oauth_plugin_system,
            commands::oauth_plugin_cmd::list_oauth_plugins,
            commands::oauth_plugin_cmd::get_oauth_plugin,
            commands::oauth_plugin_cmd::enable_oauth_plugin,
            commands::oauth_plugin_cmd::disable_oauth_plugin,
            commands::oauth_plugin_cmd::install_oauth_plugin,
            commands::oauth_plugin_cmd::uninstall_oauth_plugin,
            commands::oauth_plugin_cmd::check_oauth_plugin_updates,
            commands::oauth_plugin_cmd::update_oauth_plugin,
            commands::oauth_plugin_cmd::reload_oauth_plugins,
            commands::oauth_plugin_cmd::get_oauth_plugin_config,
            commands::oauth_plugin_cmd::update_oauth_plugin_config,
            commands::oauth_plugin_cmd::scan_oauth_plugin_directory,
            // OAuth Plugin credential commands
            commands::oauth_plugin_cmd::plugin_credential_list,
            commands::oauth_plugin_cmd::plugin_credential_get,
            commands::oauth_plugin_cmd::plugin_credential_create,
            commands::oauth_plugin_cmd::plugin_credential_update,
            commands::oauth_plugin_cmd::plugin_credential_delete,
            commands::oauth_plugin_cmd::plugin_credential_validate,
            commands::oauth_plugin_cmd::plugin_credential_refresh,
            // OAuth Plugin SDK commands
            commands::oauth_plugin_cmd::plugin_database_query,
            commands::oauth_plugin_cmd::plugin_database_execute,
            commands::oauth_plugin_cmd::plugin_http_request,
            commands::oauth_plugin_cmd::plugin_crypto_encrypt,
            commands::oauth_plugin_cmd::plugin_crypto_decrypt,
            commands::oauth_plugin_cmd::plugin_notification,
            commands::oauth_plugin_cmd::plugin_storage_get,
            commands::oauth_plugin_cmd::plugin_storage_set,
            commands::oauth_plugin_cmd::plugin_storage_delete,
            commands::oauth_plugin_cmd::plugin_storage_keys,
            commands::oauth_plugin_cmd::plugin_config_get,
            commands::oauth_plugin_cmd::plugin_config_set,
            // OAuth Plugin UI commands
            commands::oauth_plugin_cmd::read_plugin_ui_file,
            // Orchestrator commands
            commands::orchestrator_cmd::init_orchestrator,
            commands::orchestrator_cmd::get_orchestrator_config,
            commands::orchestrator_cmd::update_orchestrator_config,
            commands::orchestrator_cmd::get_pool_stats,
            commands::orchestrator_cmd::get_tier_models,
            commands::orchestrator_cmd::get_all_models,
            commands::orchestrator_cmd::update_orchestrator_credentials,
            commands::orchestrator_cmd::add_orchestrator_credential,
            commands::orchestrator_cmd::remove_orchestrator_credential,
            commands::orchestrator_cmd::mark_credential_unhealthy,
            commands::orchestrator_cmd::mark_credential_healthy,
            commands::orchestrator_cmd::update_credential_load,
            commands::orchestrator_cmd::select_model,
            commands::orchestrator_cmd::quick_select_model,
            commands::orchestrator_cmd::select_model_for_task,
            commands::orchestrator_cmd::list_strategies,
            commands::orchestrator_cmd::list_service_tiers,
            commands::orchestrator_cmd::list_task_hints,
            // Connect commands
            // _Requirements: 1.4, 2.3, 4.1, 5.3_
            commands::connect_cmd::handle_deep_link,
            commands::connect_cmd::get_relay_info,
            commands::connect_cmd::save_relay_api_key,
            commands::connect_cmd::refresh_relay_registry,
            commands::connect_cmd::list_relay_providers,
            commands::connect_cmd::send_connect_callback,
            // Model Registry commands
            commands::model_registry_cmd::get_model_registry,
            commands::model_registry_cmd::refresh_model_registry,
            commands::model_registry_cmd::search_models,
            commands::model_registry_cmd::get_model_preferences,
            commands::model_registry_cmd::toggle_model_favorite,
            commands::model_registry_cmd::hide_model,
            commands::model_registry_cmd::record_model_usage,
            commands::model_registry_cmd::get_model_sync_state,
            commands::model_registry_cmd::get_models_for_provider,
            commands::model_registry_cmd::get_models_by_tier,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
