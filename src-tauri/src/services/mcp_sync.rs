use crate::models::{AppType, McpServer};
use serde_json::{json, Map, Value};
use std::path::PathBuf;

/// Get the MCP config file path for an app type
#[allow(dead_code)]
pub fn get_mcp_config_path(app_type: &AppType) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match app_type {
        AppType::Claude => Some(home.join(".claude").join("settings.json")),
        AppType::Codex => Some(home.join(".codex").join("config.toml")),
        AppType::Gemini => Some(home.join(".gemini").join("settings.json")),
        AppType::ProxyCast => None,
    }
}

/// Sync all enabled MCP servers to their respective app configurations
pub fn sync_all_mcp_to_live(
    servers: &[McpServer],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Sync to Claude
    let claude_servers: Vec<&McpServer> = servers.iter().filter(|s| s.enabled_claude).collect();
    sync_mcp_to_claude(&claude_servers)?;

    // Sync to Codex
    let codex_servers: Vec<&McpServer> = servers.iter().filter(|s| s.enabled_codex).collect();
    sync_mcp_to_codex(&codex_servers)?;

    // Sync to Gemini
    let gemini_servers: Vec<&McpServer> = servers.iter().filter(|s| s.enabled_gemini).collect();
    sync_mcp_to_gemini(&gemini_servers)?;

    Ok(())
}

/// Sync MCP servers to a specific app
pub fn sync_mcp_to_app(
    app_type: &AppType,
    servers: &[McpServer],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let enabled_servers: Vec<&McpServer> = servers
        .iter()
        .filter(|s| match app_type {
            AppType::Claude => s.enabled_claude,
            AppType::Codex => s.enabled_codex,
            AppType::Gemini => s.enabled_gemini,
            AppType::ProxyCast => s.enabled_proxycast,
        })
        .collect();

    match app_type {
        AppType::Claude => sync_mcp_to_claude(&enabled_servers),
        AppType::Codex => sync_mcp_to_codex(&enabled_servers),
        AppType::Gemini => sync_mcp_to_gemini(&enabled_servers),
        AppType::ProxyCast => Ok(()),
    }
}

/// Sync MCP servers to Claude's settings.json
/// Claude uses the mcpServers field in ~/.claude/settings.json
fn sync_mcp_to_claude(
    servers: &[&McpServer],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let claude_dir = home.join(".claude");
    let config_path = claude_dir.join("settings.json");

    // Create directory if not exists
    std::fs::create_dir_all(&claude_dir)?;

    // Read existing settings
    let mut settings: Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Build mcpServers object - use name as key (not id which may be UUID)
    let mut mcp_servers = Map::new();
    for server in servers {
        if let Some(config) = server.server_config.as_object() {
            mcp_servers.insert(server.name.clone(), Value::Object(config.clone()));
        }
    }

    // Update settings with mcpServers
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("mcpServers".to_string(), Value::Object(mcp_servers));
    }

    // Write settings
    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&config_path, content)?;

    Ok(())
}

/// Sync MCP servers to Codex's config.toml
/// Codex uses [mcp_servers.*] sections in ~/.codex/config.toml
fn sync_mcp_to_codex(
    servers: &[&McpServer],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let codex_dir = home.join(".codex");
    let config_path = codex_dir.join("config.toml");

    // Create directory if not exists
    std::fs::create_dir_all(&codex_dir)?;

    // Read existing config
    let existing_content = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    // Remove existing [mcp_servers.*] sections
    let lines: Vec<&str> = existing_content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    let mut in_mcp_section = false;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with("[mcp_servers.") {
            in_mcp_section = true;
            continue;
        }
        if in_mcp_section && trimmed.starts_with('[') {
            in_mcp_section = false;
        }
        if !in_mcp_section {
            new_lines.push(line.to_string());
        }
    }

    // Add new MCP server sections - use name as key
    for server in servers {
        new_lines.push(String::new());
        new_lines.push(format!("[mcp_servers.{}]", server.name));

        if let Some(config) = server.server_config.as_object() {
            // Convert JSON config to TOML format
            if let Some(command) = config.get("command").and_then(|v| v.as_str()) {
                new_lines.push(format!("command = \"{command}\""));
            }

            if let Some(args) = config.get("args").and_then(|v| v.as_array()) {
                let args_str: Vec<String> = args
                    .iter()
                    .filter_map(|a| a.as_str())
                    .map(|s| format!("\"{s}\""))
                    .collect();
                new_lines.push(format!("args = [{}]", args_str.join(", ")));
            }

            if let Some(env) = config.get("env").and_then(|v| v.as_object()) {
                new_lines.push("[mcp_servers.".to_string() + &server.name + ".env]");
                for (key, value) in env {
                    if let Some(val) = value.as_str() {
                        new_lines.push(format!("{key} = \"{val}\""));
                    }
                }
            }
        }
    }

    // Write config
    let content = new_lines.join("\n");
    std::fs::write(&config_path, content)?;

    Ok(())
}

/// Sync MCP servers to Gemini's settings.json
/// Gemini uses the mcpServers field in ~/.gemini/settings.json
fn sync_mcp_to_gemini(
    servers: &[&McpServer],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let gemini_dir = home.join(".gemini");
    let settings_path = gemini_dir.join("settings.json");

    // Create directory if not exists
    std::fs::create_dir_all(&gemini_dir)?;

    // Read existing settings
    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Build mcpServers object - use name as key
    let mut mcp_servers = Map::new();
    for server in servers {
        if let Some(config) = server.server_config.as_object() {
            mcp_servers.insert(server.name.clone(), Value::Object(config.clone()));
        }
    }

    // Update settings with mcpServers
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("mcpServers".to_string(), Value::Object(mcp_servers));
    }

    // Write settings
    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, content)?;

    Ok(())
}

/// Remove a specific MCP server from an app's config
pub fn remove_mcp_from_app(
    app_type: &AppType,
    server_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match app_type {
        AppType::Claude => remove_mcp_from_claude(server_id),
        AppType::Codex => remove_mcp_from_codex(server_id),
        AppType::Gemini => remove_mcp_from_gemini(server_id),
        AppType::ProxyCast => Ok(()),
    }
}

fn remove_mcp_from_claude(server_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let mut settings: Value = serde_json::from_str(&content)?;

    if let Some(mcp_servers) = settings
        .as_object_mut()
        .and_then(|o| o.get_mut("mcpServers"))
        .and_then(|v| v.as_object_mut())
    {
        mcp_servers.remove(server_id);
    }

    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&config_path, content)?;

    Ok(())
}

fn remove_mcp_from_codex(server_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_path = home.join(".codex").join("config.toml");

    if !config_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    let section_header = format!("[mcp_servers.{server_id}]");
    let env_header = format!("[mcp_servers.{server_id}.env]");
    let mut skip_section = false;

    for line in &lines {
        let trimmed = line.trim();

        // Check if this is the section we want to remove
        if trimmed == section_header || trimmed == env_header {
            skip_section = true;
            continue;
        }

        // Check if we've reached a new section
        if skip_section && trimmed.starts_with('[') {
            skip_section = false;
        }

        if !skip_section {
            new_lines.push(line.to_string());
        }
    }

    let content = new_lines.join("\n");
    std::fs::write(&config_path, content)?;

    Ok(())
}

fn remove_mcp_from_gemini(server_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let settings_path = home.join(".gemini").join("settings.json");

    if !settings_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&settings_path)?;
    let mut settings: Value = serde_json::from_str(&content)?;

    if let Some(mcp_servers) = settings
        .as_object_mut()
        .and_then(|o| o.get_mut("mcpServers"))
        .and_then(|v| v.as_object_mut())
    {
        mcp_servers.remove(server_id);
    }

    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, content)?;

    Ok(())
}

/// Remove a specific MCP server from all apps
pub fn remove_mcp_from_all_apps(
    server_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    remove_mcp_from_claude(server_id)?;
    remove_mcp_from_codex(server_id)?;
    remove_mcp_from_gemini(server_id)?;
    Ok(())
}

/// Import MCP servers from Claude's settings.json
pub fn import_mcp_from_claude(
) -> Result<Vec<crate::models::McpServer>, Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_path = home.join(".claude").join("settings.json");

    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let settings: Value = serde_json::from_str(&content)?;

    let mut servers = Vec::new();

    if let Some(mcp_servers) = settings.get("mcpServers").and_then(|v| v.as_object()) {
        for (id, config) in mcp_servers {
            let server = crate::models::McpServer {
                id: id.clone(),
                name: id.clone(),
                server_config: config.clone(),
                description: None,
                enabled_proxycast: false,
                enabled_claude: true,
                enabled_codex: false,
                enabled_gemini: false,
                created_at: Some(chrono::Utc::now().timestamp()),
            };
            servers.push(server);
        }
    }

    Ok(servers)
}

/// Import MCP servers from Codex's config.toml
pub fn import_mcp_from_codex(
) -> Result<Vec<crate::models::McpServer>, Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_path = home.join(".codex").join("config.toml");

    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let mut servers = Vec::new();
    let mut current_server_id: Option<String> = None;
    let mut current_config: Map<String, Value> = Map::new();
    let mut current_env: Map<String, Value> = Map::new();
    let mut in_env_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for [mcp_servers.name] section
        if trimmed.starts_with("[mcp_servers.") && trimmed.ends_with(']') {
            // Save previous server if any
            if let Some(ref id) = current_server_id {
                if !current_env.is_empty() {
                    current_config.insert("env".to_string(), Value::Object(current_env.clone()));
                }
                let server = crate::models::McpServer {
                    id: id.clone(),
                    name: id.clone(),
                    server_config: Value::Object(current_config.clone()),
                    description: None,
                    enabled_proxycast: false,
                    enabled_claude: false,
                    enabled_codex: true,
                    enabled_gemini: false,
                    created_at: Some(chrono::Utc::now().timestamp()),
                };
                servers.push(server);
            }

            // Parse new server ID
            let section = &trimmed[13..trimmed.len() - 1]; // Remove "[mcp_servers." and "]"
            if section.ends_with(".env") {
                in_env_section = true;
            } else {
                current_server_id = Some(section.to_string());
                current_config = Map::new();
                current_env = Map::new();
                in_env_section = false;
            }
            continue;
        }

        // Check for other sections
        if trimmed.starts_with('[') {
            // Save previous server if any
            if let Some(ref id) = current_server_id {
                if !current_env.is_empty() {
                    current_config.insert("env".to_string(), Value::Object(current_env.clone()));
                }
                let server = crate::models::McpServer {
                    id: id.clone(),
                    name: id.clone(),
                    server_config: Value::Object(current_config.clone()),
                    description: None,
                    enabled_proxycast: false,
                    enabled_claude: false,
                    enabled_codex: true,
                    enabled_gemini: false,
                    created_at: Some(chrono::Utc::now().timestamp()),
                };
                servers.push(server);
                current_server_id = None;
            }
            in_env_section = false;
            continue;
        }

        // Parse key = value
        if current_server_id.is_some() && trimmed.contains('=') {
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                if in_env_section {
                    current_env.insert(key.to_string(), Value::String(value.to_string()));
                } else if key == "command" {
                    current_config.insert(key.to_string(), Value::String(value.to_string()));
                } else if key == "args" {
                    // Parse array: ["arg1", "arg2"]
                    if value.starts_with('[') && value.ends_with(']') {
                        let args_str = &value[1..value.len() - 1];
                        let args: Vec<Value> = args_str
                            .split(',')
                            .map(|s| Value::String(s.trim().trim_matches('"').to_string()))
                            .collect();
                        current_config.insert(key.to_string(), Value::Array(args));
                    }
                }
            }
        }
    }

    // Save last server if any
    if let Some(ref id) = current_server_id {
        if !current_env.is_empty() {
            current_config.insert("env".to_string(), Value::Object(current_env));
        }
        let server = crate::models::McpServer {
            id: id.clone(),
            name: id.clone(),
            server_config: Value::Object(current_config),
            description: None,
            enabled_proxycast: false,
            enabled_claude: false,
            enabled_codex: true,
            enabled_gemini: false,
            created_at: Some(chrono::Utc::now().timestamp()),
        };
        servers.push(server);
    }

    Ok(servers)
}

/// Import MCP servers from Gemini's settings.json
pub fn import_mcp_from_gemini(
) -> Result<Vec<crate::models::McpServer>, Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let settings_path = home.join(".gemini").join("settings.json");

    if !settings_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&settings_path)?;
    let settings: Value = serde_json::from_str(&content)?;

    let mut servers = Vec::new();

    if let Some(mcp_servers) = settings.get("mcpServers").and_then(|v| v.as_object()) {
        for (id, config) in mcp_servers {
            let server = crate::models::McpServer {
                id: id.clone(),
                name: id.clone(),
                server_config: config.clone(),
                description: None,
                enabled_proxycast: false,
                enabled_claude: false,
                enabled_codex: false,
                enabled_gemini: true,
                created_at: Some(chrono::Utc::now().timestamp()),
            };
            servers.push(server);
        }
    }

    Ok(servers)
}

/// Import MCP servers from a specific app
pub fn import_mcp_from_app(
    app_type: &AppType,
) -> Result<Vec<crate::models::McpServer>, Box<dyn std::error::Error + Send + Sync>> {
    match app_type {
        AppType::Claude => import_mcp_from_claude(),
        AppType::Codex => import_mcp_from_codex(),
        AppType::Gemini => import_mcp_from_gemini(),
        AppType::ProxyCast => Ok(Vec::new()),
    }
}
