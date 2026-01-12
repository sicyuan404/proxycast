//! ProxyCast - AI API 代理服务
//!
//! 这是一个 Tauri 应用，提供 AI API 的代理和管理功能。

// 抑制 objc crate 宏内部的 unexpected_cfgs 警告
// 该警告来自 cocoa/objc 依赖的 msg_send! 宏，是已知的 issue
#![allow(unexpected_cfgs)]

// 核心模块
pub mod agent;
pub mod app;
pub mod backends;
pub mod browser_interceptor;
pub mod connect;
pub mod credential;
pub mod database;
pub mod flow_monitor;
pub mod injection;
pub mod middleware;
pub mod orchestrator;
pub mod plugin;
pub mod processor;
pub mod proxy;
pub mod resilience;
pub mod router;
pub mod screenshot;
pub mod services;
pub mod session;
pub mod session_files;
pub mod stream;
pub mod streaming;
pub mod telemetry;
pub mod terminal;
pub mod translator;
pub mod tray;
pub mod websocket;

// 内部模块
mod commands;
mod config;
mod converter;
mod data;
#[cfg(debug_assertions)]
mod dev_bridge;
mod logger;
mod models;
mod providers;
mod server;
mod server_utils;

// 重新导出核心类型以保持向后兼容
pub use app::{AppState, LogState, ProviderType, TokenCacheServiceState, TrayManagerState};
pub use services::provider_pool_service::ProviderPoolService;

// 重新导出 run 函数
pub use app::run;
