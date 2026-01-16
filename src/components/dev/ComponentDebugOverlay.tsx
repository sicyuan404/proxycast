/**
 * @file ComponentDebugOverlay.tsx
 * @description 组件视图调试覆盖层 - Alt+悬浮显示轮廓，Alt+点击显示组件信息
 */
import React, { useEffect, useState, useMemo } from "react";
import {
  useComponentDebug,
  ComponentInfo,
} from "@/contexts/ComponentDebugContext";
import {
  X,
  Copy,
  Check,
  Component,
  FileCode,
  Layers,
  Hash,
  ChevronUp,
} from "lucide-react";

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
            errorCount: prev.errorCount + 1,
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
  memoizedProps: (Record<string, unknown> & { __source?: DebugSource }) | null;
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
function getComponentName(
  type: FiberType | ((...args: unknown[]) => unknown) | null,
): string {
  if (!type) return "Unknown";

  // 直接函数组件
  if (typeof type === "function") {
    const funcType = type as {
      displayName?: string;
      name?: string;
      styledComponentId?: string;
      target?: unknown;
    };

    // styled-components 有 styledComponentId 属性，尝试获取更好的名称
    if (funcType.styledComponentId) {
      // 如果有 displayName 且不是 styled.xxx 格式，使用它
      if (
        funcType.displayName &&
        !isStyledComponentName(funcType.displayName)
      ) {
        return funcType.displayName;
      }
      // 否则返回特殊标记，让调用者知道这是 styled-component
      return `[styled]${funcType.displayName || funcType.name || "Component"}`;
    }

    return funcType.displayName || funcType.name || "Anonymous";
  }

  // 对象类型（memo、forwardRef 等）
  if (typeof type === "object") {
    const fiberType = type as FiberType & {
      styledComponentId?: string;
      target?: unknown;
    };

    // styled-components 检测
    if (fiberType.styledComponentId) {
      if (
        fiberType.displayName &&
        !isStyledComponentName(fiberType.displayName)
      ) {
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
// eslint-disable-next-line react-refresh/only-export-components
export function throttle<T extends (...args: Parameters<T>) => void>(
  fn: T,
  delay: number,
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
  HOVER_HIGHLIGHT_COLOR: "rgba(59, 130, 246, 0.8)", // 蓝色
  HOVER_HIGHLIGHT_BG: "rgba(59, 130, 246, 0.1)",
  SELECTED_HIGHLIGHT_COLOR: "rgba(34, 197, 94, 0.9)", // 绿色
  SELECTED_HIGHLIGHT_BG: "rgba(34, 197, 94, 0.15)",
  // 节流间隔
  MOUSEMOVE_THROTTLE_MS: 16,
} as const;

/**
 * 从 Fiber 节点提取组件信息
 */
function extractFiberInfo(
  fiber: FiberNode,
  element: HTMLElement,
  x: number,
  y: number,
): ComponentInfo | null {
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
      if (key.startsWith("_") || key === "__source" || key === "__self")
        continue;
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
function getReactFiberInfo(
  element: HTMLElement,
  x: number,
  y: number,
): ComponentInfo | null {
  try {
    const fiberKey = Object.keys(element).find(
      (key) =>
        key.startsWith("__reactFiber$") ||
        key.startsWith("__reactInternalInstance$"),
    );

    if (!fiberKey) return null;

    let fiber: FiberNode | null = (
      element as unknown as Record<string, FiberNode>
    )[fiberKey];
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
 * 递归遍历子节点，找到第一个 DOM 元素
 */
function findDomElement(fiber: FiberNode | null): HTMLElement | null {
  if (!fiber) return null;

  // 如果当前节点有 stateNode 且是 DOM 元素
  if (fiber.stateNode instanceof HTMLElement) {
    return fiber.stateNode;
  }

  // 递归查找子节点
  let child = fiber.child;
  while (child) {
    const result = findDomElement(child);
    if (result) return result;
    child = child.sibling;
  }

  return null;
}

/**
 * 获取父组件信息
 */
function getParentComponentInfo(
  currentFiber: FiberNode | unknown,
  x: number,
  y: number,
  fallbackElement: HTMLElement,
): ComponentInfo | null {
  try {
    if (!currentFiber) return null;

    let fiber = (currentFiber as FiberNode).return;

    while (fiber) {
      if (isValidUserComponent(fiber)) {
        // 尝试找到父组件对应的 DOM 元素
        let parentElement = findDomElement(fiber);

        // 如果找不到父组件的 DOM 元素，使用子元素的父元素
        if (!parentElement && fallbackElement.parentElement) {
          parentElement = fallbackElement.parentElement;
        }

        // 如果还是找不到，使用 fallbackElement
        if (!parentElement) {
          parentElement = fallbackElement;
        }

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

/**
 * 获取完整的组件层级路径（从当前组件到根组件）
 */
function getComponentHierarchy(
  currentFiber: FiberNode | unknown,
  currentElement: HTMLElement,
  x: number,
  y: number,
): ComponentInfo[] {
  const hierarchy: ComponentInfo[] = [];
  
  try {
    if (!currentFiber) return hierarchy;

    // 添加当前组件
    if (currentFiber && isValidUserComponent(currentFiber as FiberNode)) {
      const currentInfo = extractFiberInfo(
        currentFiber as FiberNode,
        currentElement,
        x,
        y,
      );
      if (currentInfo) {
        hierarchy.push(currentInfo);
      }
    }

    // 遍历父组件
    let fiber = (currentFiber as FiberNode).return;
    while (fiber) {
      if (isValidUserComponent(fiber)) {
        let parentElement = findDomElement(fiber);
        if (!parentElement && currentElement.parentElement) {
          parentElement = currentElement.parentElement;
        }
        if (!parentElement) {
          parentElement = currentElement;
        }

        const parentInfo = extractFiberInfo(fiber, parentElement, x, y);
        if (parentInfo) {
          hierarchy.push(parentInfo);
          currentElement = parentElement;
        }
      }
      fiber = fiber.return;
    }
  } catch {
    // 发生错误时返回已收集的层级
  }

  return hierarchy;
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
  const { componentInfo, hideComponentInfo, showComponentInfo } =
    useComponentDebug();
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const [showHierarchyDropdown, setShowHierarchyDropdown] = useState(false);
  const dropdownRef = React.useRef<HTMLDivElement>(null);

  // 使用 useMemo 缓存 propsEntries 计算结果 - 必须在所有条件返回之前调用
  const propsEntries = useMemo(() => {
    if (!componentInfo?.props) return [];
    return Object.entries(componentInfo.props).filter(
      ([key]) => key !== "children",
    );
  }, [componentInfo?.props]);

  // 计算组件层级路径
  const hierarchy = useMemo(() => {
    if (!componentInfo?.fiber || !componentInfo.element) return [];
    return getComponentHierarchy(
      componentInfo.fiber,
      componentInfo.element,
      componentInfo.x,
      componentInfo.y,
    );
  }, [componentInfo?.fiber, componentInfo?.element, componentInfo?.x, componentInfo?.y]);

  // 点击外部关闭下拉菜单
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setShowHierarchyDropdown(false);
      }
    };

    if (showHierarchyDropdown) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [showHierarchyDropdown]);

  // 早期返回必须在所有 hooks 之后
  if (!componentInfo) return null;

  // 渲染选中高亮边框
  const selectedHighlight = (
    <SelectedHighlight element={componentInfo.element} />
  );

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
      componentInfo.element || document.body,
    );

    if (parentInfo) {
      showComponentInfo(parentInfo);
    }
  };

  const handleSelectHierarchyItem = (info: ComponentInfo) => {
    showComponentInfo(info);
    setShowHierarchyDropdown(false);
  };

  // 检查是否有父组件
  const hasParent = componentInfo.fiber?.return != null;

  return (
    <>
      <div
        className="fixed z-[99999] rounded-lg shadow-xl min-w-[320px] max-w-[450px] border border-gray-200 bg-white text-gray-900"
        style={{
          left: Math.min(
            componentInfo.x,
            window.innerWidth - DEBUG_CONFIG.POPUP_WIDTH_OFFSET,
          ),
          top: Math.min(
            componentInfo.y,
            window.innerHeight - DEBUG_CONFIG.POPUP_HEIGHT_OFFSET,
          ),
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* 标题栏 */}
        <div className="flex items-center justify-between px-3 py-2 border-b border-gray-200 rounded-t-lg bg-gray-50">
          <div className="flex items-center gap-2">
            <Component className="w-4 h-4 text-blue-500" />
            <span className="font-semibold text-sm">组件信息</span>
          </div>
          <button
            onClick={hideComponentInfo}
            className="p-1 hover:bg-gray-200 rounded transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* 面包屑导航 */}
        {hierarchy.length > 1 && (
          <div className="px-3 py-2 border-b border-gray-200 bg-gray-50/50">
            <div className="flex items-center gap-1 overflow-x-auto text-xs">
              {hierarchy.slice().reverse().map((item, index) => {
                const isLast = index === hierarchy.length - 1;
                return (
                  <React.Fragment key={index}>
                    <button
                      onClick={() => handleSelectHierarchyItem(item)}
                      className={`truncate hover:text-blue-600 transition-colors ${
                        isLast
                          ? "font-medium text-blue-600"
                          : "text-gray-600"
                      }`}
                      title={item.name}
                    >
                      {item.name}
                    </button>
                    {!isLast && (
                      <span className="text-gray-400 shrink-0">›</span>
                    )}
                  </React.Fragment>
                );
              })}
            </div>
          </div>
        )}

        {/* 内容区域 */}
        <div className="p-3 space-y-3">
          {/* 组件名称 */}
          <div className="flex items-start gap-2">
            <Component className="w-4 h-4 text-blue-500 mt-0.5 shrink-0" />
            <div className="flex-1 min-w-0">
              <div className="text-xs text-gray-500 mb-0.5">组件名称</div>
              <div className="flex items-center gap-2">
                <code className="font-mono text-sm text-blue-600 font-medium">
                  {componentInfo.name}
                </code>
                <button
                  onClick={() => handleCopy(componentInfo.name, "name")}
                  className="p-0.5 hover:bg-gray-100 rounded shrink-0"
                  title="复制名称"
                >
                  {copiedField === "name" ? (
                    <Check className="w-3 h-3 text-green-500" />
                  ) : (
                    <Copy className="w-3 h-3 text-gray-400" />
                  )}
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
                <code className="text-xs bg-gray-100 px-2 py-1 rounded truncate flex-1 block text-gray-700">
                  {componentInfo.filePath}
                </code>
                <button
                  onClick={() => handleCopy(componentInfo.filePath, "path")}
                  className="p-0.5 hover:bg-gray-100 rounded shrink-0"
                  title="复制路径"
                >
                  {copiedField === "path" ? (
                    <Check className="w-3 h-3 text-green-500" />
                  ) : (
                    <Copy className="w-3 h-3 text-gray-400" />
                  )}
                </button>
              </div>
            </div>
          </div>

          {/* HTML 标签 */}
          <div className="flex items-start gap-2">
            <Hash className="w-4 h-4 text-purple-500 mt-0.5 shrink-0" />
            <div className="flex-1 min-w-0">
              <div className="text-xs text-gray-500 mb-0.5">DOM 元素</div>
              <code className="text-xs text-gray-600">
                &lt;{componentInfo.tagName.toLowerCase()}&gt;
              </code>
            </div>
          </div>

          {/* 组件层级 */}
          <div className="flex items-start gap-2">
            <Layers className="w-4 h-4 text-green-500 mt-0.5 shrink-0" />
            <div className="flex-1 min-w-0">
              <div className="text-xs text-gray-500 mb-0.5">组件层级</div>
              <span className="text-xs text-gray-700">
                第 {componentInfo.depth} 层
              </span>
            </div>
          </div>

          {/* Props */}
          {propsEntries.length > 0 && (
            <div className="border-t border-gray-200 pt-3">
              <div className="text-xs text-gray-500 mb-2">Props</div>
              <div className="bg-gray-50 rounded p-2 max-h-[120px] overflow-auto">
                <div className="space-y-1">
                  {propsEntries
                    .slice(0, DEBUG_CONFIG.MAX_PROPS_DISPLAY)
                    .map(([key, value]) => (
                      <div key={key} className="flex items-start gap-2 text-xs">
                        <span className="text-blue-500 font-mono shrink-0">
                          {key}:
                        </span>
                        <span className="text-gray-600 font-mono truncate">
                          {typeof value === "string"
                            ? `"${value}"`
                            : String(value)}
                        </span>
                      </div>
                    ))}
                  {propsEntries.length > DEBUG_CONFIG.MAX_PROPS_DISPLAY && (
                    <div className="text-xs text-gray-400">
                      ... 还有{" "}
                      {propsEntries.length - DEBUG_CONFIG.MAX_PROPS_DISPLAY}{" "}
                      个属性
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="px-3 py-2 border-t border-gray-200 rounded-b-lg bg-gray-50 flex items-center justify-between relative">
          <p className="text-[10px] text-gray-400">按 Esc 关闭</p>
          <div className="flex items-center gap-2">
            {hierarchy.length > 1 && (
              <div className="relative" ref={dropdownRef}>
                <button
                  onClick={() => setShowHierarchyDropdown(!showHierarchyDropdown)}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-gray-600 text-white rounded hover:bg-gray-700 transition-colors"
                >
                  <Layers className="w-3 h-3" />
                  层级树
                </button>
                {/* 下拉菜单 */}
                {showHierarchyDropdown && (
                  <div className="absolute bottom-full right-0 mb-2 w-64 bg-white border border-gray-200 rounded-lg shadow-xl max-h-64 overflow-auto z-[100000]">
                    <div className="p-2">
                      <div className="text-xs text-gray-500 mb-2">组件层级（点击跳转）</div>
                      {hierarchy.slice().reverse().map((item, index) => (
                        <button
                          key={index}
                          onClick={() => handleSelectHierarchyItem(item)}
                          className={`w-full text-left px-2 py-1.5 rounded text-xs flex items-center gap-2 hover:bg-gray-100 transition-colors ${
                            index === hierarchy.length - 1
                              ? "bg-blue-50 text-blue-600 font-medium"
                              : "text-gray-700"
                          }`}
                        >
                          <Layers className="w-3 h-3 shrink-0" />
                          <span className="truncate">{item.name}</span>
                          <span className="text-gray-400 text-[10px] shrink-0">
                            L{item.depth}
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
            {hasParent && (
              <button
                onClick={handleSelectParent}
                className="flex items-center gap-1 px-2 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
              >
                <ChevronUp className="w-3 h-3" />
                切换上一层
              </button>
            )}
          </div>
        </div>
      </div>
      {/* 选中高亮边框 - 渲染在弹窗外部 */}
      {selectedHighlight}
    </>
  );
}

/** 调试交互处理 */
function DebugInteractionHandler() {
  const { enabled, showComponentInfo, hideComponentInfo, componentInfo } = useComponentDebug();
  const [altPressed, setAltPressed] = useState(false);
  const [hoveredElement, setHoveredElement] = useState<HTMLElement | null>(
    null,
  );

  // 添加/移除全局调试样式类
  useEffect(() => {
    if (enabled) {
      document.body.classList.add("component-debug-mode");
    } else {
      document.body.classList.remove("component-debug-mode");
    }

    return () => {
      document.body.classList.remove("component-debug-mode");
    };
  }, [enabled]);

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
        const isPopupClick = target.closest(".component-debug-popup");

        if (!isPopupClick) {
          if (e.altKey) {
            // 按住 Alt 键：显示组件信息
            e.preventDefault();
            e.stopPropagation();

            const info = getReactFiberInfo(
              target,
              e.clientX + 10,
              e.clientY + 10,
            );
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
          } else {
            // 没有按住 Alt 键
            // 如果有组件信息弹窗显示，隐藏它
            if (componentInfo) {
              hideComponentInfo();
            }
            // 让事件正常传播，不影响其他组件的点击行为
            return;
          }
        }
      } catch {
        // 发生错误时静默处理，避免白屏
        console.warn("[ComponentDebugOverlay] 点击处理出错");
      }
    };

    document.addEventListener("click", handleClick, true);
    return () => document.removeEventListener("click", handleClick, true);
  }, [enabled, showComponentInfo, hideComponentInfo, componentInfo]);

  // 只有在 Alt 按下且有悬浮元素时才显示高亮
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
        outline: `3px solid ${DEBUG_CONFIG.HOVER_HIGHLIGHT_COLOR}`,
        outlineOffset: "-2px",
        backgroundColor: DEBUG_CONFIG.HOVER_HIGHLIGHT_BG,
        boxShadow: `0 0 10px ${DEBUG_CONFIG.HOVER_HIGHLIGHT_COLOR}`,
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
  if (!import.meta.env.DEV) return null;
  if (!enabled) return null;

  // 使用错误边界包装，防止任何错误导致白屏
  return (
    <DebugErrorBoundary>
      {/* 全局调试样式 */}
      <style>{`
        /* 组件调试模式：显示所有组件边框 */
        body.component-debug-mode * {
          outline: 1px solid rgba(156, 163, 175, 0.3) !important;
          outline-offset: -1px !important;
        }

        /* 调试模式下，调试覆盖层和弹窗不受影响 */
        body.component-debug-mode .component-debug-popup *,
        body.component-debug-mode [class*="fixed"][style*="z-index"],
        body.component-debug-mode [style*="z-index"] {
          outline: none !important;
        }

        /* 调试模式下，输入框和按钮使用不同的边框颜色 */
        body.component-debug-mode input,
        body.component-debug-mode textarea,
        body.component-debug-mode button {
          outline-color: rgba(59, 130, 246, 0.4) !important;
        }

        /* 调试模式下，文本节点使用浅色边框 */
        body.component-debug-mode p,
        body.component-debug-mode span,
        body.component-debug-mode div {
          outline-color: rgba(156, 163, 175, 0.2) !important;
        }
      `}</style>
      <SafeDebugContent />
    </DebugErrorBoundary>
  );
}

/* eslint-disable react-refresh/only-export-components */
// 导出用于测试
export {
  SelectedHighlight,
  getComponentName,
  isValidUserComponent,
  DEBUG_CONFIG,
};
/* eslint-enable react-refresh/only-export-components */
