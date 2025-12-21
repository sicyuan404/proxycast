//! 日志管理模块
use chrono::{Duration, Local, Utc};
use flate2::write::GzEncoder;
use flate2::Compression;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct LogStoreConfig {
    pub max_logs: usize,
    pub retention_days: u32,
    pub max_file_size: u64,
    pub enable_file_logging: bool,
}

impl Default for LogStoreConfig {
    fn default() -> Self {
        Self {
            max_logs: 1000,
            retention_days: 7,
            max_file_size: 10 * 1024 * 1024,
            enable_file_logging: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub struct LogStore {
    logs: VecDeque<LogEntry>,
    max_logs: usize,
    config: LogStoreConfig,
    log_file_path: Option<PathBuf>,
}

impl Default for LogStore {
    fn default() -> Self {
        // 默认日志文件路径: ~/.proxycast/logs/proxycast.log
        let log_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".proxycast")
            .join("logs");

        // 创建日志目录
        let _ = fs::create_dir_all(&log_dir);

        let log_file = log_dir.join("proxycast.log");

        let config = LogStoreConfig::default();

        Self {
            logs: VecDeque::new(),
            max_logs: config.max_logs,
            config,
            log_file_path: Some(log_file),
        }
    }
}

impl LogStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(logging: &crate::config::LoggingConfig) -> Self {
        let mut store = Self::default();
        store.config.retention_days = logging.retention_days;
        store.config.enable_file_logging = logging.enabled;
        store.max_logs = store.config.max_logs;
        store
    }

    pub fn add(&mut self, level: &str, message: &str) {
        let sanitized = sanitize_log_message(message);
        let now = Utc::now();
        let entry = LogEntry {
            timestamp: now.to_rfc3339(),
            level: level.to_string(),
            message: sanitized.clone(),
        };

        self.logs.push_back(entry.clone());

        // 写入日志文件
        if self.config.enable_file_logging {
            if let Some(ref path) = self.log_file_path {
                self.rotate_log_file_if_needed(path);
                let local_time = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
                let log_line = format!("{} [{}] {}\n", local_time, level.to_uppercase(), sanitized);

                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                    let _ = file.write_all(log_line.as_bytes());
                }
                self.prune_old_logs(path);
            }
        }

        // 保持日志数量在限制内
        if self.logs.len() > self.max_logs {
            self.logs.pop_front();
        }
    }

    /// 记录原始响应到单独的文件（用于调试）
    pub fn log_raw_response(&self, request_id: &str, body: &str) {
        if let Some(ref log_path) = self.log_file_path {
            let log_dir = log_path.parent().unwrap_or(std::path::Path::new("."));
            let raw_file = log_dir.join(format!("raw_response_{request_id}.txt"));
            let sanitized = sanitize_log_message(body);

            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&raw_file)
            {
                let _ = file.write_all(sanitized.as_bytes());
            }
        }
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.logs.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.logs.clear();
    }

    pub fn get_log_file_path(&self) -> Option<String> {
        self.log_file_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
    }

    fn rotate_log_file_if_needed(&self, path: &PathBuf) {
        let Ok(metadata) = fs::metadata(path) else {
            return;
        };

        if metadata.len() <= self.config.max_file_size {
            return;
        }

        let suffix = Local::now().format("%Y%m%d-%H%M%S");
        let rotated = path.with_file_name(format!(
            "{}.{}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            suffix
        ));

        let _ = fs::rename(path, &rotated);
        self.prune_old_logs(path);
    }

    fn prune_old_logs(&self, path: &PathBuf) {
        let Some(dir) = path.parent() else {
            return;
        };
        self.archive_old_logs(path);
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let cutoff = Utc::now() - Duration::days(self.config.retention_days as i64);
        let prefix = format!(
            "{}.",
            path.file_name().unwrap_or_default().to_string_lossy()
        );

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if !file_name.starts_with(&prefix) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let modified = chrono::DateTime::<Utc>::from(modified);
            if modified < cutoff {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    fn archive_old_logs(&self, path: &PathBuf) {
        let Some(dir) = path.parent() else {
            return;
        };
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let archive_cutoff = Utc::now() - Duration::days(7);
        let delete_cutoff = Utc::now() - Duration::days(30);
        let prefix = format!(
            "{}.",
            path.file_name().unwrap_or_default().to_string_lossy()
        );

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if !file_name.starts_with(&prefix) {
                continue;
            }
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let modified = chrono::DateTime::<Utc>::from(modified);

            if file_name.ends_with(".gz") {
                if modified < delete_cutoff {
                    let _ = fs::remove_file(path);
                }
                continue;
            }

            if modified >= archive_cutoff {
                continue;
            }

            let mut input = Vec::new();
            if let Ok(mut file) = fs::File::open(&path) {
                if file.read_to_end(&mut input).is_err() {
                    continue;
                }
            } else {
                continue;
            }

            let gz_path = path.with_extension(format!(
                "{}.gz",
                path.extension().unwrap_or_default().to_string_lossy()
            ));
            if let Ok(gz_file) = fs::File::create(&gz_path) {
                let mut encoder = GzEncoder::new(gz_file, Compression::default());
                if encoder.write_all(&input).is_ok() && encoder.finish().is_ok() {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

#[allow(dead_code)]
pub type SharedLogStore = Arc<RwLock<LogStore>>;

fn sanitize_log_message(message: &str) -> String {
    let patterns = [
        (r"Bearer\s+[A-Za-z0-9._-]+", "Bearer ***"),
        (
            r#"api[_-]?key["']?\s*[:=]\s*["']?[A-Za-z0-9._-]+"#,
            "api_key: ***",
        ),
        (r#"token["']?\s*[:=]\s*["']?[A-Za-z0-9._-]+"#, "token: ***"),
    ];

    let mut sanitized = message.to_string();
    for (pattern, replacement) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            sanitized = re.replace_all(&sanitized, replacement).to_string();
        }
    }
    sanitized
}
