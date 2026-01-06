/**
 * @file 插件组件全局暴露
 * @description 将插件组件库暴露到全局变量，供动态加载的插件使用
 */

import React from "react";
import * as PluginComponents from "./index";

// 将组件库和 React 暴露到全局变量
if (typeof window !== "undefined") {
  (window as unknown as Record<string, unknown>).React = React;
  (window as unknown as Record<string, unknown>).ProxyCastPluginComponents =
    PluginComponents;

  // 调试：检查所有导出
  console.log("[PluginComponents] 已暴露到全局变量");
  console.log("[PluginComponents] 导出的键:", Object.keys(PluginComponents));

  // 检查是否有 undefined 的导出
  const undefinedExports = Object.entries(PluginComponents)
    .filter(([, value]) => value === undefined)
    .map(([key]) => key);

  if (undefinedExports.length > 0) {
    console.error("[PluginComponents] 以下导出是 undefined:", undefinedExports);
  }
}

export {};
