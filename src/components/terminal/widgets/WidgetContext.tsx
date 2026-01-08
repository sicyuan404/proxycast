/**
 * 小部件上下文管理
 *
 * 提供小部件配置的全局状态管理
 * 支持配置持久化到 localStorage
 *
 * @module widgets/WidgetContext
 */

import { useState, useEffect, useCallback, ReactNode } from "react";
import { WidgetConfig, WidgetType, WidgetContextValue } from "./types";
import { DEFAULT_WIDGETS, STORAGE_KEYS } from "./constants";
import { WidgetContext } from "./context";

export { WidgetContext };

interface WidgetProviderProps {
  children: ReactNode;
}

/**
 * 从 localStorage 加载小部件配置
 *
 * 合并策略：
 * 1. 以 DEFAULT_WIDGETS 为基准，确保新增的 widget 会被包含
 * 2. 保留用户对已有 widget 的自定义配置（如 hidden 状态）
 * 3. 移除 DEFAULT_WIDGETS 中不存在的旧 widget
 */
function loadWidgetConfig(): WidgetConfig[] {
  try {
    const stored = localStorage.getItem(STORAGE_KEYS.WIDGET_CONFIG);
    if (stored) {
      const parsed = JSON.parse(stored) as WidgetConfig[];
      // 以 DEFAULT_WIDGETS 为基准合并，确保新 widget 会被添加
      const merged = DEFAULT_WIDGETS.map((defaultWidget) => {
        const storedWidget = parsed.find(
          (w: WidgetConfig) => w.id === defaultWidget.id,
        );
        // 只保留用户可自定义的属性（如 hidden），其他用默认值
        return storedWidget
          ? { ...defaultWidget, hidden: storedWidget.hidden }
          : defaultWidget;
      });

      // 检查是否有新增的 widget，如果有则更新 localStorage
      const storedIds = new Set(parsed.map((w: WidgetConfig) => w.id));
      const hasNewWidgets = DEFAULT_WIDGETS.some((w) => !storedIds.has(w.id));
      if (hasNewWidgets) {
        // 异步更新 localStorage，不阻塞加载
        setTimeout(() => saveWidgetConfig(merged), 0);
      }

      return merged;
    }
  } catch (e) {
    console.error("加载小部件配置失败:", e);
  }
  return DEFAULT_WIDGETS;
}

/**
 * 保存小部件配置到 localStorage
 */
function saveWidgetConfig(widgets: WidgetConfig[]): void {
  try {
    localStorage.setItem(STORAGE_KEYS.WIDGET_CONFIG, JSON.stringify(widgets));
  } catch (e) {
    console.error("保存小部件配置失败:", e);
  }
}

/**
 * 小部件上下文 Provider
 */
export function WidgetProvider({ children }: WidgetProviderProps) {
  const [widgets, setWidgets] = useState<WidgetConfig[]>(loadWidgetConfig);
  const [activeWidget, setActiveWidget] = useState<WidgetType | null>(null);

  // 配置变化时保存到 localStorage
  useEffect(() => {
    saveWidgetConfig(widgets);
  }, [widgets]);

  const updateWidget = useCallback(
    (id: string, config: Partial<WidgetConfig>) => {
      setWidgets((prev) =>
        prev.map((widget) =>
          widget.id === id ? { ...widget, ...config } : widget,
        ),
      );
    },
    [],
  );

  const toggleWidgetVisibility = useCallback((id: string) => {
    setWidgets((prev) =>
      prev.map((widget) =>
        widget.id === id ? { ...widget, hidden: !widget.hidden } : widget,
      ),
    );
  }, []);

  const value: WidgetContextValue = {
    widgets,
    updateWidget,
    toggleWidgetVisibility,
    activeWidget,
    setActiveWidget,
  };

  return (
    <WidgetContext.Provider value={value}>{children}</WidgetContext.Provider>
  );
}
