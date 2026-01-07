/**
 * @file ComponentDebugOverlay.tsx
 * @description 组件视图调试覆盖层 - Alt+悬浮显示轮廓，Alt+点击显示组件信息
 */
import React, { useEffect, useState, useMemo } from "react";
import { useComponentDebug, ComponentInfo } from "@/contexts/ComponentDebugContext";
import { X, Copy, Check, Component, FileCode, Layers, Hash, ChevronUp } from "lucide-react";

// ============================================================================
// 错误边界组件
// ============================================================================

interface ErrorBoundaryState {
  hasError: boolean;
  errorCount: number;
}

class DebugErrorBoundary extends React.Component<
  { children: React.ReactNode },
  ErrorBoundaryState
> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = { hasError: false, errorCount: 0 };
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    console.warn("[ComponentDebugOverlay] 捕获到渲染错误:", error.message);
    return { hasError: true };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.warn("[ComponentDebugOverlay] 错误详情:", error, errorInfo);
  }

  componentDidUpdate(prevProps: { children: React.ReactNode }) {
    // 当 children 变化时，尝试恢复
    if (this.state.hasError && prevProps.children !== this.props.children) {
      this.setState({ hasError: false });
    }
  }

  render() {
    if (this.state.hasError) {
      // 使用 setTimeout 在下一帧尝试恢复
      setTimeout(() => {
        if (this.state.hasError) {
          this.setState((prev) => ({ 
            hasError: false, 
            errorCount: prev.errorCount + 1 
          }));
        }
      }, 100);
      
      // 如果错误次数过多，完全禁用
      if (this.state.errorCount > 5) {
        return null;
      }
      return null;
    }
    return this.props.children;
  }
}

// ============================================================================
// 类型定义
// ============================================================================

/** React Fiber 调试源信息 */
interface DebugSource {
  fileName?: string;
  lineNumber?: number;
  columnNumber?: number;
}

/** Fiber 节点类型（支持 memo/forwardRef 包装） */
interface FiberType {
  displayName?: string;
  name?: string;
  $typeof?: symbol;
  type?: FiberType; // for memo wrapped components
  render?: FiberType; // for forwardRef wrapped components
}

/** React Fiber 节点结构 */
interface FiberNode {
  type: FiberType | ((...args: unknown[]) => unknown) | null;
  return: FiberNode | null;
  memoizedProps: Record<string, unknown> & { __source?: DebugSource } | null;
  _debugSource?: DebugSource;
  _debugOwner?: FiberNode;
  stateNode?: HTMLElement | null; // DOM 元素引用
}

// ============================================================================
// Fiber 工具函数
// ============================================================================

/**
 * 判断组件名称是否为 styled-components 生成的名称
 * styled-components 的名称格式通常是 "styled.xxx" 或 "Styled(xxx)"
 */
function isStyledComponentName(name: string): boolean {
  if (!name) return false;
  return (
    name.startsWith("styled.") ||
    name.startsWith("Styled(") ||
    name === "styled" ||
    /^styled[A-Z]/.test(name) // styledButton, styledDiv 等
  );
}

/**
 * 从 Fiber 类型中提取组件名称
 * 支持 memo、forwardRef、styled-components 等包装组件
 */
function getComponentName(type: FiberType | ((...args: unknown[]) => unknown) | null): string {
  if (!type) return "Unknown";

  // 直接函数组件
  if (typeof type === "function") {
    const funcType = type as { displayName?: string; name?: string; styledComponentId?: string; target?: unknown };
    
    // styled-components 有 styledComponentId 属性，尝试获取更好的名称
    if (funcType.styledComponentId) {
      // 如果有 displayName 且不是 styled.xxx 格式，使用它
      if (funcType.displayName && !isStyledComponentName(funcType.displayName)) {
        return funcType.displayName;
      }
      // 否则返回特殊标记，让调用者知道这是 styled-component
      return `[styled]${funcType.displayName || funcType.name || "Component"}`;
    }
    
    return funcType.displayName || funcType.name || "Anonymous";
  }

  // 对象类型（memo、forwardRef 等）
  if (typeof type === "object") {
    const fiberType = type as FiberType & { styledComponentId?: string; target?: unknown };
    
    // styled-components 检测
    if (fiberType.styledComponentId) {
      if (fiberType.displayName && !isStyledComponentName(fiberType.displayName)) {
        return fiberType.displayName;
      }
      return `[styled]${fiberType.displayName || fiberType.name || "Component"}`;
    }
    
    // 直接有 displayName 或 name
    if (fiberType.displayName) return fiberType.displayName;
    if (fiberType.name) return fiberType.name;

    // memo 包装：type.type 是内部组件
    if (fiberType.type) {
      return getComponentName(fiberType.type);
    }

    // forwardRef 包装：type.render 是内部组件
    if (fiberType.render) {
      return getComponentName(fiberType.render);
    }
  }

  return "Unknown";
}

/**
 * 判断 Fiber 节点是否为有效的用户组件
 * 过滤掉内部组件、匿名组件、React 内置组件和 styled-components
 */
function isValidUserComponent(fiber: FiberNode): boolean {
  const type = fiber.type;
  if (!type) return false;

  // 必须是函数组件或对象类型（memo/forwardRef）
  if (typeof type !== "function" && typeof type !== "object") return false;

  const name = getComponentName(type);
  
  // 过滤掉匿名组件
  if (name === "Anonymous" || name === "Unknown") return false;
  
  // 过滤掉 styled-components（以 [styled] 开头的是我们标记的）
  if (name.startsWith("[styled]") || isStyledComponentName(name)) return false;
  // 过滤掉以下划线开头的内部组件
  if (name.startsWith("_")) return false;
  
  // 过滤掉 React 内置组件（如 Fragment、Suspense 等）
  const reactInternals = ["Fragment", "Suspense", "StrictMode", "Profiler"];
  if (reactInternals.includes(name)) return false;

  return true;
}

// ============================================================================
// 工具函数
// ============================================================================

/**
 * 节流函数 - 限制函数在指定时间间隔内最多执行一次
 * @param fn 要节流的函数
 * @param delay 节流间隔（毫秒）
 * @returns 节流后的函数
 */
export function throttle<T extends (...args: Parameters<T>) => void>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let lastCall = 0;
  return (...args: Parameters<T>) => {
    const now = Date.now();
    if (now - lastCall >= delay) {
      lastCall = now;
      fn(...args);
    }
  };
}

// ============================================================================
// 配置常量
// ============================================================================

const DEBUG_CONFIG = {
  // 弹窗位置偏移
  POPUP_WIDTH_OFFSET: 470,
  POPUP_HEIGHT_OFFSET: 350,
  // Props 显示限制
  MAX_PROPS_DISPLAY: 8,
  // 高亮颜色
  HOVER_HIGHLIGHT_COLOR: "rgba(59, 130, 246, 0.8)",      // 蓝色
  HOVER_HIGHLIGHT_BG: "rgba(59, 130, 246, 0.1)",
  SELECTED_HIGHLIGHT_COLOR: "rgba(34, 197, 94, 0.9)",    // 绿色
  SELECTED_HIGHLIGHT_BG: "rgba(34, 197, 94, 0.15)",
  // 节流间隔
  MOUSEMOVE_THROTTLE_MS: 16,
} as const;

/**
 * 从 Fiber 节点提取组件信息
 */
function extractFiberInfo(fiber: FiberNode, element: HTMLElement, x: number, y: number): ComponentInfo | null {
  try {
    if (!fiber || !element) return null;

    const name = getComponentName(fiber.type);

  // 尝试多种方式获取文件路径
  let filePath = "";

  if (fiber._debugSource) {
    const source = fiber._debugSource;
    filePath = source.fileName || "";
    if (source.lineNumber) {
      filePath += `:${source.lineNumber}`;
      if (source.columnNumber) {
        filePath += `:${source.columnNumber}`;
      }
    }
  } else if (fiber.memoizedProps?.__source) {
    const source = fiber.memoizedProps.__source;
    filePath = source.fileName || "";
    if (source.lineNumber) {
      filePath += `:${source.lineNumber}`;
    }
  } else if (fiber._debugOwner?._debugSource) {
    const source = fiber._debugOwner._debugSource;
    filePath = source.fileName || "";
    if (source.lineNumber) {
      filePath += `:${source.lineNumber}`;
    }
  }

  // 简化路径显示
  if (filePath) {
    const srcIndex = filePath.indexOf("/src/");
    if (srcIndex !== -1) {
      filePath = filePath.substring(srcIndex + 1);
    }
    const srcIndexWin = filePath.indexOf("\\src\\");
    if (srcIndexWin !== -1) {
      filePath = filePath.substring(srcIndexWin + 1).replace(/\\/g, "/");
    }
  }

  if (!filePath) {
    filePath = "生产构建中不可用";
  }

  // 获取 props
  const props = fiber.memoizedProps || {};
  const safeProps: Record<string, unknown> = {};
  for (const key of Object.keys(props)) {
    if (key.startsWith("_") || key === "__source" || key === "__self") continue;
    const value = props[key];
    if (typeof value === "function") {
      safeProps[key] = "[Function]";
    } else if (typeof value === "object" && value !== null) {
      if (Array.isArray(value)) {
        safeProps[key] = `[Array(${value.length})]`;
      } else if ((value as Record<string, unknown>).$$typeof) {
        safeProps[key] = "[ReactElement]";
      } else {
        safeProps[key] = "[Object]";
      }
    } else {
      safeProps[key] = value;
    }
  }

  // 计算深度
  let depth = 0;
  let tempFiber = fiber;
  while (tempFiber.return) {
    tempFiber = tempFiber.return;
    depth++;
  }

  return {
    name,
    filePath,
    props: safeProps,
    depth,
    tagName: element.tagName,
    x,
    y,
    element,
    fiber,
  };
  } catch {
    // 发生错误时返回 null，避免白屏
    return null;
  }
}

/**
 * 从 React Fiber 节点获取组件信息
 */
function getReactFiberInfo(element: HTMLElement, x: number, y: number): ComponentInfo | null {
  try {
    const fiberKey = Object.keys(element).find(
      (key) => key.startsWith("__reactFiber$") || key.startsWith("__reactInternalInstance$")
    );

    if (!fiberKey) return null;

    let fiber: FiberNode | null = (element as unknown as Record<string, FiberNode>)[fiberKey];
    if (!fiber) return null;

    // 遍历 Fiber 树找到最近的用户组件
    while (fiber) {
      if (isValidUserComponent(fiber)) {
        return extractFiberInfo(fiber, element, x, y);
      }
      fiber = fiber.return;
    }

    return null;
  } catch {
    // 发生错误时返回 null，避免白屏
    return null;
  }
}

/**
 * 从 Fiber 节点向下查找最近的 DOM 元素
 */
function findDomElement(fiber: FiberNode | null): HTMLElement | null {
  if (!fiber) return null;
  
  // 如果当前节点有 stateNode 且是 DOM 元素
  if (fiber.stateNode instanceof HTMLElement) {
    return fiber.stateNode;
  }
  
  return null;
}

/**
 * 获取父组件信息
 */
function getParentComponentInfo(currentFiber: FiberNode | unknown, x: number, y: number, fallbackElement: HTMLElement): ComponentInfo | null {
  try {
    if (!currentFiber) return null;

    let fiber = (currentFiber as FiberNode).return;

    while (fiber) {
      if (isValidUserComponent(fiber)) {
        // 尝试找到父组件对应的 DOM 元素
        const parentElement = findDomElement(fiber) || fallbackElement;
        return extractFiberInfo(fiber, parentElement, x, y);
      }
      fiber = fiber.return;
    }

    return null;
  } catch {
    // 发生错误时返回 null，避免白屏
    return null;
  }
}

/** 选中组件的持久高亮边框 */
function SelectedHighlight({ element }: { element: HTMLElement | undefined }) {
  const [rect, setRect] = useState<DOMRect | null>(null);

  useEffect(() => {
    if (!element) {
      setRect(null);
      return;
    }

    const updateRect = () => {
      try {
        if (document.contains(element)) {
          setRect(element.getBoundingClientRect());
        } else {
          setRect(null);
        }
      } catch {
        setRect(null);
      }
    };

    updateRect();

    // 监听滚动和窗口调整
    window.addEventListener("scroll", updateRect, true);
    window.addEventListener("resize", updateRect);

    // 监听 DOM 变化（元素可能被移除）
    const observer = new MutationObserver(() => {
      updateRect();
    });
    
    try {
      observer.observe(document.body, { childList: true, subtree: true });
    } catch {
      // 忽略 observer 错误
    }

    return () => {
      window.removeEventListener("scroll", updateRect, true);
      window.removeEventListener("resize", updateRect);
      observer.disconnect();
    };
  }, [element]);

  if (!rect) return null;

  return (
    <div
      className="fixed pointer-events-none z-[99997]"
      style={{
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        outline: `2px solid ${DEBUG_CONFIG.SELECTED_HIGHLIGHT_COLOR}`,
        outlineOffset: "-2px",
        backgroundColor: DEBUG_CONFIG.SELECTED_HIGHLIGHT_BG,
      }}
    />
  );
}

/** 组件信息弹窗 */
function ComponentInfoPopup() {
  const { componentInfo, hideComponentInfo, showComponentInfo } = useComponentDebug();
  const [copiedField, setCopiedField] = useState<string | null>(null);

  // 使用 useMemo 缓存 propsEntries 计算结果 - 必须在所有条件返回之前调用
  const propsEntries = useMemo(() => {
    if (!componentInfo?.props) return [];
    return Object.entries(componentInfo.props).filter(
      ([key]) => key !== "children"
    );
  }, [componentInfo?.props]);

  // 早期返回必须在所有 hooks 之后
  if (!componentInfo) return null;

  // 渲染选中高亮边框
  const selectedHighlight = <SelectedHighlight element={componentInfo.element} />;

  const handleCopy = async (text: string, field: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedField(field);
    setTimeout(() => setCopiedField(null), 2000);
  };

  const handleSelectParent = () => {
    if (!componentInfo.fiber) return;

    const parentInfo = getParentComponentInfo(
      componentInfo.fiber,
      componentInfo.x,
      componentInfo.y,
      componentInfo.element || document.body
    );

    if (parentInfo) {
      showComponentInfo(parentInfo);
    }
  };

  // 检查是否有父组件
  const hasParent = componentInfo.fiber?.return != null;

  return (
    <>
    <div
      className="fixed z-[99999] rounded-lg shadow-xl min-w-[320px] max-w-[450px] border border-gray-200 bg-white text-gray-900"
      style={{
        left: Math.min(componentInfo.x, window.innerWidth - DEBUG_CONFIG.POPUP_WIDTH_OFFSET),
        top: Math.min(componentInfo.y, window.innerHeight - DEBUG_CONFIG.POPUP_HEIGHT_OFFSET),
      }}
      onClick={(e) => e.stopPropagation()}
    >
      {/* 标题栏 */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-gray-200 rounded-t-lg bg-gray-50">
        <div className="flex items-center gap-2">
          <Component className="w-4 h-4 text-blue-500" />
          <span className="font-semibold text-sm">组件信息</span>
        </div>
        <button onClick={hideComponentInfo} className="p-1 hover:bg-gray-200 rounded transition-colors">
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* 内容区域 */}
      <div className="p-3 space-y-3">
        {/* 组件名称 */}
        <div className="flex items-start gap-2">
          <Component className="w-4 h-4 text-blue-500 mt-0.5 shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-xs text-gray-500 mb-0.5">组件名称</div>
            <div className="flex items-center gap-2">
              <code className="font-mono text-sm text-blue-600 font-medium">{componentInfo.name}</code>
              <button onClick={() => handleCopy(componentInfo.name, "name")} className="p-0.5 hover:bg-gray-100 rounded shrink-0" title="复制名称">
                {copiedField === "name" ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3 text-gray-400" />}
              </button>
            </div>
          </div>
        </div>

        {/* 文件路径 */}
        <div className="flex items-start gap-2">
          <FileCode className="w-4 h-4 text-orange-500 mt-0.5 shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-xs text-gray-500 mb-0.5">文件路径</div>
            <div className="flex items-center gap-2">
              <code className="text-xs bg-gray-100 px-2 py-1 rounded truncate flex-1 block text-gray-700">{componentInfo.filePath}</code>
              <button onClick={() => handleCopy(componentInfo.filePath, "path")} className="p-0.5 hover:bg-gray-100 rounded shrink-0" title="复制路径">
                {copiedField === "path" ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3 text-gray-400" />}
              </button>
            </div>
          </div>
        </div>

        {/* HTML 标签 */}
        <div className="flex items-start gap-2">
          <Hash className="w-4 h-4 text-purple-500 mt-0.5 shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-xs text-gray-500 mb-0.5">DOM 元素</div>
            <code className="text-xs text-gray-600">&lt;{componentInfo.tagName.toLowerCase()}&gt;</code>
          </div>
        </div>

        {/* 组件层级 */}
        <div className="flex items-start gap-2">
          <Layers className="w-4 h-4 text-green-500 mt-0.5 shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-xs text-gray-500 mb-0.5">组件层级</div>
            <span className="text-xs text-gray-700">第 {componentInfo.depth} 层</span>
          </div>
        </div>

        {/* Props */}
        {propsEntries.length > 0 && (
          <div className="border-t border-gray-200 pt-3">
            <div className="text-xs text-gray-500 mb-2">Props</div>
            <div className="bg-gray-50 rounded p-2 max-h-[120px] overflow-auto">
              <div className="space-y-1">
                {propsEntries.slice(0, DEBUG_CONFIG.MAX_PROPS_DISPLAY).map(([key, value]) => (
                  <div key={key} className="flex items-start gap-2 text-xs">
                    <span className="text-blue-500 font-mono shrink-0">{key}:</span>
                    <span className="text-gray-600 font-mono truncate">
                      {typeof value === "string" ? `"${value}"` : String(value)}
                    </span>
                  </div>
                ))}
                {propsEntries.length > DEBUG_CONFIG.MAX_PROPS_DISPLAY && (
                  <div className="text-xs text-gray-400">... 还有 {propsEntries.length - DEBUG_CONFIG.MAX_PROPS_DISPLAY} 个属性</div>
                )}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* 底部操作栏 */}
      <div className="px-3 py-2 border-t border-gray-200 rounded-b-lg bg-gray-50 flex items-center justify-between">
        <p className="text-[10px] text-gray-400">按 Esc 关闭</p>
        {hasParent && (
          <button
            onClick={handleSelectParent}
            className="flex items-center gap-1 px-2 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
          >
            <ChevronUp className="w-3 h-3" />
            选择父组件
          </button>
        )}
      </div>
    </div>
    {/* 选中高亮边框 - 渲染在弹窗外部 */}
    {selectedHighlight}
    </>
  );
}

/** 调试交互处理 */
function DebugInteractionHandler() {
  const { enabled, showComponentInfo, hideComponentInfo } = useComponentDebug();
  const [altPressed, setAltPressed] = useState(false);
  const [hoveredElement, setHoveredElement] = useState<HTMLElement | null>(null);

  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Alt") {
        setAltPressed(true);
      }
      if (e.key === "Escape") {
        hideComponentInfo();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === "Alt") {
        setAltPressed(false);
        setHoveredElement(null);
      }
    };

    const handleBlur = () => {
      setAltPressed(false);
      setHoveredElement(null);
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleBlur);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleBlur);
    };
  }, [enabled, hideComponentInfo]);

  useEffect(() => {
    if (!enabled || !altPressed) {
      setHoveredElement(null);
      return;
    }

    const handleMouseMove = throttle((e: MouseEvent) => {
      try {
        const target = e.target as HTMLElement;
        if (target.closest(".component-debug-popup")) return;
        setHoveredElement(target);
      } catch {
        // 静默处理错误
      }
    }, DEBUG_CONFIG.MOUSEMOVE_THROTTLE_MS);

    document.addEventListener("mousemove", handleMouseMove);
    return () => document.removeEventListener("mousemove", handleMouseMove);
  }, [enabled, altPressed]);

  useEffect(() => {
    if (!enabled) return;

    const handleClick = (e: MouseEvent) => {
      try {
        const target = e.target as HTMLElement;

        if (!target.closest(".component-debug-popup")) {
          if (!e.altKey) {
            hideComponentInfo();
            return;
          }

          e.preventDefault();
          e.stopPropagation();

          const info = getReactFiberInfo(target, e.clientX + 10, e.clientY + 10);
          if (info) {
            showComponentInfo(info);
          } else {
            showComponentInfo({
              name: "DOM Element",
              filePath: "非 React 组件",
              props: {},
              depth: 0,
              tagName: target.tagName || "UNKNOWN",
              x: e.clientX + 10,
              y: e.clientY + 10,
              element: target,
            });
          }
        }
      } catch {
        // 发生错误时静默处理，避免白屏
        console.warn("[ComponentDebugOverlay] 点击处理出错");
      }
    };

    document.addEventListener("click", handleClick, true);
    return () => document.removeEventListener("click", handleClick, true);
  }, [enabled, showComponentInfo, hideComponentInfo]);

  if (!altPressed || !hoveredElement) return null;

  // 安全获取元素边界，如果元素已被移除则返回 null
  let rect: DOMRect | null = null;
  try {
    if (document.contains(hoveredElement)) {
      rect = hoveredElement.getBoundingClientRect();
    }
  } catch {
    // 静默处理错误
  }

  if (!rect) return null;

  return (
    <div
      className="fixed pointer-events-none z-[99998]"
      style={{
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        outline: `2px solid ${DEBUG_CONFIG.HOVER_HIGHLIGHT_COLOR}`,
        outlineOffset: "-2px",
        backgroundColor: DEBUG_CONFIG.HOVER_HIGHLIGHT_BG,
      }}
    />
  );
}

/** 安全的调试覆盖层内容 */
function SafeDebugContent() {
  return (
    <>
      <DebugInteractionHandler />
      <div className="component-debug-popup">
        <ComponentInfoPopup />
      </div>
    </>
  );
}

export function ComponentDebugOverlay() {
  const { enabled } = useComponentDebug();

  // 仅在开发环境启用
  if (process.env.NODE_ENV !== "development") return null;
  if (!enabled) return null;

  // 使用错误边界包装，防止任何错误导致白屏
  return (
    <DebugErrorBoundary>
      <SafeDebugContent />
    </DebugErrorBoundary>
  );
}

// 导出用于测试
export { SelectedHighlight, getComponentName, isValidUserComponent, DEBUG_CONFIG };
