import { Download, Trash2, ExternalLink, Loader2 } from "lucide-react";
import type { Skill } from "@/lib/api/skills";

interface SkillCardProps {
  skill: Skill;
  onInstall: (directory: string) => void;
  onUninstall: (directory: string) => void;
  installing: boolean;
}

export function SkillCard({
  skill,
  onInstall,
  onUninstall,
  installing,
}: SkillCardProps) {
  const handleAction = () => {
    if (installing) return;
    if (skill.installed) {
      onUninstall(skill.directory);
    } else {
      onInstall(skill.directory);
    }
  };

  const openGithub = () => {
    if (skill.readmeUrl) {
      window.open(skill.readmeUrl, "_blank");
    }
  };

  return (
    <div className="rounded-lg border bg-card p-4 hover:shadow-md transition-shadow">
      <div className="flex items-start justify-between mb-3">
        <div className="flex-1">
          <h3 className="font-semibold text-lg mb-1">{skill.name}</h3>
          {skill.repoOwner && skill.repoName && (
            <p className="text-xs text-muted-foreground">
              {skill.repoOwner}/{skill.repoName}
            </p>
          )}
        </div>
        {skill.installed && (
          <span className="rounded-full bg-green-100 px-2 py-1 text-xs font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
            已安装
          </span>
        )}
      </div>

      <p className="text-sm text-muted-foreground mb-4 line-clamp-3">
        {skill.description || "暂无描述"}
      </p>

      <div className="flex items-center gap-2">
        <button
          onClick={handleAction}
          disabled={installing}
          className={`flex-1 flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
            skill.installed
              ? "border border-red-500 text-red-500 hover:bg-red-50 dark:hover:bg-red-950/30"
              : "bg-primary text-primary-foreground hover:bg-primary/90"
          } disabled:opacity-50 disabled:cursor-not-allowed`}
        >
          {installing ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              {skill.installed ? "卸载中..." : "安装中..."}
            </>
          ) : (
            <>
              {skill.installed ? (
                <>
                  <Trash2 className="h-4 w-4" />
                  卸载
                </>
              ) : (
                <>
                  <Download className="h-4 w-4" />
                  安装
                </>
              )}
            </>
          )}
        </button>

        {skill.readmeUrl && (
          <button
            onClick={openGithub}
            className="rounded-lg border p-2 hover:bg-muted"
            title="在 GitHub 上查看"
          >
            <ExternalLink className="h-4 w-4" />
          </button>
        )}
      </div>
    </div>
  );
}
