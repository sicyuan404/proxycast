/**
 * @file OAuth Provider 插件 API
 * @description 提供 OAuth Provider 插件管理的前端 API
 * @module lib/api/oauthPlugin
 */

import { safeInvoke } from "@/lib/dev-bridge";

// ============================================================================
// 类型定义
// ============================================================================

/** 插件信息 */
export interface OAuthPluginInfo {
  /** 插件 ID */
  id: string;
  /** 显示名称 */
  displayName: string;
  /** 版本 */
  version: string;
  /** 描述 */
  description?: string;
  /** 作者 */
  author?: string;
  /** 主页 */
  homepage?: string;
  /** 许可证 */
  license?: string;
  /** 目标协议 */
  targetProtocol: string;
  /** 是否启用 */
  enabled: boolean;
  /** 安装路径 */
  installPath: string;
  /** 安装时间 */
  installedAt: string;
  /** 最后使用时间 */
  lastUsedAt?: string;
  /** 凭证数量 */
  credentialCount: number;
  /** 支持的认证类型 */
  authTypes: AuthTypeInfo[];
  /** 支持的模型家族 */
  modelFamilies: ModelFamilyInfo[];
  /** UI 入口文件（相对路径） */
  uiEntry?: string;
}

/** 认证类型信息 */
export interface AuthTypeInfo {
  /** 认证类型 ID */
  id: string;
  /** 显示名称 */
  displayName: string;
  /** 描述 */
  description: string;
  /** 类别 */
  category: "oauth" | "api_key" | "custom";
  /** 图标 */
  icon?: string;
}

/** 模型家族信息 */
export interface ModelFamilyInfo {
  /** 名称 */
  name: string;
  /** 匹配模式 */
  pattern: string;
  /** 服务等级 */
  tier?: "mini" | "pro" | "max";
  /** 描述 */
  description?: string;
}

/** 插件安装来源 */
export type PluginSource =
  | { type: "git_hub"; owner: string; repo: string; version?: string }
  | { type: "local_file"; path: string }
  | { type: "builtin"; id: string };

/** 插件安装结果 */
export interface InstallResult {
  /** 是否成功 */
  success: boolean;
  /** 插件 ID */
  pluginId?: string;
  /** 错误消息 */
  error?: string;
}

/** 插件更新信息 */
export interface PluginUpdate {
  /** 插件 ID */
  pluginId: string;
  /** 当前版本 */
  currentVersion: string;
  /** 最新版本 */
  latestVersion: string;
  /** 更新说明 */
  changelog?: string;
}

// ============================================================================
// API 函数
// ============================================================================

/**
 * 初始化 OAuth 插件系统
 */
export async function initOAuthPluginSystem(): Promise<void> {
  try {
    await safeInvoke("init_oauth_plugin_system");
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to init plugin system:", error);
    throw error;
  }
}

/**
 * 获取所有已安装的 OAuth Provider 插件
 */
export async function listOAuthPlugins(): Promise<OAuthPluginInfo[]> {
  try {
    // 先尝试初始化系统（如果已初始化会直接返回）
    await initOAuthPluginSystem();

    const result = await safeInvoke<OAuthPluginInfo[]>("list_oauth_plugins");
    return result;
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to list plugins:", error);
    throw error;
  }
}

/**
 * 获取单个插件信息
 */
export async function getOAuthPlugin(
  pluginId: string,
): Promise<OAuthPluginInfo | null> {
  try {
    const result = await safeInvoke<{ plugin: OAuthPluginInfo | null }>(
      "get_oauth_plugin",
      { pluginId },
    );
    return result.plugin;
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to get plugin:", error);
    throw error;
  }
}

/**
 * 启用插件
 */
export async function enableOAuthPlugin(pluginId: string): Promise<void> {
  try {
    await safeInvoke("enable_oauth_plugin", { pluginId });
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to enable plugin:", error);
    throw error;
  }
}

/**
 * 禁用插件
 */
export async function disableOAuthPlugin(pluginId: string): Promise<void> {
  try {
    await safeInvoke("disable_oauth_plugin", { pluginId });
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to disable plugin:", error);
    throw error;
  }
}

/**
 * 安装插件
 */
export async function installOAuthPlugin(
  source: PluginSource,
): Promise<InstallResult> {
  try {
    const result = await safeInvoke<InstallResult>("install_oauth_plugin", {
      source,
    });
    return result;
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to install plugin:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * 卸载插件
 */
export async function uninstallOAuthPlugin(pluginId: string): Promise<void> {
  try {
    await safeInvoke("uninstall_oauth_plugin", { pluginId });
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to uninstall plugin:", error);
    throw error;
  }
}

/**
 * 检查插件更新
 */
export async function checkOAuthPluginUpdates(): Promise<PluginUpdate[]> {
  try {
    const result = await safeInvoke<{ updates: PluginUpdate[] }>(
      "check_oauth_plugin_updates",
    );
    return result.updates;
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to check updates:", error);
    throw error;
  }
}

/**
 * 更新插件
 */
export async function updateOAuthPlugin(pluginId: string): Promise<void> {
  try {
    await safeInvoke("update_oauth_plugin", { pluginId });
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to update plugin:", error);
    throw error;
  }
}

/**
 * 重新加载所有插件
 */
export async function reloadOAuthPlugins(): Promise<void> {
  try {
    await safeInvoke("reload_oauth_plugins");
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to reload plugins:", error);
    throw error;
  }
}

/**
 * 获取插件配置
 */
export async function getOAuthPluginConfig(
  pluginId: string,
): Promise<Record<string, unknown>> {
  try {
    const result = await safeInvoke<{ config: Record<string, unknown> }>(
      "get_oauth_plugin_config",
      { pluginId },
    );
    return result.config;
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to get plugin config:", error);
    throw error;
  }
}

/**
 * 更新插件配置
 */
export async function updateOAuthPluginConfig(
  pluginId: string,
  config: Record<string, unknown>,
): Promise<void> {
  try {
    await safeInvoke("update_oauth_plugin_config", { pluginId, config });
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to update plugin config:", error);
    throw error;
  }
}

/**
 * 扫描插件目录
 * 用于发现未注册的插件
 */
export async function scanOAuthPluginDirectory(): Promise<string[]> {
  try {
    const result = await safeInvoke<{ paths: string[] }>(
      "scan_oauth_plugin_directory",
    );
    return result.paths;
  } catch (error) {
    console.error("[OAuthPlugin API] Failed to scan directory:", error);
    throw error;
  }
}
