//! Provider Pool 数据访问对象
//!
//! 提供凭证池的 CRUD 操作。

use crate::models::provider_pool_model::{
    CachedTokenInfo, CredentialData, PoolProviderType, ProviderCredential, ProviderPools,
};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection};

pub struct ProviderPoolDao;

impl ProviderPoolDao {
    /// 获取所有凭证
    pub fn get_all(conn: &Connection) -> Result<Vec<ProviderCredential>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT uuid, provider_type, credential_data, name, is_healthy, is_disabled,
                    check_health, check_model_name, not_supported_models, usage_count, error_count,
                    last_used, last_error_time, last_error_message, last_health_check_time,
                    last_health_check_model, created_at, updated_at
             FROM provider_pool_credentials
             ORDER BY provider_type, created_at ASC",
        )?;

        let rows = stmt.query_map([], |row| Self::row_to_credential(row))?;

        let mut credentials = Vec::new();
        for row in rows {
            if let Ok(cred) = row {
                credentials.push(cred);
            }
        }
        Ok(credentials)
    }

    /// 获取指定类型的凭证
    pub fn get_by_type(
        conn: &Connection,
        provider_type: &PoolProviderType,
    ) -> Result<Vec<ProviderCredential>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT uuid, provider_type, credential_data, name, is_healthy, is_disabled,
                    check_health, check_model_name, not_supported_models, usage_count, error_count,
                    last_used, last_error_time, last_error_message, last_health_check_time,
                    last_health_check_model, created_at, updated_at
             FROM provider_pool_credentials
             WHERE provider_type = ?1
             ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([provider_type.to_string()], |row| {
            Self::row_to_credential(row)
        })?;

        let mut credentials = Vec::new();
        for row in rows {
            if let Ok(cred) = row {
                credentials.push(cred);
            }
        }
        Ok(credentials)
    }

    /// 获取指定 UUID 的凭证
    pub fn get_by_uuid(
        conn: &Connection,
        uuid: &str,
    ) -> Result<Option<ProviderCredential>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT uuid, provider_type, credential_data, name, is_healthy, is_disabled,
                    check_health, check_model_name, not_supported_models, usage_count, error_count,
                    last_used, last_error_time, last_error_message, last_health_check_time,
                    last_health_check_model, created_at, updated_at
             FROM provider_pool_credentials
             WHERE uuid = ?1",
        )?;

        let mut rows = stmt.query([uuid])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_credential(row)?))
        } else {
            Ok(None)
        }
    }

    /// 根据名称获取凭证
    pub fn get_by_name(
        conn: &Connection,
        name: &str,
    ) -> Result<Option<ProviderCredential>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT uuid, provider_type, credential_data, name, is_healthy, is_disabled,
                    check_health, check_model_name, not_supported_models, usage_count, error_count,
                    last_used, last_error_time, last_error_message, last_health_check_time,
                    last_health_check_model, created_at, updated_at
             FROM provider_pool_credentials
             WHERE name = ?1",
        )?;

        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_credential(row)?))
        } else {
            Ok(None)
        }
    }

    /// 获取所有凭证按类型分组
    pub fn get_grouped(conn: &Connection) -> Result<ProviderPools, rusqlite::Error> {
        let all = Self::get_all(conn)?;
        let mut grouped: ProviderPools = std::collections::HashMap::new();

        for cred in all {
            grouped
                .entry(cred.provider_type)
                .or_insert_with(Vec::new)
                .push(cred);
        }

        Ok(grouped)
    }

    /// 插入新凭证
    pub fn insert(conn: &Connection, cred: &ProviderCredential) -> Result<(), rusqlite::Error> {
        let credential_json =
            serde_json::to_string(&cred.credential).unwrap_or_else(|_| "{}".to_string());
        let not_supported_models_json =
            serde_json::to_string(&cred.not_supported_models).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "INSERT INTO provider_pool_credentials
             (uuid, provider_type, credential_data, name, is_healthy, is_disabled,
              check_health, check_model_name, not_supported_models, usage_count, error_count,
              last_used, last_error_time, last_error_message, last_health_check_time,
              last_health_check_model, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                cred.uuid,
                cred.provider_type.to_string(),
                credential_json,
                cred.name,
                cred.is_healthy,
                cred.is_disabled,
                cred.check_health,
                cred.check_model_name,
                not_supported_models_json,
                cred.usage_count,
                cred.error_count,
                cred.last_used.map(|t| t.timestamp()),
                cred.last_error_time.map(|t| t.timestamp()),
                cred.last_error_message,
                cred.last_health_check_time.map(|t| t.timestamp()),
                cred.last_health_check_model,
                cred.created_at.timestamp(),
                cred.updated_at.timestamp(),
            ],
        )?;
        Ok(())
    }

    /// 更新凭证
    pub fn update(conn: &Connection, cred: &ProviderCredential) -> Result<(), rusqlite::Error> {
        let credential_json =
            serde_json::to_string(&cred.credential).unwrap_or_else(|_| "{}".to_string());
        let not_supported_models_json =
            serde_json::to_string(&cred.not_supported_models).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "UPDATE provider_pool_credentials SET
             provider_type = ?2, credential_data = ?3, name = ?4, is_healthy = ?5,
             is_disabled = ?6, check_health = ?7, check_model_name = ?8,
             not_supported_models = ?9, usage_count = ?10, error_count = ?11,
             last_used = ?12, last_error_time = ?13, last_error_message = ?14,
             last_health_check_time = ?15, last_health_check_model = ?16, updated_at = ?17
             WHERE uuid = ?1",
            params![
                cred.uuid,
                cred.provider_type.to_string(),
                credential_json,
                cred.name,
                cred.is_healthy,
                cred.is_disabled,
                cred.check_health,
                cred.check_model_name,
                not_supported_models_json,
                cred.usage_count,
                cred.error_count,
                cred.last_used.map(|t| t.timestamp()),
                cred.last_error_time.map(|t| t.timestamp()),
                cred.last_error_message,
                cred.last_health_check_time.map(|t| t.timestamp()),
                cred.last_health_check_model,
                cred.updated_at.timestamp(),
            ],
        )?;
        Ok(())
    }

    /// 删除凭证
    pub fn delete(conn: &Connection, uuid: &str) -> Result<bool, rusqlite::Error> {
        let affected = conn.execute(
            "DELETE FROM provider_pool_credentials WHERE uuid = ?1",
            [uuid],
        )?;
        Ok(affected > 0)
    }

    /// 更新健康状态
    pub fn update_health_status(
        conn: &Connection,
        uuid: &str,
        is_healthy: bool,
        error_count: u32,
        last_error_time: Option<DateTime<Utc>>,
        last_error_message: Option<&str>,
        last_health_check_time: Option<DateTime<Utc>>,
        last_health_check_model: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE provider_pool_credentials SET
             is_healthy = ?2, error_count = ?3, last_error_time = ?4,
             last_error_message = ?5, last_health_check_time = ?6,
             last_health_check_model = ?7, updated_at = ?8
             WHERE uuid = ?1",
            params![
                uuid,
                is_healthy,
                error_count,
                last_error_time.map(|t| t.timestamp()),
                last_error_message,
                last_health_check_time.map(|t| t.timestamp()),
                last_health_check_model,
                Utc::now().timestamp(),
            ],
        )?;
        Ok(())
    }

    /// 更新使用统计
    pub fn update_usage(
        conn: &Connection,
        uuid: &str,
        usage_count: u64,
        last_used: DateTime<Utc>,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE provider_pool_credentials SET
             usage_count = ?2, last_used = ?3, updated_at = ?4
             WHERE uuid = ?1",
            params![
                uuid,
                usage_count,
                last_used.timestamp(),
                Utc::now().timestamp()
            ],
        )?;
        Ok(())
    }

    /// 重置凭证计数器
    pub fn reset_counters(conn: &Connection, uuid: &str) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE provider_pool_credentials SET
             usage_count = 0, error_count = 0, is_healthy = 1,
             last_error_time = NULL, last_error_message = NULL, updated_at = ?2
             WHERE uuid = ?1",
            params![uuid, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    /// 重置指定类型的所有凭证健康状态
    pub fn reset_health_by_type(
        conn: &Connection,
        provider_type: &PoolProviderType,
    ) -> Result<usize, rusqlite::Error> {
        let affected = conn.execute(
            "UPDATE provider_pool_credentials SET
             is_healthy = 1, error_count = 0, last_error_time = NULL,
             last_error_message = NULL, updated_at = ?2
             WHERE provider_type = ?1",
            params![provider_type.to_string(), Utc::now().timestamp()],
        )?;
        Ok(affected)
    }

    /// 从数据库行转换为 ProviderCredential
    fn row_to_credential(row: &rusqlite::Row) -> Result<ProviderCredential, rusqlite::Error> {
        let uuid: String = row.get(0)?;
        let provider_type_str: String = row.get(1)?;
        let credential_json: String = row.get(2)?;
        let name: Option<String> = row.get(3)?;
        let is_healthy: bool = row.get(4)?;
        let is_disabled: bool = row.get(5)?;
        let check_health: bool = row.get(6)?;
        let check_model_name: Option<String> = row.get(7)?;
        let not_supported_models_json: Option<String> = row.get(8)?;
        let usage_count: u64 = row.get::<_, i64>(9)? as u64;
        let error_count: u32 = row.get::<_, i32>(10)? as u32;
        let last_used_ts: Option<i64> = row.get(11)?;
        let last_error_time_ts: Option<i64> = row.get(12)?;
        let last_error_message: Option<String> = row.get(13)?;
        let last_health_check_time_ts: Option<i64> = row.get(14)?;
        let last_health_check_model: Option<String> = row.get(15)?;
        let created_at_ts: i64 = row.get(16)?;
        let updated_at_ts: i64 = row.get(17)?;

        let provider_type: PoolProviderType =
            provider_type_str.parse().unwrap_or(PoolProviderType::Kiro);

        let credential: CredentialData = serde_json::from_str(&credential_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let not_supported_models: Vec<String> = not_supported_models_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(ProviderCredential {
            uuid,
            provider_type,
            credential,
            name,
            is_healthy,
            is_disabled,
            check_health,
            check_model_name,
            not_supported_models,
            usage_count,
            error_count,
            last_used: last_used_ts.and_then(|ts| Utc.timestamp_opt(ts, 0).single()),
            last_error_time: last_error_time_ts.and_then(|ts| Utc.timestamp_opt(ts, 0).single()),
            last_error_message,
            last_health_check_time: last_health_check_time_ts
                .and_then(|ts| Utc.timestamp_opt(ts, 0).single()),
            last_health_check_model,
            created_at: Utc
                .timestamp_opt(created_at_ts, 0)
                .single()
                .unwrap_or_default(),
            updated_at: Utc
                .timestamp_opt(updated_at_ts, 0)
                .single()
                .unwrap_or_default(),
            cached_token: None, // 从 get_token_cache 单独获取
        })
    }

    // ==================== Token 缓存操作 ====================

    /// 获取凭证的 Token 缓存信息
    pub fn get_token_cache(
        conn: &Connection,
        uuid: &str,
    ) -> Result<Option<CachedTokenInfo>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT cached_access_token, cached_refresh_token, token_expiry_time,
                    last_refresh_time, refresh_error_count, last_refresh_error
             FROM provider_pool_credentials
             WHERE uuid = ?1",
        )?;

        let mut rows = stmt.query([uuid])?;
        if let Some(row) = rows.next()? {
            let access_token: Option<String> = row.get(0)?;
            let refresh_token: Option<String> = row.get(1)?;
            let expiry_time_str: Option<String> = row.get(2)?;
            let last_refresh_str: Option<String> = row.get(3)?;
            let refresh_error_count: i32 = row.get::<_, Option<i32>>(4)?.unwrap_or(0);
            let last_refresh_error: Option<String> = row.get(5)?;

            // 如果没有缓存的 token，返回 None
            if access_token.is_none() {
                return Ok(None);
            }

            let expiry_time = expiry_time_str
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            let last_refresh = last_refresh_str
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            Ok(Some(CachedTokenInfo {
                access_token,
                refresh_token,
                expiry_time,
                last_refresh,
                refresh_error_count: refresh_error_count as u32,
                last_refresh_error,
            }))
        } else {
            Ok(None)
        }
    }

    /// 更新凭证的 Token 缓存
    pub fn update_token_cache(
        conn: &Connection,
        uuid: &str,
        token_info: &CachedTokenInfo,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE provider_pool_credentials SET
             cached_access_token = ?2,
             cached_refresh_token = ?3,
             token_expiry_time = ?4,
             last_refresh_time = ?5,
             refresh_error_count = ?6,
             last_refresh_error = ?7,
             updated_at = ?8
             WHERE uuid = ?1",
            params![
                uuid,
                token_info.access_token,
                token_info.refresh_token,
                token_info.expiry_time.map(|t| t.to_rfc3339()),
                token_info.last_refresh.map(|t| t.to_rfc3339()),
                token_info.refresh_error_count as i32,
                token_info.last_refresh_error,
                Utc::now().timestamp(),
            ],
        )?;
        Ok(())
    }

    /// 清除凭证的 Token 缓存
    pub fn clear_token_cache(conn: &Connection, uuid: &str) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE provider_pool_credentials SET
             cached_access_token = NULL,
             cached_refresh_token = NULL,
             token_expiry_time = NULL,
             last_refresh_time = NULL,
             refresh_error_count = 0,
             last_refresh_error = NULL,
             updated_at = ?2
             WHERE uuid = ?1",
            params![uuid, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    /// 记录 Token 刷新错误
    pub fn record_token_refresh_error(
        conn: &Connection,
        uuid: &str,
        error_message: &str,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE provider_pool_credentials SET
             refresh_error_count = COALESCE(refresh_error_count, 0) + 1,
             last_refresh_error = ?2,
             updated_at = ?3
             WHERE uuid = ?1",
            params![uuid, error_message, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    /// 重置 Token 刷新错误计数
    #[allow(dead_code)]
    pub fn reset_token_refresh_errors(
        conn: &Connection,
        uuid: &str,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE provider_pool_credentials SET
             refresh_error_count = 0,
             last_refresh_error = NULL,
             updated_at = ?2
             WHERE uuid = ?1",
            params![uuid, Utc::now().timestamp()],
        )?;
        Ok(())
    }
}
