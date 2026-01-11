/**
 * 拦截器配置面板组件
 *
 * 实现拦截配置面板和拦截状态显示
 * **Validates: Requirements 2.1, 2.7, 2.8**
 */

import React, { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import { safeListen } from "@/lib/dev-bridge";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  Shield,
  ShieldOff,
  Settings,
  AlertCircle,
  CheckCircle2,
  Clock,
  Loader2,
  ChevronDown,
  ChevronUp,
  Play,
  X,
  Filter,
  ArrowRight,
  ArrowLeft,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { FilterExpressionInput } from "./FilterExpressionInput";

// ============================================================================
// 类型定义
// ============================================================================

/**
 * 超时动作
 */
export type TimeoutAction = "continue" | "cancel";

/**
 * 拦截配置
 */
export interface InterceptConfig {
  enabled: boolean;
  filter_expr: string | null;
  intercept_request: boolean;
  intercept_response: boolean;
  timeout_ms: number;
  timeout_action: TimeoutAction;
}

/**
 * 拦截状态
 */
export type InterceptState =
  | "pending"
  | "editing"
  | "continued"
  | "cancelled"
  | "timedout";

/**
 * 拦截类型
 */
export type InterceptType = "request" | "response";

/**
 * 被拦截的 Flow
 */
export interface InterceptedFlow {
  flow_id: string;
  state: InterceptState;
  intercept_type: InterceptType;
  original_request?: unknown;
  modified_request?: unknown;
  original_response?: unknown;
  modified_response?: unknown;
  intercepted_at: string;
}

/**
 * 拦截事件
 */
export type InterceptEvent =
  | { type: "FlowIntercepted"; flow: InterceptedFlow }
  | { type: "FlowContinued"; flow_id: string; modified: boolean }
  | { type: "FlowCancelled"; flow_id: string }
  | { type: "FlowTimedOut"; flow_id: string; action: TimeoutAction }
  | { type: "ConfigUpdated"; config: InterceptConfig };

// ============================================================================
// 组件属性
// ============================================================================

interface InterceptPanelProps {
  className?: string;
  onInterceptedFlowSelect?: (flowId: string) => void;
}

// ============================================================================
// 主组件
// ============================================================================

export function InterceptPanel({
  className,
  onInterceptedFlowSelect,
}: InterceptPanelProps) {
  // 状态
  const [config, setConfig] = useState<InterceptConfig>({
    enabled: false,
    filter_expr: null,
    intercept_request: true,
    intercept_response: false,
    timeout_ms: 30000,
    timeout_action: "continue",
  });
  const [interceptedFlows, setInterceptedFlows] = useState<InterceptedFlow[]>(
    [],
  );
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [filterExpr, setFilterExpr] = useState("");
  const [filterValid, setFilterValid] = useState(true);

  // 加载配置
  const loadConfig = useCallback(async () => {
    try {
      setLoading(true);
      const result = await safeInvoke<InterceptConfig>("intercept_config_get");
      setConfig(result);
      setFilterExpr(result.filter_expr || "");
    } catch (e) {
      console.error("加载拦截配置失败:", e);
      setError(e instanceof Error ? e.message : "加载配置失败");
    } finally {
      setLoading(false);
    }
  }, []);

  // 加载被拦截的 Flow 列表
  const loadInterceptedFlows = useCallback(async () => {
    try {
      const flows = await safeInvoke<InterceptedFlow[]>("intercept_list_flows");
      setInterceptedFlows(flows);
    } catch (e) {
      console.error("加载拦截列表失败:", e);
    }
  }, []);

  // 保存配置
  const saveConfig = useCallback(async (newConfig: InterceptConfig) => {
    try {
      setSaving(true);
      setError(null);
      await safeInvoke("intercept_config_set", { config: newConfig });
      setConfig(newConfig);
    } catch (e) {
      console.error("保存拦截配置失败:", e);
      setError(e instanceof Error ? e.message : "保存配置失败");
    } finally {
      setSaving(false);
    }
  }, []);

  // 切换启用状态
  const toggleEnabled = useCallback(async () => {
    const newConfig = { ...config, enabled: !config.enabled };
    await saveConfig(newConfig);
  }, [config, saveConfig]);

  // 更新过滤表达式
  const updateFilterExpr = useCallback(
    async (expr: string) => {
      if (!filterValid && expr) return;
      const newConfig = {
        ...config,
        filter_expr: expr || null,
      };
      await saveConfig(newConfig);
    },
    [config, filterValid, saveConfig],
  );

  // 更新拦截类型
  const updateInterceptType = useCallback(
    async (type: "request" | "response", enabled: boolean) => {
      const newConfig = {
        ...config,
        intercept_request:
          type === "request" ? enabled : config.intercept_request,
        intercept_response:
          type === "response" ? enabled : config.intercept_response,
      };
      await saveConfig(newConfig);
    },
    [config, saveConfig],
  );

  // 更新超时设置
  const updateTimeout = useCallback(
    async (timeoutMs: number, action: TimeoutAction) => {
      const newConfig = {
        ...config,
        timeout_ms: timeoutMs,
        timeout_action: action,
      };
      await saveConfig(newConfig);
    },
    [config, saveConfig],
  );

  // 初始化加载
  useEffect(() => {
    loadConfig();
    loadInterceptedFlows();
  }, [loadConfig, loadInterceptedFlows]);

  // 监听拦截事件
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      try {
        unlisten = await safeListen<InterceptEvent>(
          "intercept-event",
          (event) => {
            const data = event.payload;
            switch (data.type) {
              case "FlowIntercepted":
                setInterceptedFlows((prev) => [...prev, data.flow]);
                break;
              case "FlowContinued":
              case "FlowCancelled":
              case "FlowTimedOut":
                setInterceptedFlows((prev) =>
                  prev.filter((f) => f.flow_id !== data.flow_id),
                );
                break;
              case "ConfigUpdated":
                setConfig(data.config);
                setFilterExpr(data.config.filter_expr || "");
                break;
            }
          },
        );
      } catch (e) {
        console.error("设置拦截事件监听失败:", e);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // 定期刷新拦截列表
  useEffect(() => {
    const interval = setInterval(loadInterceptedFlows, 2000);
    return () => clearInterval(interval);
  }, [loadInterceptedFlows]);

  if (loading) {
    return (
      <div
        className={cn(
          "rounded-lg border bg-card p-4 flex items-center justify-center",
          className,
        )}
      >
        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className={cn("rounded-lg border bg-card", className)}>
      {/* 头部 */}
      <div
        className="flex items-center justify-between px-4 py-3 border-b cursor-pointer hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          {config.enabled ? (
            <Shield className="h-5 w-5 text-orange-500" />
          ) : (
            <ShieldOff className="h-5 w-5 text-muted-foreground" />
          )}
          <span className="font-medium">拦截器</span>
          {interceptedFlows.length > 0 && (
            <span className="px-2 py-0.5 text-xs rounded-full bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-300">
              {interceptedFlows.length} 个待处理
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {/* 快速切换按钮 */}
          <button
            onClick={(e) => {
              e.stopPropagation();
              toggleEnabled();
            }}
            disabled={saving}
            className={cn(
              "px-3 py-1 text-sm rounded-full transition-colors",
              config.enabled
                ? "bg-orange-100 text-orange-700 hover:bg-orange-200 dark:bg-orange-900/30 dark:text-orange-300"
                : "bg-muted text-muted-foreground hover:bg-muted/80",
            )}
          >
            {saving ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : config.enabled ? (
              "已启用"
            ) : (
              "已禁用"
            )}
          </button>
          {expanded ? (
            <ChevronUp className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          )}
        </div>
      </div>

      {/* 展开内容 */}
      {expanded && (
        <div className="p-4 space-y-4">
          {/* 错误提示 */}
          {error && (
            <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 text-red-600 dark:bg-red-950/20 dark:text-red-400 text-sm">
              <AlertCircle className="h-4 w-4 shrink-0" />
              <span>{error}</span>
              <button
                onClick={() => setError(null)}
                className="ml-auto p-1 hover:bg-red-100 dark:hover:bg-red-900/30 rounded"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          )}

          {/* 拦截类型选择 */}
          <div className="space-y-2">
            <label className="text-sm font-medium text-muted-foreground">
              拦截类型
            </label>
            <div className="flex gap-4">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={config.intercept_request}
                  onChange={(e) =>
                    updateInterceptType("request", e.target.checked)
                  }
                  disabled={saving}
                  className="rounded border-gray-300"
                />
                <span className="text-sm flex items-center gap-1">
                  <ArrowRight className="h-4 w-4 text-blue-500" />
                  请求
                </span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={config.intercept_response}
                  onChange={(e) =>
                    updateInterceptType("response", e.target.checked)
                  }
                  disabled={saving}
                  className="rounded border-gray-300"
                />
                <span className="text-sm flex items-center gap-1">
                  <ArrowLeft className="h-4 w-4 text-green-500" />
                  响应
                </span>
              </label>
            </div>
          </div>

          {/* 过滤表达式 */}
          <div className="space-y-2">
            <label className="text-sm font-medium text-muted-foreground flex items-center gap-1">
              <Filter className="h-4 w-4" />
              过滤表达式（可选）
            </label>
            <FilterExpressionInput
              value={filterExpr}
              onChange={setFilterExpr}
              onSubmit={updateFilterExpr}
              onValidationChange={(valid) => setFilterValid(valid)}
              placeholder="输入过滤表达式，如 ~m claude & ~p kiro"
            />
            <p className="text-xs text-muted-foreground">
              留空则拦截所有匹配类型的 Flow
            </p>
          </div>

          {/* 超时设置 */}
          <div className="space-y-2">
            <label className="text-sm font-medium text-muted-foreground flex items-center gap-1">
              <Clock className="h-4 w-4" />
              超时设置
            </label>
            <div className="flex items-center gap-4">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  value={config.timeout_ms / 1000}
                  onChange={(e) => {
                    const seconds = parseInt(e.target.value) || 30;
                    updateTimeout(seconds * 1000, config.timeout_action);
                  }}
                  min={5}
                  max={300}
                  disabled={saving}
                  className="w-20 rounded border bg-background px-2 py-1 text-sm"
                />
                <span className="text-sm text-muted-foreground">秒</span>
              </div>
              <select
                value={config.timeout_action}
                onChange={(e) =>
                  updateTimeout(
                    config.timeout_ms,
                    e.target.value as TimeoutAction,
                  )
                }
                disabled={saving}
                className="rounded border bg-background px-2 py-1 text-sm"
              >
                <option value="continue">超时后继续</option>
                <option value="cancel">超时后取消</option>
              </select>
            </div>
          </div>

          {/* 被拦截的 Flow 列表 */}
          {interceptedFlows.length > 0 && (
            <div className="space-y-2">
              <label className="text-sm font-medium text-muted-foreground">
                待处理的拦截 ({interceptedFlows.length})
              </label>
              <div className="space-y-2 max-h-60 overflow-y-auto">
                {interceptedFlows.map((flow) => (
                  <InterceptedFlowItem
                    key={flow.flow_id}
                    flow={flow}
                    onSelect={() => onInterceptedFlowSelect?.(flow.flow_id)}
                  />
                ))}
              </div>
            </div>
          )}

          {/* 空状态 */}
          {config.enabled && interceptedFlows.length === 0 && (
            <div className="text-center py-6 text-muted-foreground">
              <Shield className="h-8 w-8 mx-auto mb-2 opacity-50" />
              <p className="text-sm">拦截器已启用，等待匹配的 Flow...</p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 子组件
// ============================================================================

interface InterceptedFlowItemProps {
  flow: InterceptedFlow;
  onSelect?: () => void;
}

function InterceptedFlowItem({ flow, onSelect }: InterceptedFlowItemProps) {
  const [continuing, setContinuing] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  const handleContinue = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      setContinuing(true);
      await safeInvoke("intercept_continue", {
        flowId: flow.flow_id,
        modifiedRequest: null,
        modifiedResponse: null,
      });
    } catch (err) {
      console.error("继续 Flow 失败:", err);
    } finally {
      setContinuing(false);
    }
  };

  const handleCancel = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      setCancelling(true);
      await safeInvoke("intercept_cancel", { flowId: flow.flow_id });
    } catch (err) {
      console.error("取消 Flow 失败:", err);
    } finally {
      setCancelling(false);
    }
  };

  const getStateIcon = () => {
    switch (flow.state) {
      case "pending":
        return <Clock className="h-4 w-4 text-yellow-500" />;
      case "editing":
        return <Settings className="h-4 w-4 text-blue-500" />;
      case "continued":
        return <CheckCircle2 className="h-4 w-4 text-green-500" />;
      case "cancelled":
        return <X className="h-4 w-4 text-red-500" />;
      case "timedout":
        return <AlertCircle className="h-4 w-4 text-orange-500" />;
      default:
        return <Clock className="h-4 w-4 text-muted-foreground" />;
    }
  };

  const formatTime = (timestamp: string) => {
    return new Date(timestamp).toLocaleTimeString("zh-CN");
  };

  return (
    <div
      className="flex items-center justify-between p-3 rounded-lg border bg-muted/30 hover:bg-muted/50 cursor-pointer"
      onClick={onSelect}
    >
      <div className="flex items-center gap-3">
        {getStateIcon()}
        <div>
          <div className="text-sm font-mono truncate max-w-[200px]">
            {flow.flow_id.slice(0, 8)}...
          </div>
          <div className="text-xs text-muted-foreground flex items-center gap-2">
            <span
              className={cn(
                "px-1.5 py-0.5 rounded",
                flow.intercept_type === "request"
                  ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300"
                  : "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300",
              )}
            >
              {flow.intercept_type === "request" ? "请求" : "响应"}
            </span>
            <span>{formatTime(flow.intercepted_at)}</span>
          </div>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handleContinue}
          disabled={continuing || cancelling}
          className="p-1.5 rounded hover:bg-green-100 dark:hover:bg-green-900/30 text-green-600 dark:text-green-400"
          title="继续"
        >
          {continuing ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Play className="h-4 w-4" />
          )}
        </button>
        <button
          onClick={handleCancel}
          disabled={continuing || cancelling}
          className="p-1.5 rounded hover:bg-red-100 dark:hover:bg-red-900/30 text-red-600 dark:text-red-400"
          title="取消"
        >
          {cancelling ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <X className="h-4 w-4" />
          )}
        </button>
      </div>
    </div>
  );
}

export default InterceptPanel;
