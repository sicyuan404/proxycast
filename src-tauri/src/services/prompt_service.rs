use crate::database::dao::prompts::PromptDao;
use crate::database::DbConnection;
use crate::models::{AppType, Prompt};
use crate::services::prompt_sync;
use std::collections::HashMap;

pub struct PromptService;

#[allow(dead_code)]
impl PromptService {
    /// Get all prompts for an app type
    pub fn get_all(db: &DbConnection, app_type: &str) -> Result<Vec<Prompt>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        PromptDao::get_all(&conn, app_type).map_err(|e| e.to_string())
    }

    /// Get all prompts as a HashMap (for frontend)
    pub fn get_all_map(
        db: &DbConnection,
        app_type: &str,
    ) -> Result<HashMap<String, Prompt>, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        PromptDao::get_all_map(&conn, app_type).map_err(|e| e.to_string())
    }

    /// Upsert a prompt (insert or update)
    /// If the prompt is enabled, sync to live file
    pub fn upsert(db: &DbConnection, app_type: &str, prompt: Prompt) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        PromptDao::upsert(&conn, &prompt).map_err(|e| e.to_string())?;

        // If this prompt is enabled, sync to live file
        if prompt.enabled {
            let app = app_type.parse::<AppType>().map_err(|e| e.to_string())?;
            prompt_sync::write_live_prompt(&app, &prompt.content)?;
        }

        Ok(())
    }

    /// Add a new prompt
    pub fn add(db: &DbConnection, prompt: Prompt) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        PromptDao::insert(&conn, &prompt).map_err(|e| e.to_string())
    }

    /// Update an existing prompt
    /// If the prompt is enabled, sync to live file
    pub fn update(db: &DbConnection, prompt: Prompt) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        PromptDao::update(&conn, &prompt).map_err(|e| e.to_string())?;

        // If this prompt is enabled, sync to live file
        if prompt.enabled {
            let app = prompt
                .app_type
                .parse::<AppType>()
                .map_err(|e| e.to_string())?;
            prompt_sync::write_live_prompt(&app, &prompt.content)?;
        }

        Ok(())
    }

    /// Delete a prompt
    /// Cannot delete an enabled prompt
    pub fn delete(db: &DbConnection, app_type: &str, id: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // Check if prompt is enabled
        if let Ok(Some(prompt)) = PromptDao::get_by_id(&conn, app_type, id) {
            if prompt.enabled {
                return Err("Cannot delete an enabled prompt. Disable it first.".to_string());
            }
        }

        PromptDao::delete(&conn, app_type, id).map_err(|e| e.to_string())
    }

    /// Enable a prompt and sync to live file
    /// This will:
    /// 1. Backfill the current live file content to the currently enabled prompt (if any)
    /// 2. Disable all other prompts
    /// 3. Enable the specified prompt
    /// 4. Write the prompt content to the live file
    pub fn enable(db: &DbConnection, app_type: &str, id: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let app = app_type.parse::<AppType>().map_err(|e| e.to_string())?;

        // Step 1: Backfill current live content to the currently enabled prompt
        if let Ok(Some(live_content)) = prompt_sync::read_live_prompt(&app) {
            if !live_content.trim().is_empty() {
                if let Ok(Some(mut current_enabled)) = PromptDao::get_enabled(&conn, app_type) {
                    // Update the current enabled prompt with live content
                    current_enabled.content = live_content.clone();
                    current_enabled.updated_at = Some(chrono::Utc::now().timestamp());
                    let _ = PromptDao::update(&conn, &current_enabled);
                    tracing::info!(
                        "Backfilled live content to enabled prompt: {}",
                        current_enabled.id
                    );
                } else {
                    // No enabled prompt, check if we should create a backup
                    let prompts = PromptDao::get_all(&conn, app_type).map_err(|e| e.to_string())?;
                    let content_exists = prompts
                        .iter()
                        .any(|p| p.content.trim() == live_content.trim());

                    if !content_exists {
                        // Create a backup prompt
                        let timestamp = chrono::Utc::now().timestamp();
                        let backup = Prompt {
                            id: format!("backup-{timestamp}"),
                            app_type: app_type.to_string(),
                            name: format!(
                                "Original Prompt {}",
                                chrono::Local::now().format("%Y-%m-%d %H:%M")
                            ),
                            content: live_content,
                            description: Some("Auto-backup of original prompt".to_string()),
                            enabled: false,
                            created_at: Some(timestamp),
                            updated_at: Some(timestamp),
                        };
                        let _ = PromptDao::insert(&conn, &backup);
                        tracing::info!("Created backup prompt: {}", backup.id);
                    }
                }
            }
        }

        // Step 2 & 3: Enable the specified prompt (this also disables others)
        PromptDao::enable(&conn, app_type, id).map_err(|e| e.to_string())?;

        // Step 4: Write to live file
        if let Ok(Some(prompt)) = PromptDao::get_by_id(&conn, app_type, id) {
            prompt_sync::write_live_prompt(&app, &prompt.content)?;
            tracing::info!("Synced prompt {} to live file", id);
        }

        Ok(())
    }

    /// Disable a prompt
    pub fn disable(db: &DbConnection, app_type: &str, id: &str) -> Result<(), String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // Get the prompt and set enabled to false
        if let Ok(Some(mut prompt)) = PromptDao::get_by_id(&conn, app_type, id) {
            prompt.enabled = false;
            prompt.updated_at = Some(chrono::Utc::now().timestamp());
            PromptDao::update(&conn, &prompt).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Import prompt from live file
    pub fn import_from_file(db: &DbConnection, app_type: &str) -> Result<String, String> {
        let app = app_type.parse::<AppType>().map_err(|e| e.to_string())?;
        let content = prompt_sync::read_live_prompt(&app)?
            .ok_or_else(|| "Prompt file does not exist or is empty".to_string())?;

        if content.trim().is_empty() {
            return Err("Prompt file is empty".to_string());
        }

        let timestamp = chrono::Utc::now().timestamp();
        let id = format!("imported-{timestamp}");

        let prompt = Prompt {
            id: id.clone(),
            app_type: app_type.to_string(),
            name: format!(
                "Imported Prompt {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M")
            ),
            content,
            description: Some("Imported from existing config file".to_string()),
            enabled: false,
            created_at: Some(timestamp),
            updated_at: Some(timestamp),
        };

        let conn = db.lock().map_err(|e| e.to_string())?;
        PromptDao::insert(&conn, &prompt).map_err(|e| e.to_string())?;

        Ok(id)
    }

    /// Get current live file content
    pub fn get_live_content(app_type: &str) -> Result<Option<String>, String> {
        let app = app_type.parse::<AppType>().map_err(|e| e.to_string())?;
        prompt_sync::read_live_prompt(&app)
    }

    /// Import from file on first launch (if no prompts exist)
    pub fn import_on_first_launch(db: &DbConnection, app_type: &str) -> Result<usize, String> {
        let conn = db.lock().map_err(|e| e.to_string())?;

        // Check if prompts already exist
        let existing = PromptDao::get_all(&conn, app_type).map_err(|e| e.to_string())?;
        if !existing.is_empty() {
            return Ok(0);
        }

        // Read from live file
        let app = app_type.parse::<AppType>().map_err(|e| e.to_string())?;
        let content = match prompt_sync::read_live_prompt(&app) {
            Ok(Some(c)) if !c.trim().is_empty() => c,
            _ => return Ok(0),
        };

        tracing::info!("Found prompt file, auto-importing for {}", app_type);

        let timestamp = chrono::Utc::now().timestamp();
        let prompt = Prompt {
            id: format!("auto-imported-{timestamp}"),
            app_type: app_type.to_string(),
            name: format!(
                "Auto-imported {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M")
            ),
            content,
            description: Some("Automatically imported on first launch".to_string()),
            enabled: true, // Enable on first import
            created_at: Some(timestamp),
            updated_at: Some(timestamp),
        };

        PromptDao::insert(&conn, &prompt).map_err(|e| e.to_string())?;
        tracing::info!("Auto-imported prompt for {}", app_type);

        Ok(1)
    }

    // Legacy method for compatibility
    pub fn set_current(db: &DbConnection, app_type: &str, id: &str) -> Result<(), String> {
        Self::enable(db, app_type, id)
    }
}
