/**
 * Flow 差异对比视图组件
 *
 * 实现差异对比视图和并排/统一视图切换
 * **Validates: Requirements 4.1-4.7**
 */

import React, { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  X,
  Loader2,
  AlertCircle,
  ArrowLeftRight,
  Columns,
  Rows,
  ChevronDown,
  ChevronRight,
  Plus,
  Minus,
  Edit3,
  Settings,
  MessageSquare,
  Zap,
  FileJson,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { LLMFlow } from "@/lib/api/flowMonitor";

// ============================================================================
// 类型定义
// ============================================================================

/**
 * 差异类型
 */
export type DiffType = "Added" | "Removed" | "Modified" | "Unchanged";

/**
 * 差异项
 */
export interface DiffItem {
  path: string;
  diff_type: DiffType;
  left_value: unknown;
  right_value: unknown;
}

/**
 * 消息差异项
 */
export interface MessageDiffItem {
  index: number;
  diff_type: DiffType;
  left_message: unknown;
  right_message: unknown;
  content_diffs: DiffItem[];
}

/**
 * Token 差异
 */
export interface TokenDiff {
  input_diff: number;
  output_diff: number;
  total_diff: number;
}

/**
 * 差异配置
 */
export interface DiffConfig {
  ignore_fields: string[];
  ignore_timestamps: boolean;
  ignore_ids: boolean;
}

/**
 * Flow 差异结果
 */
export interface FlowDiffResult {
  left_flow_id: string;
  right_flow_id: string;
  request_diffs: DiffItem[];
  response_diffs: DiffItem[];
  metadata_diffs: DiffItem[];
  message_diffs: MessageDiffItem[];
  token_diff: TokenDiff;
}

/**
 * 视图模式
 */
export type ViewMode = "side-by-side" | "unified";

// ============================================================================
// 组件属性
// ============================================================================

interface FlowDiffViewProps {
  /** 左侧 Flow ID */
  leftFlowId: string;
  /** 右侧 Flow ID */
  rightFlowId: string;
  /** 左侧 Flow（可选，如果提供则不需要加载） */
  leftFlow?: LLMFlow;
  /** 右侧 Flow（可选，如果提供则不需要加载） */
  rightFlow?: LLMFlow;
  /** 关闭回调 */
  onClose?: () => void;
  /** 自定义类名 */
  className?: string;
}

// ============================================================================
// 主组件
// ============================================================================

export function FlowDiffView({
  leftFlowId,
  rightFlowId,
  leftFlow: initialLeftFlow,
  rightFlow: initialRightFlow,
  onClose,
  className,
}: FlowDiffViewProps) {
  // 状态
  const [leftFlow, setLeftFlow] = useState<LLMFlow | null>(
    initialLeftFlow || null,
  );
  const [rightFlow, setRightFlow] = useState<LLMFlow | null>(
    initialRightFlow || null,
  );
  const [diffResult, setDiffResult] = useState<FlowDiffResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("side-by-side");
  const [config, setConfig] = useState<DiffConfig>({
    ignore_fields: [],
    ignore_timestamps: true,
    ignore_ids: true,
  });
  const [showConfig, setShowConfig] = useState(false);
  const [activeSection, setActiveSection] = useState<string>("request");
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  // 加载 Flow 和计算差异
  const loadDiff = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      // 调用后端计算差异
      const result = await safeInvoke<FlowDiffResult>("diff_flows", {
        request: {
          left_flow_id: leftFlowId,
          right_flow_id: rightFlowId,
          config,
        },
      });

      setDiffResult(result);

      // 如果没有提供 Flow，加载它们用于显示
      if (!initialLeftFlow) {
        const left = await safeInvoke<LLMFlow | null>("get_flow_detail", {
          flowId: leftFlowId,
        });
        setLeftFlow(left);
      }
      if (!initialRightFlow) {
        const right = await safeInvoke<LLMFlow | null>("get_flow_detail", {
          flowId: rightFlowId,
        });
        setRightFlow(right);
      }
    } catch (e) {
      console.error("加载差异失败:", e);
      setError(e instanceof Error ? e.message : "加载差异失败");
    } finally {
      setLoading(false);
    }
  }, [leftFlowId, rightFlowId, config, initialLeftFlow, initialRightFlow]);

  useEffect(() => {
    loadDiff();
  }, [loadDiff]);

  // 切换路径展开状态
  const togglePath = (path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  if (loading) {
    return (
      <div
        className={cn(
          "rounded-lg border bg-card p-8 flex items-center justify-center",
          className,
        )}
      >
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error) {
    return (
      <div className={cn("rounded-lg border bg-card p-4", className)}>
        <div className="flex items-center gap-2 text-red-600 dark:text-red-400">
          <AlertCircle className="h-5 w-5" />
          <span>{error}</span>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            className="mt-3 text-sm text-muted-foreground hover:text-foreground"
          >
            关闭
          </button>
        )}
      </div>
    );
  }

  if (!diffResult) {
    return null;
  }

  return (
    <div className={cn("rounded-lg border bg-card flex flex-col", className)}>
      {/* 头部 */}
      <DiffHeader
        leftFlow={leftFlow}
        rightFlow={rightFlow}
        viewMode={viewMode}
        onViewModeChange={setViewMode}
        showConfig={showConfig}
        onToggleConfig={() => setShowConfig(!showConfig)}
        onClose={onClose}
      />

      {/* 配置面板 */}
      {showConfig && <DiffConfigPanel config={config} onChange={setConfig} />}

      {/* Token 差异摘要 */}
      <TokenDiffSummary tokenDiff={diffResult.token_diff} />

      {/* 标签页 */}
      <div className="flex border-b px-4">
        <DiffTabButton
          active={activeSection === "request"}
          onClick={() => setActiveSection("request")}
          count={
            diffResult.request_diffs.filter((d) => d.diff_type !== "Unchanged")
              .length
          }
        >
          请求
        </DiffTabButton>
        <DiffTabButton
          active={activeSection === "response"}
          onClick={() => setActiveSection("response")}
          count={
            diffResult.response_diffs.filter((d) => d.diff_type !== "Unchanged")
              .length
          }
        >
          响应
        </DiffTabButton>
        <DiffTabButton
          active={activeSection === "messages"}
          onClick={() => setActiveSection("messages")}
          count={
            diffResult.message_diffs.filter((d) => d.diff_type !== "Unchanged")
              .length
          }
        >
          消息
        </DiffTabButton>
        <DiffTabButton
          active={activeSection === "metadata"}
          onClick={() => setActiveSection("metadata")}
          count={
            diffResult.metadata_diffs.filter((d) => d.diff_type !== "Unchanged")
              .length
          }
        >
          元数据
        </DiffTabButton>
      </div>

      {/* 差异内容 */}
      <div className="flex-1 overflow-auto p-4">
        {activeSection === "request" && (
          <DiffSection
            diffs={diffResult.request_diffs}
            viewMode={viewMode}
            expandedPaths={expandedPaths}
            onTogglePath={togglePath}
          />
        )}
        {activeSection === "response" && (
          <DiffSection
            diffs={diffResult.response_diffs}
            viewMode={viewMode}
            expandedPaths={expandedPaths}
            onTogglePath={togglePath}
          />
        )}
        {activeSection === "messages" && (
          <MessageDiffSection
            diffs={diffResult.message_diffs}
            viewMode={viewMode}
          />
        )}
        {activeSection === "metadata" && (
          <DiffSection
            diffs={diffResult.metadata_diffs}
            viewMode={viewMode}
            expandedPaths={expandedPaths}
            onTogglePath={togglePath}
          />
        )}
      </div>
    </div>
  );
}

// ============================================================================
// 头部组件
// ============================================================================

interface DiffHeaderProps {
  leftFlow: LLMFlow | null;
  rightFlow: LLMFlow | null;
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  showConfig: boolean;
  onToggleConfig: () => void;
  onClose?: () => void;
}

function DiffHeader({
  leftFlow,
  rightFlow,
  viewMode,
  onViewModeChange,
  showConfig,
  onToggleConfig,
  onClose,
}: DiffHeaderProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 border-b">
      <div className="flex items-center gap-3">
        <ArrowLeftRight className="h-5 w-5 text-primary" />
        <div className="flex items-center gap-2 text-sm">
          <span className="font-mono text-muted-foreground">
            {leftFlow?.id.slice(0, 8) || "..."}
          </span>
          <span className="text-muted-foreground">vs</span>
          <span className="font-mono text-muted-foreground">
            {rightFlow?.id.slice(0, 8) || "..."}
          </span>
        </div>
      </div>

      <div className="flex items-center gap-2">
        {/* 视图模式切换 */}
        <div className="flex rounded-lg border overflow-hidden">
          <button
            onClick={() => onViewModeChange("side-by-side")}
            className={cn(
              "p-1.5",
              viewMode === "side-by-side"
                ? "bg-primary text-primary-foreground"
                : "hover:bg-muted",
            )}
            title="并排视图"
          >
            <Columns className="h-4 w-4" />
          </button>
          <button
            onClick={() => onViewModeChange("unified")}
            className={cn(
              "p-1.5",
              viewMode === "unified"
                ? "bg-primary text-primary-foreground"
                : "hover:bg-muted",
            )}
            title="统一视图"
          >
            <Rows className="h-4 w-4" />
          </button>
        </div>

        {/* 配置按钮 */}
        <button
          onClick={onToggleConfig}
          className={cn(
            "p-1.5 rounded hover:bg-muted",
            showConfig && "bg-muted",
          )}
          title="配置"
        >
          <Settings className="h-4 w-4 text-muted-foreground" />
        </button>

        {/* 关闭按钮 */}
        {onClose && (
          <button
            onClick={onClose}
            className="p-1.5 rounded hover:bg-muted"
            title="关闭"
          >
            <X className="h-4 w-4" />
          </button>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// 配置面板
// ============================================================================

interface DiffConfigPanelProps {
  config: DiffConfig;
  onChange: (config: DiffConfig) => void;
}

function DiffConfigPanel({ config, onChange }: DiffConfigPanelProps) {
  return (
    <div className="px-4 py-3 border-b bg-muted/30 space-y-3">
      <div className="text-sm font-medium">差异配置</div>
      <div className="flex flex-wrap gap-4">
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={config.ignore_timestamps}
            onChange={(e) =>
              onChange({ ...config, ignore_timestamps: e.target.checked })
            }
            className="rounded border-gray-300"
          />
          <span className="text-sm">忽略时间戳</span>
        </label>
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={config.ignore_ids}
            onChange={(e) =>
              onChange({ ...config, ignore_ids: e.target.checked })
            }
            className="rounded border-gray-300"
          />
          <span className="text-sm">忽略 ID</span>
        </label>
      </div>
    </div>
  );
}

// ============================================================================
// Token 差异摘要
// ============================================================================

interface TokenDiffSummaryProps {
  tokenDiff: TokenDiff;
}

function TokenDiffSummary({ tokenDiff }: TokenDiffSummaryProps) {
  const hasDiff =
    tokenDiff.input_diff !== 0 ||
    tokenDiff.output_diff !== 0 ||
    tokenDiff.total_diff !== 0;

  if (!hasDiff) return null;

  const formatDiff = (diff: number) => {
    if (diff > 0) return `+${diff}`;
    return diff.toString();
  };

  const getDiffColor = (diff: number) => {
    if (diff > 0) return "text-green-600";
    if (diff < 0) return "text-red-600";
    return "text-muted-foreground";
  };

  return (
    <div className="px-4 py-2 border-b bg-muted/20">
      <div className="flex items-center gap-4 text-sm">
        <Zap className="h-4 w-4 text-muted-foreground" />
        <span className="text-muted-foreground">Token 差异:</span>
        <span className={getDiffColor(tokenDiff.input_diff)}>
          输入 {formatDiff(tokenDiff.input_diff)}
        </span>
        <span className={getDiffColor(tokenDiff.output_diff)}>
          输出 {formatDiff(tokenDiff.output_diff)}
        </span>
        <span className={cn("font-medium", getDiffColor(tokenDiff.total_diff))}>
          总计 {formatDiff(tokenDiff.total_diff)}
        </span>
      </div>
    </div>
  );
}

// ============================================================================
// 标签页按钮
// ============================================================================

interface DiffTabButtonProps {
  active: boolean;
  onClick: () => void;
  count: number;
  children: React.ReactNode;
}

function DiffTabButton({
  active,
  onClick,
  count,
  children,
}: DiffTabButtonProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors flex items-center gap-2",
        active
          ? "border-primary text-primary"
          : "border-transparent text-muted-foreground hover:text-foreground",
      )}
    >
      {children}
      {count > 0 && (
        <span
          className={cn(
            "px-1.5 py-0.5 text-xs rounded-full",
            active
              ? "bg-primary/10 text-primary"
              : "bg-muted text-muted-foreground",
          )}
        >
          {count}
        </span>
      )}
    </button>
  );
}

// ============================================================================
// 差异区域组件
// ============================================================================

interface DiffSectionProps {
  diffs: DiffItem[];
  viewMode: ViewMode;
  expandedPaths: Set<string>;
  onTogglePath: (path: string) => void;
}

function DiffSection({
  diffs,
  viewMode,
  expandedPaths,
  onTogglePath,
}: DiffSectionProps) {
  // 过滤掉未变化的项
  const changedDiffs = diffs.filter((d) => d.diff_type !== "Unchanged");

  if (changedDiffs.length === 0) {
    return (
      <div className="text-center py-8 text-muted-foreground">
        <FileJson className="h-8 w-8 mx-auto mb-2 opacity-50" />
        <p>没有差异</p>
      </div>
    );
  }

  if (viewMode === "side-by-side") {
    return (
      <div className="space-y-2">
        {changedDiffs.map((diff, idx) => (
          <SideBySideDiffItem
            key={idx}
            diff={diff}
            expanded={expandedPaths.has(diff.path)}
            onToggle={() => onTogglePath(diff.path)}
          />
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {changedDiffs.map((diff, idx) => (
        <UnifiedDiffItem
          key={idx}
          diff={diff}
          expanded={expandedPaths.has(diff.path)}
          onToggle={() => onTogglePath(diff.path)}
        />
      ))}
    </div>
  );
}

// ============================================================================
// 并排差异项
// ============================================================================

interface SideBySideDiffItemProps {
  diff: DiffItem;
  expanded: boolean;
  onToggle: () => void;
}

function SideBySideDiffItem({
  diff,
  expanded,
  onToggle,
}: SideBySideDiffItemProps) {
  const isLongValue =
    JSON.stringify(diff.left_value || diff.right_value).length > 100;

  return (
    <div className="rounded border overflow-hidden">
      {/* 路径头部 */}
      <div
        className={cn(
          "flex items-center gap-2 px-3 py-2 cursor-pointer",
          getDiffBgColor(diff.diff_type, true),
        )}
        onClick={onToggle}
      >
        {isLongValue ? (
          expanded ? (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          )
        ) : (
          <span className="w-4" />
        )}
        <DiffTypeIcon type={diff.diff_type} />
        <span className="font-mono text-sm">{diff.path}</span>
      </div>

      {/* 值对比 */}
      {(!isLongValue || expanded) && (
        <div className="grid grid-cols-2 divide-x">
          {/* 左侧值 */}
          <div
            className={cn(
              "p-3",
              diff.diff_type === "Removed" || diff.diff_type === "Modified"
                ? "bg-red-50/50 dark:bg-red-950/10"
                : "bg-muted/30",
            )}
          >
            {diff.left_value !== null && diff.left_value !== undefined ? (
              <pre className="text-xs font-mono whitespace-pre-wrap break-words">
                {formatValue(diff.left_value)}
              </pre>
            ) : (
              <span className="text-xs text-muted-foreground italic">(无)</span>
            )}
          </div>

          {/* 右侧值 */}
          <div
            className={cn(
              "p-3",
              diff.diff_type === "Added" || diff.diff_type === "Modified"
                ? "bg-green-50/50 dark:bg-green-950/10"
                : "bg-muted/30",
            )}
          >
            {diff.right_value !== null && diff.right_value !== undefined ? (
              <pre className="text-xs font-mono whitespace-pre-wrap break-words">
                {formatValue(diff.right_value)}
              </pre>
            ) : (
              <span className="text-xs text-muted-foreground italic">(无)</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 统一差异项
// ============================================================================

interface UnifiedDiffItemProps {
  diff: DiffItem;
  expanded: boolean;
  onToggle: () => void;
}

function UnifiedDiffItem({ diff, expanded, onToggle }: UnifiedDiffItemProps) {
  const isLongValue =
    JSON.stringify(diff.left_value || diff.right_value).length > 100;

  return (
    <div className="rounded border overflow-hidden">
      {/* 路径头部 */}
      <div
        className={cn(
          "flex items-center gap-2 px-3 py-2 cursor-pointer",
          getDiffBgColor(diff.diff_type, true),
        )}
        onClick={onToggle}
      >
        {isLongValue ? (
          expanded ? (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          )
        ) : (
          <span className="w-4" />
        )}
        <DiffTypeIcon type={diff.diff_type} />
        <span className="font-mono text-sm">{diff.path}</span>
      </div>

      {/* 值显示 */}
      {(!isLongValue || expanded) && (
        <div className="space-y-0">
          {/* 删除的值 */}
          {(diff.diff_type === "Removed" || diff.diff_type === "Modified") &&
            diff.left_value !== null &&
            diff.left_value !== undefined && (
              <div className="flex bg-red-50/50 dark:bg-red-950/10">
                <div className="w-8 flex items-center justify-center text-red-600 border-r">
                  <Minus className="h-3 w-3" />
                </div>
                <pre className="flex-1 p-2 text-xs font-mono whitespace-pre-wrap break-words">
                  {formatValue(diff.left_value)}
                </pre>
              </div>
            )}

          {/* 新增的值 */}
          {(diff.diff_type === "Added" || diff.diff_type === "Modified") &&
            diff.right_value !== null &&
            diff.right_value !== undefined && (
              <div className="flex bg-green-50/50 dark:bg-green-950/10">
                <div className="w-8 flex items-center justify-center text-green-600 border-r">
                  <Plus className="h-3 w-3" />
                </div>
                <pre className="flex-1 p-2 text-xs font-mono whitespace-pre-wrap break-words">
                  {formatValue(diff.right_value)}
                </pre>
              </div>
            )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 消息差异区域
// ============================================================================

interface MessageDiffSectionProps {
  diffs: MessageDiffItem[];
  viewMode: ViewMode;
}

function MessageDiffSection({ diffs, viewMode }: MessageDiffSectionProps) {
  const changedDiffs = diffs.filter((d) => d.diff_type !== "Unchanged");

  if (changedDiffs.length === 0) {
    return (
      <div className="text-center py-8 text-muted-foreground">
        <MessageSquare className="h-8 w-8 mx-auto mb-2 opacity-50" />
        <p>消息列表没有差异</p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {diffs.map((diff, idx) => (
        <MessageDiffItemView key={idx} diff={diff} viewMode={viewMode} />
      ))}
    </div>
  );
}

// ============================================================================
// 消息差异项视图
// ============================================================================

interface MessageDiffItemViewProps {
  diff: MessageDiffItem;
  viewMode: ViewMode;
}

function MessageDiffItemView({ diff, viewMode }: MessageDiffItemViewProps) {
  const [expanded, setExpanded] = useState(diff.diff_type !== "Unchanged");

  const leftMsg = diff.left_message as {
    role?: string;
    content?: string;
  } | null;
  const rightMsg = diff.right_message as {
    role?: string;
    content?: string;
  } | null;

  return (
    <div
      className={cn(
        "rounded border overflow-hidden",
        getDiffBorderColor(diff.diff_type),
      )}
    >
      {/* 头部 */}
      <div
        className={cn(
          "flex items-center gap-2 px-3 py-2 cursor-pointer",
          getDiffBgColor(diff.diff_type, true),
        )}
        onClick={() => setExpanded(!expanded)}
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
        <DiffTypeIcon type={diff.diff_type} />
        <span className="text-sm font-medium">消息 #{diff.index + 1}</span>
        {leftMsg?.role && (
          <span className="text-xs text-muted-foreground">
            ({leftMsg.role})
          </span>
        )}
        {!leftMsg?.role && rightMsg?.role && (
          <span className="text-xs text-muted-foreground">
            ({rightMsg.role})
          </span>
        )}
      </div>

      {/* 内容 */}
      {expanded && (
        <div
          className={
            viewMode === "side-by-side" ? "grid grid-cols-2 divide-x" : ""
          }
        >
          {viewMode === "side-by-side" ? (
            <>
              {/* 左侧消息 */}
              <div
                className={cn(
                  "p-3",
                  diff.diff_type === "Removed" || diff.diff_type === "Modified"
                    ? "bg-red-50/30 dark:bg-red-950/10"
                    : "bg-muted/30",
                )}
              >
                {leftMsg ? (
                  <div className="space-y-2">
                    <div className="text-xs font-medium text-muted-foreground">
                      {leftMsg.role}
                    </div>
                    <pre className="text-sm whitespace-pre-wrap break-words">
                      {typeof leftMsg.content === "string"
                        ? leftMsg.content
                        : JSON.stringify(leftMsg.content, null, 2)}
                    </pre>
                  </div>
                ) : (
                  <span className="text-xs text-muted-foreground italic">
                    (无)
                  </span>
                )}
              </div>

              {/* 右侧消息 */}
              <div
                className={cn(
                  "p-3",
                  diff.diff_type === "Added" || diff.diff_type === "Modified"
                    ? "bg-green-50/30 dark:bg-green-950/10"
                    : "bg-muted/30",
                )}
              >
                {rightMsg ? (
                  <div className="space-y-2">
                    <div className="text-xs font-medium text-muted-foreground">
                      {rightMsg.role}
                    </div>
                    <pre className="text-sm whitespace-pre-wrap break-words">
                      {typeof rightMsg.content === "string"
                        ? rightMsg.content
                        : JSON.stringify(rightMsg.content, null, 2)}
                    </pre>
                  </div>
                ) : (
                  <span className="text-xs text-muted-foreground italic">
                    (无)
                  </span>
                )}
              </div>
            </>
          ) : (
            <div className="space-y-0">
              {/* 删除的消息 */}
              {(diff.diff_type === "Removed" ||
                diff.diff_type === "Modified") &&
                leftMsg && (
                  <div className="flex bg-red-50/50 dark:bg-red-950/10">
                    <div className="w-8 flex items-start justify-center pt-3 text-red-600 border-r">
                      <Minus className="h-3 w-3" />
                    </div>
                    <div className="flex-1 p-3">
                      <div className="text-xs font-medium text-muted-foreground mb-1">
                        {leftMsg.role}
                      </div>
                      <pre className="text-sm whitespace-pre-wrap break-words">
                        {typeof leftMsg.content === "string"
                          ? leftMsg.content
                          : JSON.stringify(leftMsg.content, null, 2)}
                      </pre>
                    </div>
                  </div>
                )}

              {/* 新增的消息 */}
              {(diff.diff_type === "Added" || diff.diff_type === "Modified") &&
                rightMsg && (
                  <div className="flex bg-green-50/50 dark:bg-green-950/10">
                    <div className="w-8 flex items-start justify-center pt-3 text-green-600 border-r">
                      <Plus className="h-3 w-3" />
                    </div>
                    <div className="flex-1 p-3">
                      <div className="text-xs font-medium text-muted-foreground mb-1">
                        {rightMsg.role}
                      </div>
                      <pre className="text-sm whitespace-pre-wrap break-words">
                        {typeof rightMsg.content === "string"
                          ? rightMsg.content
                          : JSON.stringify(rightMsg.content, null, 2)}
                      </pre>
                    </div>
                  </div>
                )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 辅助组件和函数
// ============================================================================

/**
 * 差异类型图标
 */
function DiffTypeIcon({ type }: { type: DiffType }) {
  switch (type) {
    case "Added":
      return <Plus className="h-4 w-4 text-green-600" />;
    case "Removed":
      return <Minus className="h-4 w-4 text-red-600" />;
    case "Modified":
      return <Edit3 className="h-4 w-4 text-yellow-600" />;
    default:
      return null;
  }
}

/**
 * 获取差异背景颜色
 */
function getDiffBgColor(type: DiffType, isHeader: boolean = false): string {
  const opacity = isHeader ? "30" : "50";
  switch (type) {
    case "Added":
      return `bg-green-50/${opacity} dark:bg-green-950/10`;
    case "Removed":
      return `bg-red-50/${opacity} dark:bg-red-950/10`;
    case "Modified":
      return `bg-yellow-50/${opacity} dark:bg-yellow-950/10`;
    default:
      return "bg-muted/30";
  }
}

/**
 * 获取差异边框颜色
 */
function getDiffBorderColor(type: DiffType): string {
  switch (type) {
    case "Added":
      return "border-green-200 dark:border-green-800";
    case "Removed":
      return "border-red-200 dark:border-red-800";
    case "Modified":
      return "border-yellow-200 dark:border-yellow-800";
    default:
      return "";
  }
}

/**
 * 格式化值为字符串
 */
function formatValue(value: unknown): string {
  if (value === null) return "null";
  if (value === undefined) return "undefined";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value, null, 2);
}

// ============================================================================
// 对话框包装组件
// ============================================================================

interface FlowDiffDialogProps {
  /** 是否显示对话框 */
  open: boolean;
  /** 关闭对话框回调 */
  onClose: () => void;
  /** 左侧 Flow ID */
  leftFlowId: string;
  /** 右侧 Flow ID */
  rightFlowId: string;
}

export function FlowDiffDialog({
  open,
  onClose,
  leftFlowId,
  rightFlowId,
}: FlowDiffDialogProps) {
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* 背景遮罩 */}
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />

      {/* 对话框 */}
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-4xl mx-4 max-h-[90vh] overflow-hidden">
        <FlowDiffView
          leftFlowId={leftFlowId}
          rightFlowId={rightFlowId}
          onClose={onClose}
          className="h-[80vh]"
        />
      </div>
    </div>
  );
}

export default FlowDiffView;
