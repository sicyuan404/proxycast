/**
 * @file 插件 UI 加载器
 * @description 动态加载插件的 React 组件
 * @module lib/plugin-loader
 */

import React from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ProxyCastPluginSDK as PluginSDK } from "@/lib/plugin-sdk/types";

/**
 * 插件组件 Props
 */
export interface PluginComponentProps {
  /** 插件 SDK */
  sdk: PluginSDK;
  /** 插件 ID */
  pluginId: string;
}

/**
 * 插件模块接口
 */
export interface PluginModule {
  /** 默认导出的组件 */
  default: React.ComponentType<PluginComponentProps>;
}

/**
 * 已加载的插件缓存
 */
const loadedPlugins = new Map<string, PluginModule>();

/**
 * 插件 ID 到全局变量名的映射
 * 格式: pluginId -> GlobalVariableName
 */
const PLUGIN_GLOBAL_NAMES: Record<string, string> = {
  "kiro-provider": "KiroProviderPlugin",
  "droid-provider": "DroidProviderPlugin",
  "claude-provider": "ClaudeProviderPlugin",
  "gemini-provider": "GeminiProviderPlugin",
  "antigravity-provider": "AntigravityProviderPlugin",
  "codex-provider": "CodexProviderPlugin",
};

/**
 * 根据插件 ID 获取全局变量名
 * 如果没有预定义，则尝试从路径推断
 */
function getPluginGlobalName(pluginPath: string): string {
  // 从路径中提取插件 ID
  const parts = pluginPath.split("/");
  const pluginIdIndex = parts.findIndex((p) => p.endsWith("-provider"));
  const pluginId = pluginIdIndex >= 0 ? parts[pluginIdIndex] : null;

  // 查找预定义的全局变量名
  if (pluginId && PLUGIN_GLOBAL_NAMES[pluginId]) {
    return PLUGIN_GLOBAL_NAMES[pluginId];
  }

  // 尝试从插件 ID 推断全局变量名
  // 例如: my-plugin -> MyPluginPlugin
  if (pluginId) {
    const camelCase = pluginId
      .split("-")
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join("");
    return camelCase;
  }

  // 默认回退
  return "KiroProviderPlugin";
}

/**
 * 读取插件文件内容
 */
async function readPluginFile(filePath: string): Promise<string> {
  try {
    const content = await invoke<string>("read_plugin_ui_file", {
      path: filePath,
    });
    return content;
  } catch (error) {
    console.error(`[PluginLoader] 读取插件文件失败: ${filePath}`, error);
    throw error;
  }
}

/**
 * 通过 script 标签执行代码
 */
function executeScript(code: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.type = "text/javascript";
    script.text = code;

    script.onerror = (error) => {
      document.head.removeChild(script);
      reject(error);
    };

    // 同步执行，完成后立即 resolve
    try {
      document.head.appendChild(script);
      document.head.removeChild(script);
      resolve();
    } catch (error) {
      reject(error);
    }
  });
}

/**
 * 加载插件 UI 组件
 *
 * 插件使用 IIFE 格式构建，从全局变量获取依赖（React, ProxyCastPluginComponents）
 * 并将组件导出到全局变量
 *
 * @param pluginPath - 插件 JS 文件路径
 * @returns 插件模块
 */
export async function loadPluginUI(
  pluginPath: string,
): Promise<PluginModule | null> {
  // 检查缓存
  if (loadedPlugins.has(pluginPath)) {
    return loadedPlugins.get(pluginPath)!;
  }

  try {
    // 读取插件文件内容
    const content = await readPluginFile(pluginPath);

    // 获取插件的全局变量名
    const globalName = getPluginGlobalName(pluginPath);

    console.log(`[PluginLoader] 加载插件: ${pluginPath}`);
    console.log(`[PluginLoader] 全局变量名: ${globalName}`);
    console.log(
      `[PluginLoader] 全局变量检查: React=${typeof (window as unknown as Record<string, unknown>).React}, ProxyCastPluginComponents=${typeof (window as unknown as Record<string, unknown>).ProxyCastPluginComponents}`,
    );

    // 检查 ProxyCastPluginComponents 中的所有导出
    const components = (window as unknown as Record<string, unknown>)
      .ProxyCastPluginComponents as Record<string, unknown> | undefined;
    if (components) {
      const undefinedKeys = Object.keys(components).filter(
        (key) => components[key] === undefined,
      );
      if (undefinedKeys.length > 0) {
        console.error(
          `[PluginLoader] ProxyCastPluginComponents 中有 undefined 的导出:`,
          undefinedKeys,
        );
      }
    }

    // 执行插件代码
    await executeScript(content);

    // 获取插件模块
    const pluginExports = (window as unknown as Record<string, unknown>)[
      globalName
    ] as Record<string, unknown> | undefined;

    if (!pluginExports) {
      console.error(
        `[PluginLoader] 插件 ${pluginPath} 没有导出到 window.${globalName}`,
      );
      return null;
    }

    console.log(`[PluginLoader] 插件导出:`, Object.keys(pluginExports));

    // 获取默认导出
    const defaultExport = pluginExports.default as
      | React.ComponentType<PluginComponentProps>
      | undefined;

    if (!defaultExport) {
      console.error(`[PluginLoader] 插件 ${pluginPath} 没有默认导出`);
      return null;
    }

    const module: PluginModule = {
      default: defaultExport,
    };

    // 缓存
    loadedPlugins.set(pluginPath, module);
    return module;
  } catch (error) {
    console.error(`[PluginLoader] 加载插件失败: ${pluginPath}`, error);
    return null;
  }
}

/**
 * 清除插件缓存
 */
export function clearPluginCache(pluginPath?: string): void {
  if (pluginPath) {
    loadedPlugins.delete(pluginPath);
  } else {
    loadedPlugins.clear();
  }
}

/**
 * 获取插件 UI 文件路径
 *
 * @param pluginsDir - 插件目录
 * @param pluginId - 插件 ID
 * @param uiEntry - UI 入口文件（相对路径）
 * @returns 完整路径
 */
export function getPluginUIPath(
  pluginsDir: string,
  pluginId: string,
  uiEntry: string = "dist/index.js",
): string {
  // 返回文件系统路径（不是 URL）
  return `${pluginsDir}/${pluginId}/${uiEntry}`;
}
