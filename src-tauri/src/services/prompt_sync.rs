use crate::models::AppType;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Get the prompt file path for an app
/// Claude: ~/CLAUDE.md (user's home directory)
/// Codex: ~/AGENTS.md
/// Gemini: ~/GEMINI.md
pub fn get_prompt_file_path(app: &AppType) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match app {
        AppType::Claude => Some(home.join("CLAUDE.md")),
        AppType::Codex => Some(home.join("AGENTS.md")),
        AppType::Gemini => Some(home.join("GEMINI.md")),
        AppType::ProxyCast => None, // ProxyCast doesn't have a prompt file
    }
}

/// Get the prompt file name for an app
#[allow(dead_code)]
pub fn get_prompt_filename(app: &AppType) -> &'static str {
    match app {
        AppType::Claude => "CLAUDE.md",
        AppType::Codex => "AGENTS.md",
        AppType::Gemini => "GEMINI.md",
        AppType::ProxyCast => "",
    }
}

/// Read current content from the live prompt file
pub fn read_live_prompt(app: &AppType) -> Result<Option<String>, String> {
    let path = get_prompt_file_path(app).ok_or("Cannot determine prompt file path")?;

    if !path.exists() {
        return Ok(None);
    }

    fs::read_to_string(&path)
        .map(Some)
        .map_err(|e| format!("Failed to read prompt file: {e}"))
}

/// Write content to the live prompt file (atomic write)
pub fn write_live_prompt(app: &AppType, content: &str) -> Result<(), String> {
    let path = get_prompt_file_path(app).ok_or("Cannot determine prompt file path")?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }

    // Atomic write: write to temp file first, then rename
    let temp_path = path.with_extension("md.tmp");

    let mut file =
        fs::File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {e}"))?;

    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write content: {e}"))?;

    file.sync_all()
        .map_err(|e| format!("Failed to sync file: {e}"))?;

    fs::rename(&temp_path, &path).map_err(|e| format!("Failed to rename file: {e}"))?;

    Ok(())
}

/// Delete the live prompt file
#[allow(dead_code)]
pub fn delete_live_prompt(app: &AppType) -> Result<(), String> {
    let path = get_prompt_file_path(app).ok_or("Cannot determine prompt file path")?;

    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete prompt file: {e}"))?;
    }

    Ok(())
}
