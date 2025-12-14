//! Skills 数据访问对象
//!
//! 提供 Skills 和 Skill Repos 的 CRUD 操作。

use crate::models::{SkillRepo, SkillState, SkillStates};
use chrono::DateTime;
use rusqlite::{params, Connection};
use std::collections::HashMap;

pub struct SkillDao;

impl SkillDao {
    /// 获取所有 Skills 状态
    pub fn get_skills(conn: &Connection) -> Result<SkillStates, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT directory, app_type, installed, installed_at FROM skills ORDER BY directory ASC, app_type ASC",
        )?;

        let skill_iter = stmt.query_map([], |row| {
            let directory: String = row.get(0)?;
            let app_type: String = row.get(1)?;
            let installed: bool = row.get(2)?;
            let installed_at_ts: i64 = row.get(3)?;

            let installed_at = DateTime::from_timestamp(installed_at_ts, 0).unwrap_or_default();

            // 构建复合 key："app_type:directory"
            let key = format!("{app_type}:{directory}");

            Ok((
                key,
                SkillState {
                    installed,
                    installed_at,
                },
            ))
        })?;

        let mut skills = HashMap::new();
        for skill_res in skill_iter {
            let (key, skill) = skill_res?;
            skills.insert(key, skill);
        }
        Ok(skills)
    }

    /// 更新 Skill 状态
    pub fn update_skill_state(
        conn: &Connection,
        key: &str,
        state: &SkillState,
    ) -> Result<(), rusqlite::Error> {
        // 解析 key: "app_type:directory"
        let parts: Vec<&str> = key.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let app_type = parts[0];
        let directory = parts[1];

        conn.execute(
            "INSERT OR REPLACE INTO skills (directory, app_type, installed, installed_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                directory,
                app_type,
                state.installed,
                state.installed_at.timestamp()
            ],
        )?;
        Ok(())
    }

    /// 获取所有 Skill 仓库
    pub fn get_skill_repos(conn: &Connection) -> Result<Vec<SkillRepo>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT owner, name, branch, enabled FROM skill_repos ORDER BY owner ASC, name ASC",
        )?;

        let repo_iter = stmt.query_map([], |row| {
            Ok(SkillRepo {
                owner: row.get(0)?,
                name: row.get(1)?,
                branch: row.get(2)?,
                enabled: row.get(3)?,
            })
        })?;

        let mut repos = Vec::new();
        for repo_res in repo_iter {
            repos.push(repo_res?);
        }
        Ok(repos)
    }

    /// 保存 Skill 仓库
    pub fn save_skill_repo(conn: &Connection, repo: &SkillRepo) -> Result<(), rusqlite::Error> {
        conn.execute(
            "INSERT OR REPLACE INTO skill_repos (owner, name, branch, enabled)
             VALUES (?1, ?2, ?3, ?4)",
            params![repo.owner, repo.name, repo.branch, repo.enabled],
        )?;
        Ok(())
    }

    /// 删除 Skill 仓库
    pub fn delete_skill_repo(
        conn: &Connection,
        owner: &str,
        name: &str,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "DELETE FROM skill_repos WHERE owner = ?1 AND name = ?2",
            params![owner, name],
        )?;
        Ok(())
    }

    /// 初始化默认 Skill 仓库
    pub fn init_default_skill_repos(conn: &Connection) -> Result<usize, rusqlite::Error> {
        use crate::models::skill_model::get_default_skill_repos;

        let existing_repos = Self::get_skill_repos(conn)?;
        if !existing_repos.is_empty() {
            return Ok(0);
        }

        let default_repos = get_default_skill_repos();
        let mut count = 0;

        for repo in default_repos {
            Self::save_skill_repo(conn, &repo)?;
            count += 1;
        }

        Ok(count)
    }
}
