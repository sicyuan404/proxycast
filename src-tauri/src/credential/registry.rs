//! OAuth Provider 插件注册表
//!
//! 管理所有 OAuth Provider 插件的注册、发现和生命周期。
//! 支持从外部目录动态加载插件。

use super::plugin::{OAuthPluginError, OAuthPluginInfo, OAuthPluginResult, PluginInstance};
use dashmap::DashMap;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 插件来源
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginSource {
    /// 从 GitHub Release 安装
    GitHub {
        owner: String,
        repo: String,
        version: Option<String>,
    },
    /// 从本地文件安装
    LocalFile { path: PathBuf },
    /// 内置插件（编译时包含）
    Builtin { id: String },
}

/// 插件更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUpdate {
    /// 插件 ID
    pub plugin_id: String,
    /// 当前版本
    pub current_version: String,
    /// 最新版本
    pub latest_version: String,
    /// 更新说明
    pub changelog: Option<String>,
}

/// 插件状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginState {
    /// 是否启用
    pub enabled: bool,
    /// 插件配置
    pub config: serde_json::Value,
    /// 安装时间
    pub installed_at: Option<String>,
    /// 最后使用时间
    pub last_used_at: Option<String>,
}

/// OAuth Provider 插件注册表
///
/// 负责管理所有 OAuth Provider 插件的注册、发现和路由。
/// 支持运行时动态加载和卸载插件。
pub struct CredentialProviderRegistry {
    /// 已注册的插件（id -> 插件实例）
    providers: DashMap<String, PluginInstance>,

    /// 模型到插件的映射（用于快速查找）
    /// 键是模型模式（如 "claude-*"），值是插件 ID
    model_patterns: RwLock<Vec<(Pattern, String)>>,

    /// 插件状态
    plugin_states: DashMap<String, PluginState>,

    /// 插件目录
    plugins_dir: PathBuf,
}

impl std::fmt::Debug for CredentialProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialProviderRegistry")
            .field("plugins_dir", &self.plugins_dir)
            .field("provider_count", &self.providers.len())
            .finish()
    }
}

impl CredentialProviderRegistry {
    /// 创建新的注册表
    pub fn new(plugins_dir: PathBuf) -> Self {
        Self {
            providers: DashMap::new(),
            model_patterns: RwLock::new(Vec::new()),
            plugin_states: DashMap::new(),
            plugins_dir,
        }
    }

    /// 获取插件目录
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    // ========================================================================
    // 插件注册
    // ========================================================================

    /// 注册插件
    pub async fn register(&self, plugin: PluginInstance) -> OAuthPluginResult<()> {
        let id = plugin.id().to_string();
        let display_name = plugin.display_name().to_string();

        info!(
            "Registering OAuth provider plugin: {} ({})",
            id, display_name
        );

        // 初始化插件
        plugin.init().await?;

        // 注册模型模式
        let families = plugin.model_families();
        let mut patterns = self.model_patterns.write().await;

        for family in families {
            match Pattern::new(&family.pattern) {
                Ok(pattern) => {
                    patterns.push((pattern, id.clone()));
                    debug!(
                        "Registered model pattern '{}' for plugin '{}'",
                        family.pattern, id
                    );
                }
                Err(e) => {
                    warn!(
                        "Invalid model pattern '{}' for plugin '{}': {}",
                        family.pattern, id, e
                    );
                }
            }
        }

        // 创建默认状态
        if !self.plugin_states.contains_key(&id) {
            self.plugin_states.insert(
                id.clone(),
                PluginState {
                    enabled: true,
                    config: serde_json::json!({}),
                    installed_at: Some(chrono::Utc::now().to_rfc3339()),
                    last_used_at: None,
                },
            );
        }

        // 注册插件
        self.providers.insert(id.clone(), plugin);

        info!("Successfully registered OAuth provider plugin: {}", id);
        Ok(())
    }

    /// 注销插件
    pub async fn unregister(&self, plugin_id: &str) -> OAuthPluginResult<()> {
        info!("Unregistering OAuth provider plugin: {}", plugin_id);

        // 移除插件
        if let Some((_, plugin)) = self.providers.remove(plugin_id) {
            // 关闭插件
            if let Err(e) = plugin.shutdown().await {
                warn!("Error shutting down plugin {}: {}", plugin_id, e);
            }
        }

        // 移除模型模式
        let mut patterns = self.model_patterns.write().await;
        patterns.retain(|(_, id)| id != plugin_id);

        // 移除状态
        self.plugin_states.remove(plugin_id);

        info!(
            "Successfully unregistered OAuth provider plugin: {}",
            plugin_id
        );
        Ok(())
    }

    // ========================================================================
    // 插件查找
    // ========================================================================

    /// 根据 ID 获取插件
    pub fn get(&self, plugin_id: &str) -> Option<PluginInstance> {
        self.providers.get(plugin_id).map(|r| r.value().clone())
    }

    /// 根据模型名称查找插件
    pub async fn find_by_model(&self, model: &str) -> Option<PluginInstance> {
        let patterns = self.model_patterns.read().await;

        // 按注册顺序查找匹配的模式
        for (pattern, plugin_id) in patterns.iter() {
            if pattern.matches(model) {
                // 检查插件是否启用
                if let Some(state) = self.plugin_states.get(plugin_id) {
                    if !state.enabled {
                        continue;
                    }
                }

                if let Some(plugin) = self.providers.get(plugin_id) {
                    // 更新最后使用时间
                    if let Some(mut state) = self.plugin_states.get_mut(plugin_id) {
                        state.last_used_at = Some(chrono::Utc::now().to_rfc3339());
                    }
                    return Some(plugin.value().clone());
                }
            }
        }

        None
    }

    /// 获取所有已注册的插件
    pub fn get_all(&self) -> Vec<PluginInstance> {
        self.providers.iter().map(|r| r.value().clone()).collect()
    }

    /// 获取所有已启用的插件
    pub fn get_enabled(&self) -> Vec<PluginInstance> {
        self.providers
            .iter()
            .filter(|r| {
                self.plugin_states
                    .get(r.key())
                    .map(|s| s.enabled)
                    .unwrap_or(true)
            })
            .map(|r| r.value().clone())
            .collect()
    }

    /// 获取所有插件信息（用于 UI 显示）
    pub fn get_plugin_infos(&self) -> Vec<OAuthPluginInfo> {
        use super::plugin::CredentialCategory;

        let mut infos: Vec<OAuthPluginInfo> = self
            .providers
            .iter()
            .map(|r| {
                let plugin = r.value();
                let state = self
                    .plugin_states
                    .get(r.key())
                    .map(|s| s.value().clone())
                    .unwrap_or_default();

                let mut info = OAuthPluginInfo::from_plugin(plugin.as_ref());
                info.enabled = state.enabled;
                info
            })
            .collect();

        // 同时扫描插件目录中已安装但未加载的插件
        if let Ok(entries) = std::fs::read_dir(&self.plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let plugin_json = path.join("plugin.json");
                    if plugin_json.exists() {
                        if let Ok(content) = std::fs::read_to_string(&plugin_json) {
                            if let Ok(manifest) =
                                serde_json::from_str::<serde_json::Value>(&content)
                            {
                                // 只处理 oauth_provider 类型的插件
                                let plugin_type = manifest["plugin_type"].as_str().unwrap_or("");
                                if plugin_type != "oauth_provider" {
                                    continue;
                                }

                                let plugin_id = manifest["name"].as_str().unwrap_or_default();

                                // 跳过已经在 providers 中的插件
                                if infos.iter().any(|i| i.id == plugin_id) {
                                    continue;
                                }

                                let state = self
                                    .plugin_states
                                    .get(plugin_id)
                                    .map(|s| s.value().clone())
                                    .unwrap_or_else(|| PluginState {
                                        enabled: true, // 默认启用
                                        config: serde_json::json!({}),
                                        installed_at: None,
                                        last_used_at: None,
                                    });

                                let info = OAuthPluginInfo {
                                    id: plugin_id.to_string(),
                                    display_name: manifest["provider"]["display_name"]
                                        .as_str()
                                        .or_else(|| manifest["name"].as_str())
                                        .unwrap_or(plugin_id)
                                        .to_string(),
                                    version: manifest["version"]
                                        .as_str()
                                        .unwrap_or("0.0.0")
                                        .to_string(),
                                    description: manifest["description"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                    target_protocol: manifest["provider"]["target_protocol"]
                                        .as_str()
                                        .unwrap_or("unknown")
                                        .to_string(),
                                    category: CredentialCategory::OAuth,
                                    auth_types: vec![],
                                    enabled: state.enabled,
                                    credential_count: 0,
                                    healthy_credential_count: 0,
                                };
                                infos.push(info);
                            }
                        }
                    }
                }
            }
        }

        infos
    }

    // ========================================================================
    // 插件状态管理
    // ========================================================================

    /// 启用插件
    pub fn enable_plugin(&self, plugin_id: &str) -> bool {
        // 如果状态不存在，先检查插件目录是否存在
        if !self.plugin_states.contains_key(plugin_id) {
            let plugin_dir = self.plugins_dir.join(plugin_id);
            if plugin_dir.join("plugin.json").exists() {
                // 创建默认状态
                let state = PluginState {
                    enabled: true,
                    config: serde_json::json!({}),
                    installed_at: Some(chrono::Utc::now().to_rfc3339()),
                    last_used_at: None,
                };
                self.plugin_states.insert(plugin_id.to_string(), state);
                info!("Enabled OAuth provider plugin: {}", plugin_id);
                return true;
            }
            return false;
        }

        if let Some(mut state) = self.plugin_states.get_mut(plugin_id) {
            state.enabled = true;
            info!("Enabled OAuth provider plugin: {}", plugin_id);
            true
        } else {
            false
        }
    }

    /// 禁用插件
    pub fn disable_plugin(&self, plugin_id: &str) -> bool {
        // 如果状态不存在，先检查插件目录是否存在
        if !self.plugin_states.contains_key(plugin_id) {
            let plugin_dir = self.plugins_dir.join(plugin_id);
            if plugin_dir.join("plugin.json").exists() {
                // 创建默认状态（禁用）
                let state = PluginState {
                    enabled: false,
                    config: serde_json::json!({}),
                    installed_at: Some(chrono::Utc::now().to_rfc3339()),
                    last_used_at: None,
                };
                self.plugin_states.insert(plugin_id.to_string(), state);
                info!("Disabled OAuth provider plugin: {}", plugin_id);
                return true;
            }
            return false;
        }

        if let Some(mut state) = self.plugin_states.get_mut(plugin_id) {
            state.enabled = false;
            info!("Disabled OAuth provider plugin: {}", plugin_id);
            true
        } else {
            false
        }
    }

    /// 获取插件状态
    pub fn get_plugin_state(&self, plugin_id: &str) -> Option<PluginState> {
        self.plugin_states.get(plugin_id).map(|r| r.value().clone())
    }

    /// 更新插件配置
    pub async fn update_plugin_config(
        &self,
        plugin_id: &str,
        config: serde_json::Value,
    ) -> OAuthPluginResult<()> {
        // 更新状态中的配置
        if let Some(mut state) = self.plugin_states.get_mut(plugin_id) {
            state.config = config.clone();
        }

        // 通知插件配置更新
        if let Some(plugin) = self.providers.get(plugin_id) {
            plugin.update_plugin_config(config).await?;
        }

        Ok(())
    }

    // ========================================================================
    // 插件安装管理
    // ========================================================================

    /// 安装插件（从外部来源）
    pub async fn install_plugin(&self, source: PluginSource) -> OAuthPluginResult<String> {
        match source {
            PluginSource::GitHub {
                owner,
                repo,
                version,
            } => {
                self.install_from_github(&owner, &repo, version.as_deref())
                    .await
            }
            PluginSource::LocalFile { path } => self.install_from_local(&path).await,
            PluginSource::Builtin { id } => Err(OAuthPluginError::InitError(format!(
                "Builtin plugin '{}' cannot be installed manually",
                id
            ))),
        }
    }

    /// 从 GitHub Release 安装插件
    async fn install_from_github(
        &self,
        owner: &str,
        repo: &str,
        version: Option<&str>,
    ) -> OAuthPluginResult<String> {
        let version_tag = version.unwrap_or("latest");

        // 构建下载 URL
        let download_url = if version_tag == "latest" {
            format!(
                "https://github.com/{}/{}/releases/latest/download/{}-plugin.zip",
                owner, repo, repo
            )
        } else {
            format!(
                "https://github.com/{}/{}/releases/download/{}/{}-plugin.zip",
                owner, repo, version_tag, repo
            )
        };

        info!("Downloading plugin from: {}", download_url);

        // 下载插件包
        let client = reqwest::Client::new();
        let response = client.get(&download_url).send().await.map_err(|e| {
            OAuthPluginError::InitError(format!("Failed to download plugin: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(OAuthPluginError::InitError(format!(
                "Failed to download plugin: HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| OAuthPluginError::InitError(format!("Failed to read response: {}", e)))?;

        // 创建临时目录解压
        let temp_dir = std::env::temp_dir().join(format!("oauth_plugin_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        // 解压 ZIP 文件
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| OAuthPluginError::InitError(format!("Failed to open zip: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                OAuthPluginError::InitError(format!("Failed to read zip entry: {}", e))
            })?;

            let outpath = temp_dir.join(file.name());

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }

        // 读取 plugin.json 获取插件 ID
        let plugin_json_path = temp_dir.join("plugin.json");
        if !plugin_json_path.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
            return Err(OAuthPluginError::InitError(
                "Invalid plugin package: missing plugin.json".to_string(),
            ));
        }

        let plugin_json = std::fs::read_to_string(&plugin_json_path)?;
        let plugin_info: serde_json::Value = serde_json::from_str(&plugin_json)
            .map_err(|e| OAuthPluginError::InitError(format!("Invalid plugin.json: {}", e)))?;

        let plugin_id = plugin_info["name"]
            .as_str()
            .ok_or_else(|| {
                OAuthPluginError::InitError("Missing 'name' in plugin.json".to_string())
            })?
            .to_string();

        // 移动到插件目录
        let target_dir = self.plugins_dir.join(&plugin_id);
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir)?;
        }
        std::fs::rename(&temp_dir, &target_dir)?;

        info!("Plugin installed to: {:?}", target_dir);

        // 尝试下载 UI 资源包（如果存在）
        let ui_url = if version_tag == "latest" {
            format!(
                "https://github.com/{}/{}/releases/latest/download/{}-ui.zip",
                owner, repo, repo
            )
        } else {
            format!(
                "https://github.com/{}/{}/releases/download/{}/{}-ui.zip",
                owner, repo, version_tag, repo
            )
        };

        info!("Trying to download UI assets from: {}", ui_url);

        if let Ok(ui_response) = client.get(&ui_url).send().await {
            if ui_response.status().is_success() {
                if let Ok(ui_bytes) = ui_response.bytes().await {
                    let ui_cursor = std::io::Cursor::new(ui_bytes);
                    if let Ok(mut ui_archive) = zip::ZipArchive::new(ui_cursor) {
                        for i in 0..ui_archive.len() {
                            if let Ok(mut file) = ui_archive.by_index(i) {
                                let outpath = target_dir.join(file.name());

                                if file.name().ends_with('/') {
                                    let _ = std::fs::create_dir_all(&outpath);
                                } else {
                                    if let Some(p) = outpath.parent() {
                                        if !p.exists() {
                                            let _ = std::fs::create_dir_all(p);
                                        }
                                    }
                                    if let Ok(mut outfile) = std::fs::File::create(&outpath) {
                                        let _ = std::io::copy(&mut file, &mut outfile);
                                    }
                                }
                            }
                        }
                        info!("UI assets installed for plugin: {}", plugin_id);
                    }
                }
            } else {
                info!(
                    "No UI assets available for plugin: {} (HTTP {})",
                    plugin_id,
                    ui_response.status()
                );
            }
        } else {
            info!("No UI assets available for plugin: {}", plugin_id);
        }

        // 注册插件（创建 PluginInstance）
        self.register_from_dir(&target_dir, &plugin_id).await?;

        Ok(plugin_id)
    }

    /// 从本地文件安装插件
    async fn install_from_local(&self, path: &Path) -> OAuthPluginResult<String> {
        // 检查路径是否存在
        if !path.exists() {
            return Err(OAuthPluginError::InitError(format!(
                "Path does not exist: {:?}",
                path
            )));
        }

        // 如果是目录，直接复制
        if path.is_dir() {
            let plugin_json_path = path.join("plugin.json");
            if !plugin_json_path.exists() {
                return Err(OAuthPluginError::InitError(
                    "Invalid plugin directory: missing plugin.json".to_string(),
                ));
            }

            let plugin_json = std::fs::read_to_string(&plugin_json_path)?;
            let plugin_info: serde_json::Value = serde_json::from_str(&plugin_json)
                .map_err(|e| OAuthPluginError::InitError(format!("Invalid plugin.json: {}", e)))?;

            let plugin_id = plugin_info["name"]
                .as_str()
                .ok_or_else(|| {
                    OAuthPluginError::InitError("Missing 'name' in plugin.json".to_string())
                })?
                .to_string();

            let target_dir = self.plugins_dir.join(&plugin_id);
            if target_dir.exists() {
                std::fs::remove_dir_all(&target_dir)?;
            }

            // 复制目录
            copy_dir_all(path, &target_dir)?;

            info!("Plugin installed from local directory to: {:?}", target_dir);

            // 注册插件
            self.register_from_dir(&target_dir, &plugin_id).await?;

            return Ok(plugin_id);
        }

        // 如果是 ZIP 文件
        if path.extension().map_or(false, |ext| ext == "zip") {
            let file = std::fs::File::open(path)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| OAuthPluginError::InitError(format!("Failed to open zip: {}", e)))?;

            let temp_dir =
                std::env::temp_dir().join(format!("oauth_plugin_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir)?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i).map_err(|e| {
                    OAuthPluginError::InitError(format!("Failed to read zip entry: {}", e))
                })?;

                let outpath = temp_dir.join(file.name());

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(p)?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }

            let plugin_json_path = temp_dir.join("plugin.json");
            if !plugin_json_path.exists() {
                std::fs::remove_dir_all(&temp_dir)?;
                return Err(OAuthPluginError::InitError(
                    "Invalid plugin package: missing plugin.json".to_string(),
                ));
            }

            let plugin_json = std::fs::read_to_string(&plugin_json_path)?;
            let plugin_info: serde_json::Value = serde_json::from_str(&plugin_json)
                .map_err(|e| OAuthPluginError::InitError(format!("Invalid plugin.json: {}", e)))?;

            let plugin_id = plugin_info["name"]
                .as_str()
                .ok_or_else(|| {
                    OAuthPluginError::InitError("Missing 'name' in plugin.json".to_string())
                })?
                .to_string();

            let target_dir = self.plugins_dir.join(&plugin_id);
            if target_dir.exists() {
                std::fs::remove_dir_all(&target_dir)?;
            }
            std::fs::rename(&temp_dir, &target_dir)?;

            info!("Plugin installed from zip to: {:?}", target_dir);

            self.register_from_dir(&target_dir, &plugin_id).await?;

            return Ok(plugin_id);
        }

        Err(OAuthPluginError::InitError(format!(
            "Unsupported file type: {:?}",
            path
        )))
    }

    /// 从目录注册插件
    async fn register_from_dir(&self, plugin_dir: &Path, plugin_id: &str) -> OAuthPluginResult<()> {
        // 读取 plugin.json
        let plugin_json_path = plugin_dir.join("plugin.json");
        let plugin_json = std::fs::read_to_string(&plugin_json_path)?;
        let manifest: serde_json::Value = serde_json::from_str(&plugin_json)
            .map_err(|e| OAuthPluginError::InitError(format!("Invalid plugin.json: {}", e)))?;

        // 设置初始状态
        let state = PluginState {
            enabled: true,
            config: serde_json::json!({}),
            installed_at: Some(chrono::Utc::now().to_rfc3339()),
            last_used_at: None,
        };
        self.plugin_states.insert(plugin_id.to_string(), state);

        info!(
            "Registered plugin: {} ({})",
            plugin_id,
            manifest["version"].as_str().unwrap_or("unknown")
        );

        Ok(())
    }

    /// 卸载插件
    pub async fn uninstall_plugin(&self, plugin_id: &str) -> OAuthPluginResult<()> {
        // 1. 注销插件
        self.unregister(plugin_id).await?;

        // 2. 删除插件目录
        let plugin_dir = self.plugins_dir.join(plugin_id);
        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir)?;
            info!("Removed plugin directory: {:?}", plugin_dir);
        }

        Ok(())
    }

    /// 检查插件更新
    pub async fn check_updates(&self) -> OAuthPluginResult<Vec<PluginUpdate>> {
        // 已知插件的最新版本（与前端 OAuthPluginTab.tsx 保持同步）
        let latest_versions: std::collections::HashMap<&str, &str> = [
            ("kiro-provider", "0.3.0"),
            ("antigravity-provider", "0.4.0"),
            ("claude-provider", "0.3.0"),
            ("droid-provider", "0.3.0"),
            ("gemini-provider", "0.4.0"),
            ("codex-provider", "0.1.0"),
        ]
        .into_iter()
        .collect();

        let mut updates = Vec::new();

        // 扫描已安装的插件
        if let Ok(entries) = std::fs::read_dir(&self.plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let plugin_json = path.join("plugin.json");
                    if plugin_json.exists() {
                        if let Ok(content) = std::fs::read_to_string(&plugin_json) {
                            if let Ok(manifest) =
                                serde_json::from_str::<serde_json::Value>(&content)
                            {
                                let plugin_id = manifest["name"].as_str().unwrap_or_default();
                                let current_version =
                                    manifest["version"].as_str().unwrap_or("0.0.0");

                                // 检查是否有更新
                                if let Some(&latest) = latest_versions.get(plugin_id) {
                                    if version_compare(current_version, latest)
                                        == std::cmp::Ordering::Less
                                    {
                                        updates.push(PluginUpdate {
                                            plugin_id: plugin_id.to_string(),
                                            current_version: current_version.to_string(),
                                            latest_version: latest.to_string(),
                                            changelog: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(updates)
    }

    // ========================================================================
    // 生命周期管理
    // ========================================================================

    /// 关闭所有插件
    pub async fn shutdown_all(&self) -> OAuthPluginResult<()> {
        info!("Shutting down all OAuth provider plugins...");

        for entry in self.providers.iter() {
            let plugin_id = entry.key();
            let plugin = entry.value();

            if let Err(e) = plugin.shutdown().await {
                error!("Error shutting down plugin {}: {}", plugin_id, e);
            } else {
                debug!("Successfully shut down plugin: {}", plugin_id);
            }
        }

        info!("All OAuth provider plugins shut down");
        Ok(())
    }
}

// ============================================================================
// 全局注册表
// ============================================================================

use once_cell::sync::OnceCell;

static GLOBAL_REGISTRY: OnceCell<Arc<CredentialProviderRegistry>> = OnceCell::new();

/// 初始化全局注册表
pub fn init_global_registry(plugins_dir: PathBuf) -> Arc<CredentialProviderRegistry> {
    let registry = Arc::new(CredentialProviderRegistry::new(plugins_dir));
    GLOBAL_REGISTRY
        .set(registry.clone())
        .expect("Global registry already initialized");
    registry
}

/// 获取全局注册表
pub fn get_global_registry() -> Option<Arc<CredentialProviderRegistry>> {
    GLOBAL_REGISTRY.get().cloned()
}

/// 比较语义化版本号
fn version_compare(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u32> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let v1_parts = parse(v1);
    let v2_parts = parse(v2);

    for i in 0..std::cmp::max(v1_parts.len(), v2_parts.len()) {
        let p1 = v1_parts.get(i).copied().unwrap_or(0);
        let p2 = v2_parts.get(i).copied().unwrap_or(0);
        match p1.cmp(&p2) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// 递归复制目录
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_registry_creation() {
        let registry = CredentialProviderRegistry::new(temp_dir().join("test_plugins"));
        assert_eq!(registry.get_all().len(), 0);
    }

    #[tokio::test]
    async fn test_find_by_model_empty() {
        let registry = CredentialProviderRegistry::new(temp_dir().join("test_plugins"));
        let result = registry.find_by_model("claude-opus-4").await;
        assert!(result.is_none());
    }

    #[test]
    fn test_plugin_state_default() {
        let state = PluginState::default();
        assert!(!state.enabled);
        assert!(state.installed_at.is_none());
    }
}
