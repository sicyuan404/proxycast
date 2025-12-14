import { useState, useEffect, useCallback } from "react";
import { skillsApi, Skill, SkillRepo, AppType } from "@/lib/api/skills";

export function useSkills(app: AppType = "claude") {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [repos, setRepos] = useState<SkillRepo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSkills = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await skillsApi.getAll(app);
      setSkills(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [app]);

  const fetchRepos = useCallback(async () => {
    try {
      const data = await skillsApi.getRepos();
      setRepos(data);
    } catch (e) {
      console.error("Failed to fetch repos:", e);
    }
  }, []);

  useEffect(() => {
    fetchSkills();
    fetchRepos();
  }, [fetchSkills, fetchRepos]);

  const install = async (directory: string) => {
    await skillsApi.install(directory, app);
    await fetchSkills();
  };

  const uninstall = async (directory: string) => {
    await skillsApi.uninstall(directory, app);
    await fetchSkills();
  };

  const addRepo = async (repo: SkillRepo) => {
    await skillsApi.addRepo(repo);
    await fetchRepos();
    await fetchSkills();
  };

  const removeRepo = async (owner: string, name: string) => {
    await skillsApi.removeRepo(owner, name);
    await fetchRepos();
    await fetchSkills();
  };

  return {
    skills,
    repos,
    loading,
    error,
    refresh: fetchSkills,
    install,
    uninstall,
    addRepo,
    removeRepo,
  };
}
