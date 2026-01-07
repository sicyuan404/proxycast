/**
 * @file ComponentDebugContext.tsx
 * @description 组件视图调试上下文 - 提供全局组件轮廓显示和信息查看功能
 */
import { createContext, useContext, useState, useCallback, ReactNode } from "react";

export interface ComponentInfo {
  name: string;
  filePath: string;
  props: Record<string, unknown>;
  depth: number;
  tagName: string;
  x: number;
  y: number;
  /** 当前 DOM 元素引用，用于查找父组件 */
  element?: HTMLElement;
  /** 当前 Fiber 节点，用于向上遍历 */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  fiber?: any;
}

interface ComponentDebugContextType {
  /** 是否启用组件视图调试 */
  enabled: boolean;
  /** 切换调试模式 */
  setEnabled: (enabled: boolean) => void;
  /** 当前显示的组件信息弹窗 */
  componentInfo: ComponentInfo | null;
  /** 显示组件信息弹窗 */
  showComponentInfo: (info: ComponentInfo) => void;
  /** 隐藏组件信息弹窗 */
  hideComponentInfo: () => void;
}

const ComponentDebugContext = createContext<ComponentDebugContextType | null>(null);

const STORAGE_KEY = "component-debug-enabled";

export function ComponentDebugProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabledState] = useState(() => {
    const stored = localStorage.getItem(STORAGE_KEY);
    return stored === "true";
  });
  const [componentInfo, setComponentInfo] = useState<ComponentInfo | null>(null);

  const setEnabled = useCallback((value: boolean) => {
    setEnabledState(value);
    localStorage.setItem(STORAGE_KEY, String(value));
  }, []);

  const showComponentInfo = useCallback((info: ComponentInfo) => {
    setComponentInfo(info);
  }, []);

  const hideComponentInfo = useCallback(() => {
    setComponentInfo(null);
  }, []);

  return (
    <ComponentDebugContext.Provider
      value={{
        enabled,
        setEnabled,
        componentInfo,
        showComponentInfo,
        hideComponentInfo,
      }}
    >
      {children}
    </ComponentDebugContext.Provider>
  );
}

export function useComponentDebug() {
  const context = useContext(ComponentDebugContext);
  if (!context) {
    throw new Error("useComponentDebug must be used within ComponentDebugProvider");
  }
  return context;
}
