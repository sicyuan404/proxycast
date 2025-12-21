//! 备份服务
//!
//! 提供数据库与配置备份的基础能力

use crate::database::{get_db_path, DbConnection};
use chrono::{DateTime, Duration, Utc};
use rusqlite::DatabaseName;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct BackupService {
    backup_dir: PathBuf,
    retention_days: u32,
}

impl BackupService {
    pub fn new(backup_dir: PathBuf, retention_days: u32) -> Result<Self, String> {
        std::fs::create_dir_all(&backup_dir)
            .map_err(|e| format!("无法创建备份目录 {:?}: {}", backup_dir, e))?;
        Ok(Self {
            backup_dir,
            retention_days,
        })
    }

    pub fn with_defaults() -> Result<Self, String> {
        let home = dirs::home_dir().ok_or_else(|| "无法获取主目录".to_string())?;
        let backup_dir = home.join(".proxycast").join("backups");
        Self::new(backup_dir, 7)
    }

    pub fn backup_database(&self) -> Result<PathBuf, String> {
        let db_path = get_db_path()?;
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = self.backup_dir.join(format!("proxycast_{}.db", timestamp));

        std::fs::copy(&db_path, &backup_path).map_err(|e| format!("备份失败: {}", e))?;

        self.cleanup_old_backups()?;
        Ok(backup_path)
    }

    pub fn backup_database_with_connection(&self, db: &DbConnection) -> Result<PathBuf, String> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = self.backup_dir.join(format!("proxycast_{}.db", timestamp));
        let conn = db.lock().map_err(|_| "数据库锁已被占用".to_string())?;
        let progress: Option<fn(rusqlite::backup::Progress)> = None;
        conn.backup(DatabaseName::Main, &backup_path, progress)
            .map_err(|e| format!("备份失败: {}", e))?;

        self.cleanup_old_backups()?;
        Ok(backup_path)
    }

    pub fn restore_database(&self, backup_path: &Path) -> Result<(), String> {
        if !backup_path.exists() {
            return Err("备份文件不存在".to_string());
        }
        let db_path = get_db_path()?;
        std::fs::copy(backup_path, db_path).map_err(|e| format!("恢复失败: {}", e))?;
        Ok(())
    }

    pub fn restore_database_with_connection(
        &self,
        db: &DbConnection,
        backup_path: &Path,
    ) -> Result<(), String> {
        if !backup_path.exists() {
            return Err("备份文件不存在".to_string());
        }
        let mut conn = db.lock().map_err(|_| "数据库锁已被占用".to_string())?;
        let progress: Option<fn(rusqlite::backup::Progress)> = None;
        conn.restore(DatabaseName::Main, backup_path, progress)
            .map_err(|e| format!("恢复失败: {}", e))?;
        Ok(())
    }

    pub fn list_backups(&self) -> Result<Vec<PathBuf>, String> {
        let mut backups = Vec::new();
        let entries =
            std::fs::read_dir(&self.backup_dir).map_err(|e| format!("无法读取备份目录: {}", e))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "db").unwrap_or(false) {
                backups.push(path);
            }
        }
        backups.sort();
        Ok(backups)
    }

    pub fn cleanup_old_backups(&self) -> Result<(), String> {
        let entries =
            std::fs::read_dir(&self.backup_dir).map_err(|e| format!("无法读取备份目录: {}", e))?;
        let cutoff = Utc::now() - Duration::days(self.retention_days as i64);

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let modified = DateTime::<Utc>::from(modified);
            if modified < cutoff {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    }

    pub fn backup_dir(&self) -> &PathBuf {
        &self.backup_dir
    }
}
