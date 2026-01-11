/**
 * 插件 UI 渲染器组件
 *
 * 根据 pluginId 渲染对应的插件 UI 组件
 * 支持内置插件组件映射、动态加载外部插件和错误处理
 *
 * _需求: 3.2_
 */

import React, { useState, useEffect } from "react";
import { AlertCircle, Package, Loader2, ExternalLink } from "lucide-react";
import { safeInvoke } from "@/lib/dev-bridge";
import { BrowserInterceptorTool } from "@/components/tools/browser-interceptor/BrowserInterceptorTool";
import { FlowMonitorPage } from "@/pages";
import { ConfigManagementPage } from "@/components/config/ConfigManagementPage";
import { PluginUIRenderer as DynamicPluginRenderer } from "@/lib/plugin-loader/PluginUIRenderer";
import { usePluginSDK } from "@/lib/plugin-sdk";
import { Button } from "@/components/ui/button";

/**
 * 页面类型定义
 * 支持静态页面和动态插件页面
 */
export type Page =
  | "provider-pool"
  | "api-server"
  | "agent"
  | "tools"
  | "plugins"
  | "settings"
  | `plugin:${string}`;

/**
 * PluginUIRenderer 组件属性
 */
interface PluginUIRendererProps {
  /** 插件 ID */
  pluginId: string;
  /** 页面导航回调 */
  onNavigate: (page: Page) => void;
}

/**
 * 插件 UI 加载错误组件
 */
function PluginUIError({
  pluginId,
  error,
}: {
  pluginId: string;
  error: string;
}) {
  return (
    <div className="flex flex-col items-center justify-center h-96 space-y-4">
      <div className="p-4 bg-red-50 dark:bg-red-900/20 rounded-full">
        <AlertCircle className="w-12 h-12 text-red-500" />
      </div>
      <div className="text-center space-y-2">
        <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
          插件 UI 加载失败
        </h2>
        <p className="text-gray-600 dark:text-gray-400">
          无法加载插件 "{pluginId}" 的用户界面
        </p>
        <p className="text-sm text-gray-500 dark:text-gray-500">{error}</p>
      </div>
    </div>
  );
}

/**
 * 插件未找到组件
 */
function PluginNotFound({ pluginId }: { pluginId: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-96 space-y-4">
      <div className="p-4 bg-gray-100 dark:bg-gray-800 rounded-full">
        <Package className="w-12 h-12 text-gray-400" />
      </div>
      <div className="text-center space-y-2">
        <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
          插件未找到
        </h2>
        <p className="text-gray-600 dark:text-gray-400">
          插件 "{pluginId}" 未安装或不存在
        </p>
        <p className="text-sm text-gray-500 dark:text-gray-500">
          请检查插件是否已正确安装
        </p>
      </div>
    </div>
  );
}

/**
 * 加载中组件
 */
function PluginLoading() {
  return (
    <div className="flex flex-col items-center justify-center h-96 space-y-4">
      <Loader2 className="w-12 h-12 text-primary animate-spin" />
      <p className="text-gray-600 dark:text-gray-400">加载插件中...</p>
    </div>
  );
}

/**
 * 插件启动器组件
 * 用于显示没有嵌入式 UI 的插件（如 binary 类型）
 */
function PluginLauncher({
  pluginId,
  manifest,
}: {
  pluginId: string;
  manifest: PluginManifest;
}) {
  const [launching, setLaunching] = useState(false);

  const handleLaunch = async () => {
    setLaunching(true);
    try {
      // 调用后端启动插件
      await safeInvoke("launch_plugin_ui", { pluginId });
    } catch (err) {
      console.error("启动插件失败:", err);
    } finally {
      setLaunching(false);
    }
  };

  return (
    <div className="flex flex-col items-center justify-center h-96 space-y-6">
      <div className="p-6 bg-primary/10 rounded-full">
        <Package className="w-16 h-16 text-primary" />
      </div>
      <div className="text-center space-y-2">
        <h2 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
          {manifest.ui?.title || manifest.name}
        </h2>
        <p className="text-gray-600 dark:text-gray-400 max-w-md">
          {manifest.ui?.description || manifest.description || ""}
        </p>
        <p className="text-sm text-muted-foreground">版本 {manifest.version}</p>
      </div>
      <Button
        size="lg"
        onClick={handleLaunch}
        disabled={launching}
        className="gap-2"
      >
        {launching ? (
          <>
            <Loader2 className="w-5 h-5 animate-spin" />
            启动中...
          </>
        ) : (
          <>
            <ExternalLink className="w-5 h-5" />
            打开 {manifest.ui?.title || manifest.name}
          </>
        )}
      </Button>
    </div>
  );
}

/**
 * 内置插件组件映射
 * 注意: machine-id-tool 已移除，改为从插件包动态加载
 * 注意: terminal-plugin 已移除，终端功能已内置到应用中
 */
const builtinPluginComponents: Record<
  string,
  React.ComponentType<{ onNavigate?: (page: Page) => void }>
> = {
  "browser-interception": BrowserInterceptorTool,
  "flow-monitor": FlowMonitorPage,
  "config-switch": ConfigManagementPage,
};

/**
 * 已安装插件信息
 */
interface InstalledPlugin {
  id: string;
  name: string;
  install_path: string;
  has_ui: boolean;
  ui_entry?: string;
}

/**
 * 插件清单信息（从 plugin.json 读取）
 */
interface PluginManifest {
  name: string;
  version: string;
  description?: string;
  plugin_type?: "script" | "native" | "binary";
  ui?: {
    surfaces?: string[];
    icon?: string;
    title?: string;
    description?: string;
    entry?: string;
  };
}

/**
 * 动态插件渲染器
 * 用于加载外部安装的插件 UI
 */
function DynamicPluginUIRenderer({
  pluginId,
  pluginsDir,
  uiEntry,
}: {
  pluginId: string;
  pluginsDir: string;
  uiEntry?: string;
}) {
  const { sdk } = usePluginSDK(pluginId);

  return (
    <DynamicPluginRenderer
      pluginsDir={pluginsDir}
      pluginId={pluginId}
      uiEntry={uiEntry || "dist/index.js"}
      sdk={sdk}
      className="h-full w-full"
      fallback={<PluginNotFound pluginId={pluginId} />}
    />
  );
}

/**
 * 插件 UI 渲染器
 *
 * 根据 pluginId 渲染对应的插件 UI 组件
 * - 对于内置插件，直接渲染对应的 React 组件
 * - 对于外部安装的插件，动态加载其 UI
 * - 对于未知插件，显示错误提示
 */
export function PluginUIRenderer({
  pluginId,
  onNavigate,
}: PluginUIRendererProps) {
  const [loading, setLoading] = useState(true);
  const [pluginInfo, setPluginInfo] = useState<InstalledPlugin | null>(null);
  const [pluginManifest, setPluginManifest] = useState<PluginManifest | null>(
    null,
  );
  const [pluginsDir, setPluginsDir] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  // 查找内置插件组件
  const BuiltinComponent = builtinPluginComponents[pluginId];

  // 对于非内置插件，检查是否已安装并有 UI
  useEffect(() => {
    // 如果是内置插件，跳过检查
    if (BuiltinComponent) {
      setLoading(false);
      return;
    }

    async function checkPlugin() {
      setLoading(true);
      setError(null);

      try {
        // 获取插件目录
        const dir = await safeInvoke<string>("get_plugins_dir");
        setPluginsDir(dir);

        // 首先尝试读取插件清单
        const manifest = await safeInvoke<PluginManifest | null>(
          "read_plugin_manifest_cmd",
          {
            pluginId,
          },
        );

        if (manifest) {
          setPluginManifest(manifest);

          // 检查数据库中是否已注册
          const installed = await safeInvoke<boolean>("is_plugin_installed", {
            pluginId,
          });

          if (installed) {
            // 从数据库获取插件信息
            const plugins = await safeInvoke<InstalledPlugin[]>(
              "list_installed_plugins",
            );
            const plugin = plugins.find((p) => p.id === pluginId);

            if (plugin) {
              setPluginInfo(plugin);
              setLoading(false);
              return;
            }
          }

          // 插件存在于文件系统中但未在数据库注册，创建临时的插件信息
          setPluginInfo({
            id: pluginId,
            name: manifest.name,
            install_path: `${dir}/${pluginId}`,
            has_ui: !!manifest.ui,
            ui_entry: undefined,
          });
          setLoading(false);
          return;
        }

        // 插件不存在
        setPluginInfo(null);
        setPluginManifest(null);
      } catch (err) {
        console.error("检查插件失败:", err);
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    }

    checkPlugin();
  }, [pluginId, BuiltinComponent]);

  // 加载中
  if (loading) {
    return <PluginLoading />;
  }

  // 如果是内置插件，直接渲染
  if (BuiltinComponent) {
    try {
      return <BuiltinComponent onNavigate={onNavigate} />;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : "未知错误";
      return <PluginUIError pluginId={pluginId} error={errorMessage} />;
    }
  }

  // 错误
  if (error) {
    return <PluginUIError pluginId={pluginId} error={error} />;
  }

  // 插件未安装
  if (!pluginInfo || !pluginManifest) {
    return <PluginNotFound pluginId={pluginId} />;
  }

  // 检查插件是否有嵌入式 UI（ui.entry 配置）
  const hasEmbeddedUI = pluginManifest.ui?.entry;

  // 对于 binary 类型的插件，如果没有嵌入式 UI，显示启动器
  if (pluginManifest.plugin_type === "binary" && !hasEmbeddedUI) {
    return <PluginLauncher pluginId={pluginId} manifest={pluginManifest} />;
  }

  // 动态加载插件 UI
  return (
    <DynamicPluginUIRenderer
      pluginId={pluginId}
      pluginsDir={pluginsDir}
      uiEntry={hasEmbeddedUI || pluginInfo.ui_entry}
    />
  );
}

export default PluginUIRenderer;
