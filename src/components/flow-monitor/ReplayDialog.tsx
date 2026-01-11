/**
 * 重放对话框组件
 *
 * 实现重放配置对话框和批量重放进度显示
 * **Validates: Requirements 3.1, 3.3, 3.4, 3.6**
 */

import { useState, useCallback, useEffect } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  X,
  Play,
  Loader2,
  Check,
  AlertCircle,
  Settings,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  Clock,
  CheckCircle2,
  XCircle,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { LLMFlow, Message } from "@/lib/api/flowMonitor";

// ============================================================================
// 类型定义
// ============================================================================

/**
 * 请求修改配置
 */
export interface RequestModification {
  model?: string;
  messages?: Message[];
  parameters?: {
    temperature?: number;
    top_p?: number;
    max_tokens?: number;
    stream?: boolean;
  };
  system_prompt?: string;
}

/**
 * 重放配置
 */
export interface ReplayConfig {
  credential_id?: string;
  modify_request?: RequestModification;
  interval_ms: number;
}

/**
 * 重放结果
 */
export interface ReplayResult {
  original_flow_id: string;
  replay_flow_id: string;
  success: boolean;
  error?: string;
  started_at: string;
  completed_at: string;
  duration_ms: number;
}

/**
 * 批量重放结果
 */
export interface BatchReplayResult {
  total: number;
  success_count: number;
  failure_count: number;
  results: ReplayResult[];
  started_at: string;
  completed_at: string;
  total_duration_ms: number;
}

// ============================================================================
// 组件属性
// ============================================================================

interface ReplayDialogProps {
  /** 是否显示对话框 */
  open: boolean;
  /** 关闭对话框回调 */
  onClose: () => void;
  /** 要重放的 Flow（单个重放） */
  flow?: LLMFlow;
  /** 要重放的 Flow ID 列表（批量重放） */
  flowIds?: string[];
  /** 重放成功回调 */
  onReplaySuccess?: (result: ReplayResult | BatchReplayResult) => void;
  /** 跳转到 Flow 详情 */
  onNavigateToFlow?: (flowId: string) => void;
}

// ============================================================================
// 主组件
// ============================================================================

export function ReplayDialog({
  open,
  onClose,
  flow,
  flowIds,
  onReplaySuccess,
  onNavigateToFlow,
}: ReplayDialogProps) {
  // 状态
  const [config, setConfig] = useState<ReplayConfig>({
    interval_ms: 1000,
  });
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [replaying, setReplaying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ReplayResult | BatchReplayResult | null>(
    null,
  );
  const [progress, setProgress] = useState<{
    current: number;
    total: number;
  } | null>(null);

  // 修改请求的状态
  const [modifyModel, setModifyModel] = useState(false);
  const [newModel, setNewModel] = useState("");
  const [modifyTemperature, setModifyTemperature] = useState(false);
  const [newTemperature, setNewTemperature] = useState(0.7);
  const [modifyMaxTokens, setModifyMaxTokens] = useState(false);
  const [newMaxTokens, setNewMaxTokens] = useState(4096);

  // 判断是单个重放还是批量重放
  const isBatchReplay = flowIds && flowIds.length > 1;
  const replayCount = flowIds?.length || (flow ? 1 : 0);

  // 重置状态
  useEffect(() => {
    if (open) {
      setError(null);
      setResult(null);
      setProgress(null);
      setReplaying(false);
      // 初始化修改值
      if (flow) {
        setNewModel(flow.request.model);
        setNewTemperature(flow.request.parameters.temperature ?? 0.7);
        setNewMaxTokens(flow.request.parameters.max_tokens ?? 4096);
      }
    }
  }, [open, flow]);

  // 构建重放配置
  const buildConfig = useCallback((): ReplayConfig => {
    const replayConfig: ReplayConfig = {
      interval_ms: config.interval_ms,
    };

    // 构建请求修改
    const modifications: RequestModification = {};
    let hasModifications = false;

    if (modifyModel && newModel) {
      modifications.model = newModel;
      hasModifications = true;
    }

    if (modifyTemperature || modifyMaxTokens) {
      modifications.parameters = {};
      if (modifyTemperature) {
        modifications.parameters.temperature = newTemperature;
      }
      if (modifyMaxTokens) {
        modifications.parameters.max_tokens = newMaxTokens;
      }
      hasModifications = true;
    }

    if (hasModifications) {
      replayConfig.modify_request = modifications;
    }

    return replayConfig;
  }, [
    config.interval_ms,
    modifyModel,
    newModel,
    modifyTemperature,
    newTemperature,
    modifyMaxTokens,
    newMaxTokens,
  ]);

  // 执行重放
  const handleReplay = useCallback(async () => {
    setReplaying(true);
    setError(null);
    setResult(null);

    try {
      const replayConfig = buildConfig();

      if (isBatchReplay && flowIds) {
        // 批量重放
        setProgress({ current: 0, total: flowIds.length });
        const batchResult = await safeInvoke<BatchReplayResult>(
          "replay_flows_batch",
          {
            request: {
              flow_ids: flowIds,
              config: replayConfig,
            },
          },
        );
        setResult(batchResult);
        onReplaySuccess?.(batchResult);
      } else {
        // 单个重放
        const flowId = flow?.id || flowIds?.[0];
        if (!flowId) {
          throw new Error("没有指定要重放的 Flow");
        }

        const singleResult = await safeInvoke<ReplayResult>("replay_flow", {
          request: {
            flow_id: flowId,
            config: replayConfig,
          },
        });
        setResult(singleResult);
        onReplaySuccess?.(singleResult);
      }
    } catch (e) {
      console.error("重放失败:", e);
      setError(e instanceof Error ? e.message : "重放失败");
    } finally {
      setReplaying(false);
      setProgress(null);
    }
  }, [buildConfig, isBatchReplay, flowIds, flow, onReplaySuccess]);

  // 关闭对话框
  const handleClose = () => {
    if (!replaying) {
      onClose();
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* 背景遮罩 */}
      <div className="absolute inset-0 bg-black/50" onClick={handleClose} />

      {/* 对话框 */}
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-hidden flex flex-col">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <div className="flex items-center gap-2">
            <RefreshCw className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">
              {isBatchReplay ? "批量重放 Flow" : "重放 Flow"}
            </h2>
          </div>
          <button
            onClick={handleClose}
            className="p-1 rounded hover:bg-muted"
            disabled={replaying}
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* 内容 */}
        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-6">
          {/* 重放数量提示 */}
          <div className="rounded-lg bg-muted/50 px-4 py-3">
            <div className="text-sm">
              {isBatchReplay ? (
                <span>
                  将重放 <strong>{replayCount}</strong> 个 Flow
                </span>
              ) : flow ? (
                <div className="space-y-1">
                  <div>
                    模型: <strong>{flow.request.model}</strong>
                  </div>
                  <div className="text-xs text-muted-foreground font-mono">
                    ID: {flow.id.slice(0, 12)}...
                  </div>
                </div>
              ) : (
                <span>将重放选中的 Flow</span>
              )}
            </div>
          </div>

          {/* 结果显示 */}
          {result && (
            <ReplayResultDisplay
              result={result}
              isBatch={isBatchReplay || false}
              onNavigateToFlow={onNavigateToFlow}
            />
          )}

          {/* 进度显示 */}
          {progress && (
            <div className="space-y-2">
              <div className="flex items-center justify-between text-sm">
                <span>重放进度</span>
                <span>
                  {progress.current} / {progress.total}
                </span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-300"
                  style={{
                    width: `${(progress.current / progress.total) * 100}%`,
                  }}
                />
              </div>
            </div>
          )}

          {/* 修改请求选项（仅单个重放时显示） */}
          {!result && !isBatchReplay && flow && (
            <div className="space-y-4">
              <div className="flex items-center gap-2 text-sm font-medium text-muted-foreground">
                <Settings className="h-4 w-4" />
                修改请求参数（可选）
              </div>

              {/* 修改模型 */}
              <div className="space-y-2">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={modifyModel}
                    onChange={(e) => setModifyModel(e.target.checked)}
                    className="rounded border-gray-300"
                  />
                  <span className="text-sm">修改模型</span>
                </label>
                {modifyModel && (
                  <input
                    type="text"
                    value={newModel}
                    onChange={(e) => setNewModel(e.target.value)}
                    placeholder="输入新的模型名称"
                    className="w-full rounded border bg-background px-3 py-2 text-sm"
                  />
                )}
              </div>

              {/* 修改 Temperature */}
              <div className="space-y-2">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={modifyTemperature}
                    onChange={(e) => setModifyTemperature(e.target.checked)}
                    className="rounded border-gray-300"
                  />
                  <span className="text-sm">修改 Temperature</span>
                </label>
                {modifyTemperature && (
                  <div className="flex items-center gap-2">
                    <input
                      type="range"
                      min="0"
                      max="2"
                      step="0.1"
                      value={newTemperature}
                      onChange={(e) =>
                        setNewTemperature(parseFloat(e.target.value))
                      }
                      className="flex-1"
                    />
                    <span className="text-sm w-12 text-right">
                      {newTemperature}
                    </span>
                  </div>
                )}
              </div>

              {/* 修改 Max Tokens */}
              <div className="space-y-2">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={modifyMaxTokens}
                    onChange={(e) => setModifyMaxTokens(e.target.checked)}
                    className="rounded border-gray-300"
                  />
                  <span className="text-sm">修改 Max Tokens</span>
                </label>
                {modifyMaxTokens && (
                  <input
                    type="number"
                    value={newMaxTokens}
                    onChange={(e) =>
                      setNewMaxTokens(parseInt(e.target.value) || 4096)
                    }
                    min={1}
                    max={128000}
                    className="w-full rounded border bg-background px-3 py-2 text-sm"
                  />
                )}
              </div>
            </div>
          )}

          {/* 高级选项 */}
          {!result && (
            <div className="space-y-3">
              <button
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
              >
                <Settings className="h-4 w-4" />
                高级选项
                {showAdvanced ? (
                  <ChevronUp className="h-4 w-4" />
                ) : (
                  <ChevronDown className="h-4 w-4" />
                )}
              </button>

              {showAdvanced && (
                <div className="rounded-lg border bg-muted/30 p-4 space-y-4">
                  {/* 重放间隔 */}
                  {isBatchReplay && (
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        重放间隔（毫秒）
                      </label>
                      <div className="flex items-center gap-2">
                        <input
                          type="number"
                          value={config.interval_ms}
                          onChange={(e) =>
                            setConfig({
                              ...config,
                              interval_ms: parseInt(e.target.value) || 1000,
                            })
                          }
                          min={100}
                          max={60000}
                          className="w-32 rounded border bg-background px-3 py-2 text-sm"
                        />
                        <span className="text-xs text-muted-foreground">
                          避免触发速率限制
                        </span>
                      </div>
                    </div>
                  )}

                  <div className="text-xs text-muted-foreground">
                    <p>• 重放会创建新的 Flow 并标记为 "replay"</p>
                    <p>• 重放完成后可以对比原始 Flow 和重放 Flow</p>
                    {isBatchReplay && (
                      <p>• 批量重放会按顺序执行，每个请求之间有间隔</p>
                    )}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* 错误提示 */}
          {error && (
            <div className="rounded-lg border border-red-200 bg-red-50 dark:bg-red-950/20 px-4 py-3">
              <div className="flex items-center gap-2 text-red-600 dark:text-red-400">
                <AlertCircle className="h-4 w-4" />
                <span className="text-sm">{error}</span>
              </div>
            </div>
          )}
        </div>

        {/* 底部按钮 */}
        <div className="flex items-center justify-end gap-3 px-6 py-4 border-t bg-muted/30">
          <button
            onClick={handleClose}
            disabled={replaying}
            className="px-4 py-2 text-sm rounded-lg border hover:bg-muted disabled:opacity-50"
          >
            {result ? "关闭" : "取消"}
          </button>
          {!result && (
            <button
              onClick={handleReplay}
              disabled={replaying || replayCount === 0}
              className={cn(
                "flex items-center gap-2 px-4 py-2 text-sm rounded-lg",
                "bg-primary text-primary-foreground hover:bg-primary/90",
                "disabled:opacity-50 disabled:cursor-not-allowed",
              )}
            >
              {replaying ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  重放中...
                </>
              ) : (
                <>
                  <Play className="h-4 w-4" />
                  {isBatchReplay ? `重放 ${replayCount} 个` : "重放"}
                </>
              )}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 重放结果显示组件
// ============================================================================

interface ReplayResultDisplayProps {
  result: ReplayResult | BatchReplayResult;
  isBatch: boolean;
  onNavigateToFlow?: (flowId: string) => void;
}

function ReplayResultDisplay({
  result,
  isBatch,
  onNavigateToFlow,
}: ReplayResultDisplayProps) {
  if (isBatch) {
    const batchResult = result as BatchReplayResult;
    return (
      <div className="space-y-4">
        {/* 批量结果摘要 */}
        <div className="rounded-lg border bg-muted/30 p-4">
          <div className="grid grid-cols-3 gap-4 text-center">
            <div>
              <div className="text-2xl font-bold">{batchResult.total}</div>
              <div className="text-xs text-muted-foreground">总数</div>
            </div>
            <div>
              <div className="text-2xl font-bold text-green-600">
                {batchResult.success_count}
              </div>
              <div className="text-xs text-muted-foreground">成功</div>
            </div>
            <div>
              <div className="text-2xl font-bold text-red-600">
                {batchResult.failure_count}
              </div>
              <div className="text-xs text-muted-foreground">失败</div>
            </div>
          </div>
          <div className="mt-3 pt-3 border-t text-center text-sm text-muted-foreground">
            <Clock className="h-4 w-4 inline mr-1" />
            总耗时: {(batchResult.total_duration_ms / 1000).toFixed(2)}s
          </div>
        </div>

        {/* 详细结果列表 */}
        <div className="space-y-2 max-h-60 overflow-y-auto">
          {batchResult.results.map((r, idx) => (
            <ReplayResultItem
              key={idx}
              result={r}
              onNavigate={onNavigateToFlow}
            />
          ))}
        </div>
      </div>
    );
  }

  // 单个结果
  const singleResult = result as ReplayResult;
  return (
    <div
      className={cn(
        "rounded-lg border p-4",
        singleResult.success
          ? "border-green-200 bg-green-50 dark:bg-green-950/20"
          : "border-red-200 bg-red-50 dark:bg-red-950/20",
      )}
    >
      <div className="flex items-center gap-2">
        {singleResult.success ? (
          <CheckCircle2 className="h-5 w-5 text-green-600" />
        ) : (
          <XCircle className="h-5 w-5 text-red-600" />
        )}
        <span
          className={cn(
            "font-medium",
            singleResult.success ? "text-green-600" : "text-red-600",
          )}
        >
          {singleResult.success ? "重放成功" : "重放失败"}
        </span>
      </div>

      {singleResult.success && singleResult.replay_flow_id && (
        <div className="mt-3 space-y-2">
          <div className="text-sm">
            <span className="text-muted-foreground">新 Flow ID: </span>
            <span className="font-mono text-xs">
              {singleResult.replay_flow_id.slice(0, 16)}...
            </span>
          </div>
          <div className="text-sm text-muted-foreground">
            <Clock className="h-4 w-4 inline mr-1" />
            耗时: {singleResult.duration_ms}ms
          </div>
          {onNavigateToFlow && (
            <button
              onClick={() => onNavigateToFlow(singleResult.replay_flow_id)}
              className="text-sm text-primary hover:underline"
            >
              查看重放结果 →
            </button>
          )}
        </div>
      )}

      {singleResult.error && (
        <div className="mt-2 text-sm text-red-600 dark:text-red-400">
          {singleResult.error}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 单个重放结果项
// ============================================================================

interface ReplayResultItemProps {
  result: ReplayResult;
  onNavigate?: (flowId: string) => void;
}

function ReplayResultItem({ result, onNavigate }: ReplayResultItemProps) {
  return (
    <div
      className={cn(
        "flex items-center justify-between p-3 rounded-lg border",
        result.success
          ? "border-green-200 bg-green-50/50 dark:bg-green-950/10"
          : "border-red-200 bg-red-50/50 dark:bg-red-950/10",
      )}
    >
      <div className="flex items-center gap-2">
        {result.success ? (
          <Check className="h-4 w-4 text-green-600" />
        ) : (
          <XCircle className="h-4 w-4 text-red-600" />
        )}
        <span className="text-sm font-mono">
          {result.original_flow_id.slice(0, 8)}...
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">
          {result.duration_ms}ms
        </span>
        {result.success && result.replay_flow_id && onNavigate && (
          <button
            onClick={() => onNavigate(result.replay_flow_id)}
            className="text-xs text-primary hover:underline"
          >
            查看
          </button>
        )}
      </div>
    </div>
  );
}

export default ReplayDialog;
