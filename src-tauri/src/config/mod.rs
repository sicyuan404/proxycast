//! 配置管理模块
//!
//! 提供 YAML 配置文件支持、热重载和配置导入导出功能
//! 同时保持与旧版 JSON 配置的向后兼容性

#![allow(unused_imports)]

mod export;
mod hot_reload;
mod import;
mod path_utils;
mod types;
mod yaml;

pub use export::{ExportBundle, ExportOptions, ExportService, REDACTED_PLACEHOLDER};
pub use hot_reload::{
    ConfigChangeEvent, ConfigChangeKind, FileWatcher, HotReloadManager, ReloadResult,
};
pub use import::{ImportOptions, ImportService, ValidationResult};
pub use path_utils::{collapse_tilde, contains_tilde, expand_tilde};
pub use types::{
    generate_secure_api_key, AmpConfig, AmpModelMapping, ApiKeyEntry, Config, CredentialEntry,
    CredentialPoolConfig, CustomProviderConfig, EndpointProvidersConfig, GeminiApiKeyEntry,
    IFlowCredentialEntry, InjectionRuleConfig, InjectionSettings, LoggingConfig, ModelInfo,
    ModelsConfig, NativeAgentConfig, ProviderConfig, ProviderModelsConfig, ProvidersConfig,
    QuotaExceededConfig, RemoteManagementConfig, RetrySettings, RoutingConfig, RoutingRuleConfig,
    ServerConfig, TlsConfig, VertexApiKeyEntry, VertexModelAlias, DEFAULT_API_KEY,
};
pub use yaml::{load_config, save_config, ConfigError, ConfigManager, YamlService};

#[cfg(test)]
mod tests;
