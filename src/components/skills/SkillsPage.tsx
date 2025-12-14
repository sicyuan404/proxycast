import { useState, forwardRef, useImperativeHandle } from "react";
import { RefreshCw, Search, Settings } from "lucide-react";
import { useSkills } from "@/hooks/useSkills";
import { SkillCard } from "./SkillCard";
import { RepoManagerPanel } from "./RepoManagerPanel";
import type { AppType } from "@/lib/api/skills";

interface SkillsPageProps {
  initialApp?: AppType;
}

export interface SkillsPageRef {
  refresh: () => void;
  openRepoManager: () => void;
}

export const SkillsPage = forwardRef<SkillsPageRef, SkillsPageProps>(
  ({ initialApp = "claude" }, ref) => {
    const [app] = useState<AppType>(initialApp);
    const [searchQuery, setSearchQuery] = useState("");
    const [filterStatus, setFilterStatus] = useState<
      "all" | "installed" | "uninstalled"
    >("all");
    const [repoManagerOpen, setRepoManagerOpen] = useState(false);
    const [installingSkills, setInstallingSkills] = useState<Set<string>>(
      new Set(),
    );

    const {
      skills,
      repos,
      loading,
      error,
      refresh,
      install,
      uninstall,
      addRepo,
      removeRepo,
    } = useSkills(app);

    useImperativeHandle(ref, () => ({
      refresh,
      openRepoManager: () => setRepoManagerOpen(true),
    }));

    const handleInstall = async (directory: string) => {
      setInstallingSkills((prev) => new Set(prev).add(directory));
      try {
        await install(directory);
      } catch (e) {
        alert(`安装失败: ${e instanceof Error ? e.message : String(e)}`);
      } finally {
        setInstallingSkills((prev) => {
          const next = new Set(prev);
          next.delete(directory);
          return next;
        });
      }
    };

    const handleUninstall = async (directory: string) => {
      setInstallingSkills((prev) => new Set(prev).add(directory));
      try {
        await uninstall(directory);
      } catch (e) {
        alert(`卸载失败: ${e instanceof Error ? e.message : String(e)}`);
      } finally {
        setInstallingSkills((prev) => {
          const next = new Set(prev);
          next.delete(directory);
          return next;
        });
      }
    };

    const filteredSkills = skills.filter((skill) => {
      const matchesSearch =
        skill.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        skill.description.toLowerCase().includes(searchQuery.toLowerCase());

      const matchesFilter =
        filterStatus === "all" ||
        (filterStatus === "installed" && skill.installed) ||
        (filterStatus === "uninstalled" && !skill.installed);

      return matchesSearch && matchesFilter;
    });

    const installedCount = skills.filter((s) => s.installed).length;
    const uninstalledCount = skills.length - installedCount;

    return (
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold">Skills</h2>
            <p className="text-muted-foreground">浏览和安装 Claude Skills</p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={refresh}
              disabled={loading}
              className="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm hover:bg-muted disabled:opacity-50"
            >
              <RefreshCw
                className={`h-4 w-4 ${loading ? "animate-spin" : ""}`}
              />
              刷新
            </button>
            <button
              onClick={() => setRepoManagerOpen(true)}
              className="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm hover:bg-muted"
            >
              <Settings className="h-4 w-4" />
              仓库管理
            </button>
          </div>
        </div>

        {error && (
          <div className="rounded-lg border border-red-500 bg-red-50 p-4 text-red-700 dark:bg-red-950/30">
            {error}
          </div>
        )}

        {/* 搜索和过滤 */}
        <div className="flex items-center gap-4">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <input
              type="text"
              placeholder="搜索 skills..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full rounded-lg border bg-background pl-10 pr-4 py-2 text-sm"
            />
          </div>
        </div>

        {/* 过滤标签 */}
        <div className="flex flex-wrap gap-2">
          <button
            onClick={() => setFilterStatus("all")}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
              filterStatus === "all"
                ? "bg-primary text-primary-foreground"
                : "bg-muted hover:bg-muted/80"
            }`}
          >
            全部 ({skills.length})
          </button>
          <button
            onClick={() => setFilterStatus("installed")}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
              filterStatus === "installed"
                ? "bg-primary text-primary-foreground"
                : "bg-muted hover:bg-muted/80"
            }`}
          >
            已安装 ({installedCount})
          </button>
          <button
            onClick={() => setFilterStatus("uninstalled")}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
              filterStatus === "uninstalled"
                ? "bg-primary text-primary-foreground"
                : "bg-muted hover:bg-muted/80"
            }`}
          >
            未安装 ({uninstalledCount})
          </button>
        </div>

        {/* Skills 列表 */}
        {loading ? (
          <div className="flex items-center justify-center py-12">
            <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : filteredSkills.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
            <p>没有找到 skills</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
            {filteredSkills.map((skill) => (
              <SkillCard
                key={skill.key}
                skill={skill}
                onInstall={handleInstall}
                onUninstall={handleUninstall}
                installing={installingSkills.has(skill.directory)}
              />
            ))}
          </div>
        )}

        {/* 仓库管理面板 */}
        {repoManagerOpen && (
          <RepoManagerPanel
            repos={repos}
            onClose={() => setRepoManagerOpen(false)}
            onAddRepo={addRepo}
            onRemoveRepo={removeRepo}
            onRefresh={refresh}
          />
        )}
      </div>
    );
  },
);

SkillsPage.displayName = "SkillsPage";
