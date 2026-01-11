/**
 * 工具箱页面组件
 *
 * 显示所有可用工具，包括内置工具和插件工具
 * 支持从插件系统动态获取工具列表
 * 支持推荐插件一键安装
 *
 * _需求: 1.2, 2.1, 2.2_
 */

import React, { useState, useEffect, useCallback } from "react";
import { Package, Loader2, Download, type LucideIcon } from "lucide-react";
import * as LucideIcons from "lucide-react";
import { safeInvoke } from "@/lib/dev-bridge";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { getPluginsForSurface, type PluginUIInfo } from "@/lib/api/pluginUI";
import { PluginInstallDialog } from "@/components/plugins/PluginInstallDialog";
import { ToolCardContextMenu } from "./ToolCardContextMenu";
import { toast } from "sonner";

/**
 * 页面类型定义
 *
 * 支持静态页面和动态插件页面
 * - 静态页面: 预定义的页面标识符
 * - 动态插件页面: `plugin:${string}` 格式，如 "plugin:machine-id-tool"
 *
 * _需求: 2.2, 3.2_
 */
type Page =
  | "provider-pool"
  | "api-server"
  | "agent"
  | "tools"
  | "settings"
  | "plugins"
  | `plugin:${string}`;

interface ToolsPageProps {
  /**
   * 页面导航回调
   * 支持静态页面和动态插件页面
   */
  onNavigate: (page: Page) => void;
}

/**
 * 动态工具卡片数据结构
 */
interface DynamicToolCard {
  /** 工具 ID */
  id: string;
  /** 工具标题 */
  title: string;
  /** 工具描述 */
  description: string;
  /** 图标名称 (Lucide 图标) */
  icon: string;
  /** 工具来源: builtin (内置) 或 plugin (插件) */
  source: "builtin" | "plugin";
  /** 插件 ID (仅插件工具) */
  pluginId?: string;
  /** 是否禁用 */
  disabled?: boolean;
  /** 状态文本 */
  status?: string;
}

interface ToolCardProps {
  title: string;
  description: string;
  icon: React.ReactNode;
  status?: string;
  disabled?: boolean;
  onClick?: () => void;
  source?: "builtin" | "plugin";
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
  /** 图标名称 */
  icon: string;
  /** 下载 URL */
  downloadUrl: string;
}

/**
 * 根据图标名称获取 Lucide 图标组件
 *
 * @param iconName - 图标名称 (如 "Cpu", "Globe")
 * @returns Lucide 图标组件
 */
function getLucideIcon(iconName: string): LucideIcon {
  // 将图标名称转换为 PascalCase
  const pascalCase = iconName
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");

  // 从 LucideIcons 中获取图标
  const Icon = (LucideIcons as any)[pascalCase] as LucideIcon | undefined;
  return Icon || Package;
}

/**
 * 工具卡片组件
 */
function ToolCard({
  title,
  description,
  icon,
  status,
  disabled = false,
  onClick,
  source,
}: ToolCardProps) {
  return (
    <Card
      className={`cursor-pointer transition-colors hover:bg-muted/50 ${disabled ? "opacity-50 cursor-not-allowed" : ""}`}
    >
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="p-2 bg-primary/10 rounded-lg">{icon}</div>
            <div>
              <CardTitle className="text-lg">{title}</CardTitle>
              <div className="flex items-center gap-2 mt-1">
                {status && (
                  <Badge
                    variant={status === "运行中" ? "default" : "secondary"}
                  >
                    {status}
                  </Badge>
                )}
                {source === "plugin" && (
                  <Badge variant="outline" className="text-xs">
                    插件
                  </Badge>
                )}
              </div>
            </div>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <CardDescription className="text-sm text-muted-foreground mb-4">
          {description}
        </CardDescription>
        <Button
          variant="outline"
          size="sm"
          disabled={disabled}
          onClick={onClick}
          className="w-full"
        >
          {disabled ? "敬请期待" : "打开工具"}
        </Button>
      </CardContent>
    </Card>
  );
}

/**
 * 内置工具列表
 */
const builtinTools: DynamicToolCard[] = [];

/**
 * 占位工具列表 (敬请期待)
 */
const placeholderTools: DynamicToolCard[] = [
  {
    id: "network-monitor",
    title: "网络监控工具",
    description: "监控和分析网络请求，提供详细的流量分析",
    icon: "Activity",
    source: "builtin",
    disabled: true,
  },
  {
    id: "config-sync",
    title: "配置同步工具",
    description: "在多个设备间同步 ProxyCast 配置",
    icon: "Settings",
    source: "builtin",
    disabled: true,
  },
  {
    id: "more-tools",
    title: "更多工具",
    description: "更多实用工具正在开发中...",
    icon: "Plus",
    source: "builtin",
    disabled: true,
  },
];

/**
 * 推荐插件列表
 */
const recommendedPlugins: RecommendedPlugin[] = [
  {
    id: "machine-id-tool",
    name: "机器码管理工具",
    description: "查看、修改和管理系统机器码，支持跨平台操作",
    icon: "Cpu",
    // 插件包从 MachineIdTool 仓库 release 下载
    downloadUrl:
      "https://github.com/aiclientproxy/MachineIdTool/releases/latest/download/machine-id-tool-plugin.zip",
  },
  {
    id: "browser-interception",
    name: "浏览器拦截器",
    description: "拦截桌面应用的浏览器启动，支持手动复制 URL 到指纹浏览器",
    icon: "Globe",
    downloadUrl:
      "https://github.com/aiclientproxy/browser-interception/releases/latest/download/browser-interception-plugin.zip",
  },
  {
    id: "flow-monitor",
    name: "Flow Monitor",
    description: "监控和分析 LLM API 请求，提供详细的流量分析和调试功能",
    icon: "Activity",
    downloadUrl:
      "https://github.com/aiclientproxy/flow-monitor/releases/latest/download/flow-monitor-plugin.zip",
  },
];

export function ToolsPage({ onNavigate }: ToolsPageProps) {
  const [pluginTools, setPluginTools] = useState<DynamicToolCard[]>([]);
  const [loading, setLoading] = useState(true);
  const [showInstallDialog, setShowInstallDialog] = useState(false);
  const [pendingInstallUrl, setPendingInstallUrl] = useState<string | null>(
    null,
  );
  const [installedPluginIds, setInstalledPluginIds] = useState<Set<string>>(
    new Set(),
  );

  // 加载插件工具和已安装插件列表
  const loadPluginTools = useCallback(async () => {
    try {
      const plugins = await getPluginsForSurface("tools");
      const tools: DynamicToolCard[] = plugins.map((plugin: PluginUIInfo) => ({
        id: `plugin:${plugin.pluginId}`,
        title: plugin.name,
        description: plugin.description,
        icon: plugin.icon || "Package",
        source: "plugin" as const,
        pluginId: plugin.pluginId,
      }));
      setPluginTools(tools);

      // 更新已安装插件 ID 集合
      const installedIds = new Set(
        plugins.map((p: PluginUIInfo) => p.pluginId),
      );
      setInstalledPluginIds(installedIds);
    } catch (error) {
      console.error("加载插件工具失败:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  // 从插件系统获取工具列表
  useEffect(() => {
    loadPluginTools();
  }, [loadPluginTools]);

  // 处理安装成功
  const handleInstallSuccess = useCallback(() => {
    loadPluginTools();
    setPendingInstallUrl(null);
  }, [loadPluginTools]);

  // 处理一键安装
  const handleQuickInstall = useCallback((downloadUrl: string) => {
    setPendingInstallUrl(downloadUrl);
    setShowInstallDialog(true);
  }, []);

  // 处理插件启用/禁用
  const handleTogglePluginEnabled = useCallback(
    async (pluginId: string, enabled: boolean) => {
      try {
        if (enabled) {
          await safeInvoke("enable_plugin", { name: pluginId });
          toast.success("插件已启用");
        } else {
          await safeInvoke("disable_plugin", { name: pluginId });
          toast.success("插件已禁用");
        }
        loadPluginTools();
      } catch (error) {
        console.error("切换插件状态失败:", error);
        toast.error("操作失败");
      }
    },
    [loadPluginTools],
  );

  // 处理插件卸载
  const handleUninstallPlugin = useCallback(
    async (pluginId: string) => {
      try {
        await safeInvoke("uninstall_plugin", { pluginId });
        toast.success("插件已卸载");
        loadPluginTools();
      } catch (error) {
        console.error("卸载插件失败:", error);
        toast.error("卸载失败");
      }
    },
    [loadPluginTools],
  );

  // 合并内置工具和插件工具
  const allTools = [...builtinTools, ...pluginTools, ...placeholderTools];
  const activeToolsCount = builtinTools.length + pluginTools.length;

  // 过滤出未安装的推荐插件
  const uninstalledRecommendedPlugins = recommendedPlugins.filter(
    (plugin) => !installedPluginIds.has(plugin.id),
  );

  /**
   * 处理工具卡片点击
   */
  const handleToolClick = (tool: DynamicToolCard) => {
    if (tool.disabled) return;

    if (tool.source === "plugin" && tool.pluginId) {
      // 插件工具: 导航到 plugin:xxx 页面
      onNavigate(`plugin:${tool.pluginId}`);
    } else {
      // 内置工具: 导航到对应页面
      onNavigate(tool.id as Page);
    }
  };

  /**
   * 渲染工具图标
   */
  const renderIcon = (iconName: string, disabled?: boolean) => {
    const Icon = getLucideIcon(iconName);
    return (
      <Icon
        className={`w-6 h-6 ${disabled ? "text-muted-foreground" : "text-primary"}`}
      />
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold">工具箱</h1>
          <p className="text-muted-foreground mt-1">
            ProxyCast 提供的实用工具集合
          </p>
        </div>
        <div className="flex items-center gap-2">
          {loading && <Loader2 className="w-4 h-4 animate-spin" />}
          <Badge variant="outline">{activeToolsCount} 个工具</Badge>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {allTools.map((tool) => (
          <ToolCardContextMenu
            key={tool.id}
            tool={tool}
            onNavigate={onNavigate}
            onToggleEnabled={handleTogglePluginEnabled}
            onUninstall={handleUninstallPlugin}
            isEnabled={true}
          >
            <div>
              <ToolCard
                title={tool.title}
                description={tool.description}
                icon={renderIcon(tool.icon, tool.disabled)}
                status={tool.status}
                disabled={tool.disabled}
                source={tool.source}
                onClick={() => handleToolClick(tool)}
              />
            </div>
          </ToolCardContextMenu>
        ))}
      </div>

      <div className="mt-8 p-6 bg-muted/30 rounded-lg">
        <h3 className="text-lg font-semibold mb-2">关于工具箱</h3>
        <p className="text-sm text-muted-foreground">
          工具箱是 ProxyCast
          的扩展功能模块，提供各种实用工具来增强您的使用体验。
          每个工具都经过精心设计，旨在解决特定的使用场景和需求。
          {pluginTools.length > 0 && (
            <span className="block mt-2">
              当前已安装 {pluginTools.length} 个插件工具。
            </span>
          )}
        </p>
      </div>

      {/* 推荐插件区域 */}
      {uninstalledRecommendedPlugins.length > 0 && (
        <div className="mt-8">
          <h3 className="text-lg font-semibold mb-4">推荐插件</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            {uninstalledRecommendedPlugins.map((plugin) => {
              const Icon = getLucideIcon(plugin.icon);
              return (
                <Card
                  key={plugin.id}
                  className="border-dashed border-2 border-primary/30 bg-primary/5"
                >
                  <CardHeader className="pb-3">
                    <div className="flex items-center space-x-3">
                      <div className="p-2 bg-primary/10 rounded-lg">
                        <Icon className="w-6 h-6 text-primary" />
                      </div>
                      <div>
                        <CardTitle className="text-lg">{plugin.name}</CardTitle>
                        <Badge variant="outline" className="text-xs mt-1">
                          推荐安装
                        </Badge>
                      </div>
                    </div>
                  </CardHeader>
                  <CardContent>
                    <CardDescription className="text-sm text-muted-foreground mb-4">
                      {plugin.description}
                    </CardDescription>
                    <Button
                      variant="default"
                      size="sm"
                      className="w-full"
                      onClick={() => handleQuickInstall(plugin.downloadUrl)}
                    >
                      <Download className="w-4 h-4 mr-2" />
                      一键安装
                    </Button>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </div>
      )}

      {/* 插件安装对话框 */}
      <PluginInstallDialog
        isOpen={showInstallDialog}
        onClose={() => {
          setShowInstallDialog(false);
          setPendingInstallUrl(null);
        }}
        onSuccess={handleInstallSuccess}
        initialUrl={pendingInstallUrl || undefined}
      />
    </div>
  );
}
