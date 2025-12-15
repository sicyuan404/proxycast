use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub key: String,
    pub name: String,
    pub description: String,
    pub directory: String,
    #[serde(rename = "readmeUrl", skip_serializing_if = "Option::is_none")]
    pub readme_url: Option<String>,
    pub installed: bool,
    #[serde(rename = "repoOwner", skip_serializing_if = "Option::is_none")]
    pub repo_owner: Option<String>,
    #[serde(rename = "repoName", skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    #[serde(rename = "repoBranch", skip_serializing_if = "Option::is_none")]
    pub repo_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRepo {
    pub owner: String,
    pub name: String,
    pub branch: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillState {
    pub installed: bool,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl Default for SkillRepo {
    fn default() -> Self {
        Self {
            owner: String::new(),
            name: String::new(),
            branch: "main".to_string(),
            enabled: true,
        }
    }
}

#[allow(dead_code)]
impl SkillRepo {
    pub fn new(owner: String, name: String, branch: String) -> Self {
        Self {
            owner,
            name,
            branch,
            enabled: true,
        }
    }

    pub fn github_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.name)
    }

    pub fn zip_url(&self) -> String {
        format!(
            "https://github.com/{}/{}/archive/refs/heads/{}.zip",
            self.owner, self.name, self.branch
        )
    }
}

pub fn get_default_skill_repos() -> Vec<SkillRepo> {
    vec![
        SkillRepo {
            owner: "ComposioHQ".to_string(),
            name: "awesome-claude-skills".to_string(),
            branch: "main".to_string(),
            enabled: true,
        },
        SkillRepo {
            owner: "anthropics".to_string(),
            name: "skills".to_string(),
            branch: "main".to_string(),
            enabled: true,
        },
        SkillRepo {
            owner: "cexll".to_string(),
            name: "myclaude".to_string(),
            branch: "master".to_string(),
            enabled: true,
        },
    ]
}

pub type SkillStates = HashMap<String, SkillState>;
