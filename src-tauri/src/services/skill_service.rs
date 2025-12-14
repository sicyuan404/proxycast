use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;

use crate::models::{AppType, Skill, SkillMetadata, SkillRepo, SkillState};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

pub struct SkillService {
    client: Client,
}

impl SkillService {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    /// 获取技能安装目录
    fn get_skills_dir(app_type: &AppType) -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Failed to get home directory"))?;

        let skills_dir = match app_type {
            AppType::Claude => home.join(".claude").join("skills"),
            AppType::Codex => home.join(".codex").join("skills"),
            AppType::Gemini => home.join(".gemini").join("skills"),
            AppType::ProxyCast => home.join(".proxycast").join("skills"),
        };

        Ok(skills_dir)
    }

    /// 列出所有技能
    pub async fn list_skills(
        &self,
        app_type: &AppType,
        repos: &[SkillRepo],
        installed_states: &HashMap<String, SkillState>,
    ) -> Result<Vec<Skill>> {
        let mut all_skills: HashMap<String, Skill> = HashMap::new();

        // 1. 从启用的仓库获取技能
        let enabled_repos: Vec<_> = repos.iter().filter(|r| r.enabled).collect();

        for repo in enabled_repos {
            match timeout(
                DOWNLOAD_TIMEOUT,
                self.fetch_skills_from_repo(repo, app_type, installed_states),
            )
            .await
            {
                Ok(Ok(skills)) => {
                    for skill in skills {
                        all_skills.insert(skill.key.clone(), skill);
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Failed to fetch skills from {}/{}: {}",
                        repo.owner,
                        repo.name,
                        e
                    );
                }
                Err(_) => {
                    tracing::warn!("Timeout fetching skills from {}/{}", repo.owner, repo.name);
                }
            }
        }

        // 2. 添加本地已安装但不在任何仓库中的技能
        let skills_dir = Self::get_skills_dir(app_type)?;
        if skills_dir.exists() {
            if let Ok(entries) = fs::read_dir(&skills_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let directory = entry.file_name().to_string_lossy().to_string();

                        // 检查是否已有相同 directory 的 skill（按 directory 去重）
                        let already_exists = all_skills.values().any(|s| s.directory == directory);

                        if !already_exists {
                            let key = format!("local:{}", directory);
                            let skill_md = entry.path().join("SKILL.md");
                            let (name, description) = if skill_md.exists() {
                                self.parse_skill_metadata(&skill_md)
                                    .map(|m| {
                                        (
                                            m.name.unwrap_or_else(|| directory.clone()),
                                            m.description.unwrap_or_default(),
                                        )
                                    })
                                    .unwrap_or_else(|_| (directory.clone(), String::new()))
                            } else {
                                (directory.clone(), String::new())
                            };

                            all_skills.insert(
                                key.clone(),
                                Skill {
                                    key,
                                    name,
                                    description,
                                    directory: directory.clone(),
                                    readme_url: None,
                                    installed: true,
                                    repo_owner: None,
                                    repo_name: None,
                                    repo_branch: None,
                                },
                            );
                        }
                    }
                }
            }
        }

        // 3. 排序并返回
        let mut skills: Vec<Skill> = all_skills.into_values().collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(skills)
    }

    /// 从仓库获取技能列表
    async fn fetch_skills_from_repo(
        &self,
        repo: &SkillRepo,
        app_type: &AppType,
        installed_states: &HashMap<String, SkillState>,
    ) -> Result<Vec<Skill>> {
        let zip_url = format!(
            "https://github.com/{}/{}/archive/refs/heads/{}.zip",
            repo.owner, repo.name, repo.branch
        );

        // 下载 ZIP
        let response = self
            .client
            .get(&zip_url)
            .send()
            .await
            .context("Failed to download repository")?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP {}: {}", response.status(), zip_url));
        }

        let bytes = response.bytes().await.context("Failed to read response")?;

        // 解压并扫描
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).context("Failed to open ZIP archive")?;

        let mut skills = Vec::new();
        let repo_key_prefix = format!("{}/{}:", repo.owner, repo.name);

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).context("Failed to read ZIP entry")?;
            let file_path = file.name().to_string();

            if file_path.ends_with("/SKILL.md") || file_path.ends_with("\\SKILL.md") {
                let path = Path::new(&file_path);
                let directory = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // 读取并解析 SKILL.md
                let mut content = String::new();
                use std::io::Read;
                file.read_to_string(&mut content)
                    .context("Failed to read SKILL.md")?;

                let metadata = self.parse_skill_metadata_from_content(&content)?;
                let name = metadata.name.unwrap_or_else(|| directory.clone());
                let description = metadata.description.unwrap_or_default();

                let key = format!("{}{}", repo_key_prefix, directory);
                let app_key = format!("{}:{}", app_type.to_string().to_lowercase(), directory);
                let installed = installed_states.contains_key(&app_key);

                let readme_url = Some(format!(
                    "https://github.com/{}/{}/blob/{}/{}/SKILL.md",
                    repo.owner,
                    repo.name,
                    repo.branch,
                    path.parent().unwrap().to_str().unwrap_or("")
                ));

                skills.push(Skill {
                    key,
                    name,
                    description,
                    directory,
                    readme_url,
                    installed,
                    repo_owner: Some(repo.owner.clone()),
                    repo_name: Some(repo.name.clone()),
                    repo_branch: Some(repo.branch.clone()),
                });
            }
        }

        Ok(skills)
    }

    /// 安装技能
    pub async fn install_skill(
        &self,
        app_type: &AppType,
        repo_owner: &str,
        repo_name: &str,
        repo_branch: &str,
        directory: &str,
    ) -> Result<()> {
        let skills_dir = Self::get_skills_dir(app_type)?;
        fs::create_dir_all(&skills_dir).context("Failed to create skills directory")?;

        let target_dir = skills_dir.join(directory);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir).context("Failed to remove existing skill")?;
        }

        // 尝试多个分支
        let branches = if repo_branch == "main" {
            vec!["main", "master"]
        } else {
            vec![repo_branch]
        };

        let mut last_error = None;

        for branch in branches {
            let zip_url = format!(
                "https://github.com/{}/{}/archive/refs/heads/{}.zip",
                repo_owner, repo_name, branch
            );

            match self
                .download_and_extract(&zip_url, &target_dir, directory)
                .await
            {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Failed to install skill")))
    }

    /// 下载并解压技能
    async fn download_and_extract(
        &self,
        zip_url: &str,
        target_dir: &Path,
        directory: &str,
    ) -> Result<()> {
        let response = self
            .client
            .get(zip_url)
            .send()
            .await
            .context("Failed to download")?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP {}", response.status()));
        }

        let bytes = response.bytes().await.context("Failed to read response")?;
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).context("Failed to open ZIP")?;

        // 查找技能目录
        let skill_prefix = format!("/{}/", directory);
        let mut found = false;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let file_path = file.name().to_string();

            if file_path.contains(&skill_prefix) {
                found = true;
                let relative_path = file_path
                    .split(&skill_prefix)
                    .nth(1)
                    .unwrap_or("")
                    .to_string();

                if !relative_path.is_empty() {
                    let output_path = target_dir.join(&relative_path);

                    if file.is_dir() {
                        fs::create_dir_all(&output_path)?;
                    } else {
                        if let Some(parent) = output_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        let mut output_file = fs::File::create(&output_path)?;
                        std::io::copy(&mut file, &mut output_file)?;
                    }
                }
            }
        }

        if !found {
            return Err(anyhow!("Skill directory not found in archive"));
        }

        Ok(())
    }

    /// 卸载技能
    pub fn uninstall_skill(app_type: &AppType, directory: &str) -> Result<()> {
        let skills_dir = Self::get_skills_dir(app_type)?;
        let target_dir = skills_dir.join(directory);

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir).context("Failed to remove skill directory")?;
        }

        Ok(())
    }

    /// 解析技能元数据
    fn parse_skill_metadata(&self, path: &Path) -> Result<SkillMetadata> {
        let content = fs::read_to_string(path).context("Failed to read SKILL.md")?;
        self.parse_skill_metadata_from_content(&content)
    }

    /// 从内容解析技能元数据
    fn parse_skill_metadata_from_content(&self, content: &str) -> Result<SkillMetadata> {
        let content = content.trim_start_matches('\u{feff}');
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Ok(SkillMetadata {
                name: None,
                description: None,
            });
        }

        let front_matter = parts[1].trim();
        let meta: SkillMetadata =
            serde_yaml::from_str(front_matter).context("Failed to parse YAML front matter")?;

        Ok(meta)
    }
}
