use crate::models::{AppType, Provider};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Get the configuration file path for an app type
#[allow(dead_code)]
pub fn get_app_config_path(app_type: &AppType) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match app_type {
        AppType::Claude => Some(home.join(".claude").join("settings.json")),
        AppType::Codex => Some(home.join(".codex")),
        AppType::Gemini => Some(home.join(".gemini")),
        AppType::ProxyCast => None,
    }
}

/// Sync provider configuration to live config files
pub fn sync_to_live(
    app_type: &AppType,
    provider: &Provider,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match app_type {
        AppType::Claude => sync_claude_settings(provider),
        AppType::Codex => sync_codex_config(provider),
        AppType::Gemini => sync_gemini_config(provider),
        AppType::ProxyCast => Ok(()),
    }
}

/// Sync Claude settings to ~/.claude/settings.json
fn sync_claude_settings(
    provider: &Provider,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let claude_dir = home.join(".claude");
    let config_path = claude_dir.join("settings.json");

    // Create directory if not exists
    std::fs::create_dir_all(&claude_dir)?;

    // Read existing settings to preserve other fields
    let mut settings: Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Merge env variables into settings
    if let Some(env_obj) = provider
        .settings_config
        .get("env")
        .and_then(|v| v.as_object())
    {
        let settings_obj = settings.as_object_mut().ok_or("Invalid settings format")?;

        // Ensure env object exists
        if !settings_obj.contains_key("env") {
            settings_obj.insert("env".to_string(), json!({}));
        }

        if let Some(target_env) = settings_obj.get_mut("env").and_then(|v| v.as_object_mut()) {
            for (key, value) in env_obj {
                target_env.insert(key.clone(), value.clone());
            }
        }
    } else {
        // If settings_config is the full settings object, use it directly
        settings = provider.settings_config.clone();
    }

    // Write settings
    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&config_path, content)?;

    Ok(())
}

/// Sync Codex config to ~/.codex/auth.json and ~/.codex/config.toml
fn sync_codex_config(provider: &Provider) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let codex_dir = home.join(".codex");

    // Create directory if not exists
    std::fs::create_dir_all(&codex_dir)?;

    if let Some(obj) = provider.settings_config.as_object() {
        // Write auth.json
        if let Some(auth) = obj.get("auth") {
            let auth_path = codex_dir.join("auth.json");
            let content = serde_json::to_string_pretty(auth)?;
            std::fs::write(&auth_path, content)?;
        }

        // Write config.toml
        if let Some(config) = obj.get("config").and_then(|v| v.as_str()) {
            let config_path = codex_dir.join("config.toml");
            std::fs::write(&config_path, config)?;
        }
    }

    Ok(())
}

/// Sync Gemini config to ~/.gemini/.env and ~/.gemini/settings.json
fn sync_gemini_config(provider: &Provider) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let gemini_dir = home.join(".gemini");

    // Create directory if not exists
    std::fs::create_dir_all(&gemini_dir)?;

    // Write .env file
    if let Some(env_obj) = provider
        .settings_config
        .get("env")
        .and_then(|v| v.as_object())
    {
        let env_path = gemini_dir.join(".env");
        let mut content = String::new();

        for (key, value) in env_obj {
            if let Some(val) = value.as_str() {
                // Only write non-empty values
                if !val.is_empty() {
                    content.push_str(&format!("{key}={val}\n"));
                }
            }
        }

        std::fs::write(&env_path, content)?;
    }

    // Write settings.json (for MCP servers and other config)
    if let Some(config) = provider.settings_config.get("config") {
        if config.is_object() {
            let settings_path = gemini_dir.join("settings.json");

            // Read existing settings to preserve mcpServers
            let mut settings: Value = if settings_path.exists() {
                let content = std::fs::read_to_string(&settings_path)?;
                serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
            } else {
                json!({})
            };

            // Merge config into settings
            if let (Some(settings_obj), Some(config_obj)) =
                (settings.as_object_mut(), config.as_object())
            {
                for (key, value) in config_obj {
                    settings_obj.insert(key.clone(), value.clone());
                }
            }

            let content = serde_json::to_string_pretty(&settings)?;
            std::fs::write(&settings_path, content)?;
        }
    }

    Ok(())
}

/// Read current live settings for an app type
pub fn read_live_settings(
    app_type: &AppType,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    match app_type {
        AppType::Claude => {
            let path = home.join(".claude").join("settings.json");
            if !path.exists() {
                return Err("Claude settings file not found".into());
            }
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        }
        AppType::Codex => {
            let codex_dir = home.join(".codex");
            let auth_path = codex_dir.join("auth.json");
            let config_path = codex_dir.join("config.toml");

            let auth: Value = if auth_path.exists() {
                let content = std::fs::read_to_string(&auth_path)?;
                serde_json::from_str(&content)?
            } else {
                json!({})
            };

            let config = if config_path.exists() {
                std::fs::read_to_string(&config_path)?
            } else {
                String::new()
            };

            Ok(json!({
                "auth": auth,
                "config": config
            }))
        }
        AppType::Gemini => {
            let gemini_dir = home.join(".gemini");
            let env_path = gemini_dir.join(".env");
            let settings_path = gemini_dir.join("settings.json");

            // Read .env file
            let mut env_map: serde_json::Map<String, Value> = serde_json::Map::new();
            if env_path.exists() {
                let content = std::fs::read_to_string(&env_path)?;
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        env_map.insert(key.trim().to_string(), json!(value.trim()));
                    }
                }
            }

            // Read settings.json
            let config: Value = if settings_path.exists() {
                let content = std::fs::read_to_string(&settings_path)?;
                serde_json::from_str(&content)?
            } else {
                json!({})
            };

            Ok(json!({
                "env": env_map,
                "config": config
            }))
        }
        AppType::ProxyCast => Ok(json!({})),
    }
}
