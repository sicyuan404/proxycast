import React, { useEffect, useState, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  Puzzle,
  RefreshCw,
  Power,
  PowerOff,
  Trash2,
  FolderOpen,
  AlertCircle,
  CheckCircle,
  Clock,
  ChevronDown,
  ChevronUp,
  Plus,
  Package,
  Download,
  Cpu,
  Globe,
  Activity,
  FileCode,
} from "lucide-react";
import { PluginInstallDialog } from "./PluginInstallDialog";
import { PluginUninstallDialog } from "./PluginUninstallDialog";
import { PluginItemContextMenu } from "./PluginItemContextMenu";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";

interface PluginState {
  name: string;
  status: string;
  loaded_at: string;
  last_executed: string | null;
  execution_count: number;
  error_count: number;
  last_error: string | null;
}

interface PluginConfig {
  enabled: boolean;
  timeout_ms: number;
  settings: Record<string, unknown>;
}

interface PluginInfo {
  name: string;
  version: string;
  description: string;
  author: string | null;
  status: string;
  path: string;
  hooks: string[];
  config_schema: Record<string, unknown> | null;
  config: PluginConfig;
  state: PluginState;
}

interface PluginServiceStatus {
  enabled: boolean;
  plugin_count: number;
  plugins_dir: string;
}

/** 安装来源 */
interface InstallSource {
  type: "local" | "url" | "github";
  path?: string;
  url?: string;
  owner?: string;
  repo?: string;
  tag?: string;
}

/** 已安装插件信息（通过安装器安装的） */
interface InstalledPlugin {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string | null;
  install_path: string;
  installed_at: string;
  source: InstallSource;
  enabled: boolean;
}

/**
 * 推荐插件配置
 */
interface RecommendedPlugin {
  /** 插件 ID */
  id: string;
  /** 插件名称 */
  name: string;
  /** 插件描述 */
  description: string;
  /** 图标组件 */
  icon: React.ComponentType<{ className?: string }>;
  /** 下载 URL */
  downloadUrl: string;
}

/**
 * 推荐插件列表
 */
const recommendedPlugins: RecommendedPlugin[] = [
  {
    id: "machine-id-tool",
    name: "机器码管理工具",
    description: "查看、修改和管理系统机器码，支持跨平台操作",
    icon: Cpu,
    // 插件包从 MachineIdTool 仓库 release 下载
    downloadUrl:
      "https://github.com/aiclientproxy/MachineIdTool/releases/latest/download/machine-id-tool-plugin.zip",
  },
  {
    id: "browser-interception",
    name: "浏览器拦截器",
    description: "拦截桌面应用的浏览器启动，支持手动复制 URL 到指纹浏览器",
    icon: Globe,
    downloadUrl:
      "https://github.com/aiclientproxy/browser-interception/releases/latest/download/browser-interception-plugin.zip",
  },
  {
    id: "flow-monitor",
    name: "Flow Monitor",
    description: "监控和分析 LLM API 请求，提供详细的流量分析和调试功能",
    icon: Activity,
    downloadUrl:
      "https://github.com/aiclientproxy/flow-monitor/releases/latest/download/flow-monitor-plugin.zip",
  },
  {
    id: "config-switch",
    name: "配置管理",
    description: "一键切换 API 配置，支持 Claude Code、Codex、Gemini 等客户端",
    icon: FileCode,
    downloadUrl:
      "https://github.com/aiclientproxy/config-switch/releases/latest/download/config-switch-plugin.zip",
  },
];

export function PluginManager() {
  const [status, setStatus] = useState<PluginServiceStatus | null>(null);
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [installedPlugins, setInstalledPlugins] = useState<InstalledPlugin[]>(
    [],
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedPlugin, setExpandedPlugin] = useState<string | null>(null);

  // 对话框状态
  const [showInstallDialog, setShowInstallDialog] = useState(false);
  const [pluginToUninstall, setPluginToUninstall] =
    useState<InstalledPlugin | null>(null);
  const [pendingInstallUrl, setPendingInstallUrl] = useState<string | null>(
    null,
  );

  const fetchData = useCallback(async () => {
    try {
      setLoading(true);
      const [serviceStatus, pluginList, installedList] = await Promise.all([
        safeInvoke<PluginServiceStatus>("get_plugin_status"),
        safeInvoke<PluginInfo[]>("get_plugins"),
        safeInvoke<InstalledPlugin[]>("list_installed_plugins").catch(() => []),
      ]);
      setStatus(serviceStatus);
      setPlugins(pluginList);
      setInstalledPlugins(installedList);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  // 处理一键安装
  const handleQuickInstall = useCallback((downloadUrl: string) => {
    setPendingInstallUrl(downloadUrl);
    setShowInstallDialog(true);
  }, []);

  // 处理安装成功
  const handleInstallSuccess = useCallback(() => {
    fetchData();
    setPendingInstallUrl(null);
    toast.success("插件安装成功");
    // 触发侧边栏刷新事件
    window.dispatchEvent(new CustomEvent("plugin-changed"));
  }, [fetchData]);

  // 过滤出未安装的推荐插件
  const installedPluginIds = new Set(installedPlugins.map((p) => p.id));
  const uninstalledRecommendedPlugins = recommendedPlugins.filter(
    (plugin) => !installedPluginIds.has(plugin.id),
  );

  const handleTogglePlugin = async (name: string, currentEnabled: boolean) => {
    try {
      if (currentEnabled) {
        await safeInvoke("disable_plugin", { name });
      } else {
        await safeInvoke("enable_plugin", { name });
      }
      await fetchData();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleReloadPlugins = async () => {
    try {
      await safeInvoke("reload_plugins");
      await fetchData();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleUnloadPlugin = async (name: string) => {
    try {
      await safeInvoke("unload_plugin", { name });
      await fetchData();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "enabled":
        return <CheckCircle className="h-4 w-4 text-green-500" />;
      case "disabled":
        return <PowerOff className="h-4 w-4 text-gray-400" />;
      case "error":
        return <AlertCircle className="h-4 w-4 text-red-500" />;
      default:
        return <Clock className="h-4 w-4 text-yellow-500" />;
    }
  };

  const getStatusText = (status: string) => {
    switch (status) {
      case "enabled":
        return "已启用";
      case "disabled":
        return "已禁用";
      case "error":
        return "错误";
      case "loaded":
        return "已加载";
      default:
        return status;
    }
  };

  if (loading) {
    return (
      <div className="rounded-lg border bg-card p-4">
        <div className="flex items-center gap-2 text-muted-foreground">
          <RefreshCw className="h-4 w-4 animate-spin" />
          <span>加载中...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 状态概览 */}
      <div className="rounded-lg border bg-card p-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold flex items-center gap-2">
            <Puzzle className="h-4 w-4" />
            插件系统
          </h3>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={() => setShowInstallDialog(true)}
              className="gap-1"
            >
              <Plus className="h-4 w-4" />
              安装插件
            </Button>
            <button
              onClick={handleReloadPlugins}
              className="p-1 hover:bg-muted rounded"
              title="重新加载插件"
            >
              <RefreshCw className="h-4 w-4" />
            </button>
          </div>
        </div>

        {error && (
          <div className="mb-4 p-2 bg-red-50 text-red-600 rounded text-sm">
            {error}
          </div>
        )}

        {status && (
          <div className="grid grid-cols-3 gap-4 mb-4">
            <div className="text-center">
              <div className="text-2xl font-bold">{status.plugin_count}</div>
              <div className="text-xs text-muted-foreground">已加载插件</div>
            </div>
            <div className="text-center">
              <div className="text-2xl font-bold">
                {installedPlugins.length}
              </div>
              <div className="text-xs text-muted-foreground">已安装插件</div>
            </div>
            <div className="text-center">
              <div
                className="text-sm font-mono truncate"
                title={status.plugins_dir}
              >
                <FolderOpen className="h-4 w-4 inline mr-1" />
                {status.plugins_dir.split("/").pop()}
              </div>
              <div className="text-xs text-muted-foreground">插件目录</div>
            </div>
          </div>
        )}
      </div>

      {/* 推荐插件 */}
      {uninstalledRecommendedPlugins.length > 0 && (
        <div className="rounded-lg border bg-card">
          <div className="p-4 border-b">
            <h4 className="font-semibold flex items-center gap-2">
              <Download className="h-4 w-4" />
              推荐插件
            </h4>
          </div>
          <div className="divide-y">
            {uninstalledRecommendedPlugins.map((plugin) => (
              <div key={plugin.id} className="p-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <div className="p-2 bg-primary/10 rounded-lg">
                      <plugin.icon className="h-5 w-5 text-primary" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-medium">{plugin.name}</span>
                        <Badge variant="outline" className="text-xs">
                          推荐
                        </Badge>
                      </div>
                      <div className="text-sm text-muted-foreground">
                        {plugin.description}
                      </div>
                    </div>
                  </div>
                  <Button
                    size="sm"
                    onClick={() => handleQuickInstall(plugin.downloadUrl)}
                    className="gap-1"
                  >
                    <Download className="h-4 w-4" />
                    一键安装
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 已安装插件列表（通过安装器安装的） */}
      {installedPlugins.length > 0 && (
        <div className="rounded-lg border bg-card">
          <div className="p-4 border-b flex items-center justify-between">
            <h4 className="font-semibold flex items-center gap-2">
              <Package className="h-4 w-4" />
              已安装插件包
            </h4>
          </div>
          <div className="divide-y">
            {installedPlugins.map((plugin) => (
              <PluginItemContextMenu
                key={plugin.id}
                plugin={plugin}
                onToggleEnabled={async () => {
                  try {
                    if (plugin.enabled) {
                      await safeInvoke("disable_plugin", { name: plugin.id });
                      toast.success("插件已禁用");
                    } else {
                      await safeInvoke("enable_plugin", { name: plugin.id });
                      toast.success("插件已启用");
                    }
                    fetchData();
                  } catch (_err) {
                    toast.error("操作失败");
                  }
                }}
                onUninstall={() => setPluginToUninstall(plugin)}
              >
                <div>
                  <InstalledPluginItem
                    plugin={plugin}
                    onUninstall={() => setPluginToUninstall(plugin)}
                  />
                </div>
              </PluginItemContextMenu>
            ))}
          </div>
        </div>
      )}

      {/* 插件列表 */}
      <div className="rounded-lg border bg-card">
        <div className="p-4 border-b">
          <h4 className="font-semibold">已加载插件</h4>
        </div>

        {plugins.length === 0 ? (
          <div className="p-8 text-center text-muted-foreground">
            <Puzzle className="h-12 w-12 mx-auto mb-2 opacity-50" />
            <p>暂无已加载的插件</p>
            <p className="text-sm mt-1">点击"安装插件"按钮添加新插件</p>
          </div>
        ) : (
          <div className="divide-y">
            {plugins.map((plugin) => (
              <PluginItem
                key={plugin.name}
                plugin={plugin}
                expanded={expandedPlugin === plugin.name}
                onToggleExpand={() =>
                  setExpandedPlugin(
                    expandedPlugin === plugin.name ? null : plugin.name,
                  )
                }
                onToggleEnabled={() =>
                  handleTogglePlugin(plugin.name, plugin.config.enabled)
                }
                onUnload={() => handleUnloadPlugin(plugin.name)}
                getStatusIcon={getStatusIcon}
                getStatusText={getStatusText}
              />
            ))}
          </div>
        )}
      </div>

      {/* 安装对话框 */}
      <PluginInstallDialog
        isOpen={showInstallDialog}
        onClose={() => {
          setShowInstallDialog(false);
          setPendingInstallUrl(null);
        }}
        onSuccess={handleInstallSuccess}
        initialUrl={pendingInstallUrl || undefined}
      />

      {/* 卸载确认对话框 */}
      <PluginUninstallDialog
        isOpen={pluginToUninstall !== null}
        plugin={pluginToUninstall}
        onClose={() => setPluginToUninstall(null)}
        onSuccess={fetchData}
      />
    </div>
  );
}

interface PluginItemProps {
  plugin: PluginInfo;
  expanded: boolean;
  onToggleExpand: () => void;
  onToggleEnabled: () => void;
  onUnload: () => void;
  getStatusIcon: (status: string) => React.ReactNode;
  getStatusText: (status: string) => string;
}

function PluginItem({
  plugin,
  expanded,
  onToggleExpand,
  onToggleEnabled,
  onUnload,
  getStatusIcon,
  getStatusText,
}: PluginItemProps) {
  return (
    <div className="p-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <button
            onClick={onToggleExpand}
            className="p-1 hover:bg-muted rounded"
          >
            {expanded ? (
              <ChevronUp className="h-4 w-4" />
            ) : (
              <ChevronDown className="h-4 w-4" />
            )}
          </button>
          <div>
            <div className="flex items-center gap-2">
              <span className="font-medium">{plugin.name}</span>
              {plugin.version && (
                <Badge variant="secondary" className="text-xs">
                  v{plugin.version}
                </Badge>
              )}
              <span className="flex items-center gap-1 text-xs">
                {getStatusIcon(plugin.status)}
                {getStatusText(plugin.status)}
              </span>
            </div>
            <div className="text-sm text-muted-foreground">
              {plugin.description || "无描述"}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={onToggleEnabled}
            className={`p-2 rounded ${
              plugin.config.enabled
                ? "bg-green-100 text-green-600 hover:bg-green-200"
                : "bg-gray-100 text-gray-600 hover:bg-gray-200"
            }`}
            title={plugin.config.enabled ? "禁用插件" : "启用插件"}
          >
            {plugin.config.enabled ? (
              <Power className="h-4 w-4" />
            ) : (
              <PowerOff className="h-4 w-4" />
            )}
          </button>
          <button
            onClick={onUnload}
            className="p-2 rounded bg-red-100 text-red-600 hover:bg-red-200"
            title="卸载插件"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </div>

      {expanded && (
        <div className="mt-4 pl-8 space-y-3">
          {plugin.author && (
            <div className="text-sm">
              <span className="text-muted-foreground">作者：</span>
              {plugin.author}
            </div>
          )}

          <div className="text-sm">
            <span className="text-muted-foreground">路径：</span>
            <span className="font-mono text-xs">{plugin.path}</span>
          </div>

          {plugin.hooks.length > 0 && (
            <div className="text-sm">
              <span className="text-muted-foreground">钩子：</span>
              <div className="flex gap-1 mt-1">
                {plugin.hooks.map((hook) => (
                  <span
                    key={hook}
                    className="px-2 py-0.5 bg-blue-100 text-blue-700 rounded text-xs"
                  >
                    {hook}
                  </span>
                ))}
              </div>
            </div>
          )}

          <div className="text-sm">
            <span className="text-muted-foreground">统计：</span>
            <div className="grid grid-cols-3 gap-2 mt-1">
              <div className="text-center p-2 bg-muted rounded">
                <div className="font-bold">{plugin.state.execution_count}</div>
                <div className="text-xs text-muted-foreground">执行次数</div>
              </div>
              <div className="text-center p-2 bg-muted rounded">
                <div className="font-bold text-red-500">
                  {plugin.state.error_count}
                </div>
                <div className="text-xs text-muted-foreground">错误次数</div>
              </div>
              <div className="text-center p-2 bg-muted rounded">
                <div className="font-bold">{plugin.config.timeout_ms}ms</div>
                <div className="text-xs text-muted-foreground">超时时间</div>
              </div>
            </div>
          </div>

          {plugin.state.last_error && (
            <div className="text-sm p-2 bg-red-50 text-red-600 rounded">
              <span className="font-medium">最后错误：</span>
              {plugin.state.last_error}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default PluginManager;

/** 已安装插件项组件 */
interface InstalledPluginItemProps {
  plugin: InstalledPlugin;
  onUninstall: () => void;
}

function InstalledPluginItem({
  plugin,
  onUninstall,
}: InstalledPluginItemProps) {
  // 获取安装来源显示文本
  const getSourceText = (source: InstallSource): string => {
    switch (source.type) {
      case "local":
        return `本地文件: ${source.path?.split("/").pop() || "未知"}`;
      case "url":
        return `URL: ${source.url?.split("/").pop() || "未知"}`;
      case "github":
        return `GitHub: ${source.owner}/${source.repo}@${source.tag}`;
      default:
        return "未知来源";
    }
  };

  return (
    <div className="p-4">
      <div className="flex items-center justify-between">
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <span className="font-medium">{plugin.name}</span>
            {plugin.version && (
              <Badge variant="secondary" className="text-xs">
                v{plugin.version}
              </Badge>
            )}
            {plugin.enabled ? (
              <span className="flex items-center gap-1 text-xs text-green-600">
                <CheckCircle className="h-3 w-3" />
                已启用
              </span>
            ) : (
              <span className="flex items-center gap-1 text-xs text-gray-400">
                <PowerOff className="h-3 w-3" />
                已禁用
              </span>
            )}
          </div>
          <div className="text-sm text-muted-foreground mt-1">
            {plugin.description || "无描述"}
          </div>
          <div className="text-xs text-muted-foreground mt-1">
            {getSourceText(plugin.source)}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={onUninstall}
            className="p-2 rounded bg-red-100 text-red-600 hover:bg-red-200"
            title="卸载插件"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  );
}
