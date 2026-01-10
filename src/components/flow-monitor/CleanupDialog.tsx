/**
 * 清理日志对话框
 * 提供多种清理选项：删除所有、按时间、按数量、按状态、按Provider、按大小
 */

import { useState } from "react";
import {
  Trash2,
  AlertTriangle,
  Clock,
  Hash,
  Activity,
  Server,
  HardDrive,
} from "lucide-react";
import { Modal } from "@/components/Modal";
import {
  flowMonitorApi,
  type CleanupFlowsRequest,
  type CleanupType,
  type ProviderType,
} from "@/lib/api/flowMonitor";

interface CleanupDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

export function CleanupDialog({
  isOpen,
  onClose,
  onSuccess,
}: CleanupDialogProps) {
  const [cleanupType, setCleanupType] = useState<CleanupType>("ByTime");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showSuccess, setShowSuccess] = useState(false);
  const [cleanupResult, setCleanupResult] = useState<{
    cleaned_count: number;
    cleaned_files: number;
    freed_bytes: number;
  } | null>(null);

  // 时间清理选项
  const [retentionDays, setRetentionDays] = useState(7);
  const [retentionHours, setRetentionHours] = useState(24);
  const [useHours, setUseHours] = useState(false);

  // 数量清理选项
  const [maxRecords, setMaxRecords] = useState(1000);

  // 状态清理选项
  const [targetStates, setTargetStates] = useState<string[]>(["Failed"]);

  // Provider清理选项
  const [targetProviders, setTargetProviders] = useState<string[]>([]);

  // 大小清理选项
  const [maxStorageGB, setMaxStorageGB] = useState(1);

  const handleCleanup = async () => {
    setLoading(true);
    setError(null);

    try {
      const request: CleanupFlowsRequest = {
        cleanup_type: cleanupType,
      };

      // 根据清理类型设置相应参数
      switch (cleanupType) {
        case "All":
          // 删除所有，无需额外参数
          break;

        case "ByTime":
          if (useHours) {
            request.retention_hours = retentionHours;
          } else {
            request.retention_days = retentionDays;
          }
          break;

        case "ByCount":
          request.max_records = maxRecords;
          break;

        case "ByStatus":
          if (targetStates.length === 0) {
            setError("请至少选择一个状态");
            setLoading(false);
            return;
          }
          request.target_states = targetStates;
          break;

        case "ByProvider":
          if (targetProviders.length === 0) {
            setError("请至少选择一个Provider");
            setLoading(false);
            return;
          }
          request.target_providers = targetProviders;
          break;

        case "BySize":
          request.max_storage_bytes = maxStorageGB * 1024 * 1024 * 1024; // GB转字节
          break;
      }

      const result = await flowMonitorApi.cleanupFlows(request);

      // 保存清理结果并显示成功提示
      setCleanupResult(result);
      setShowSuccess(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleSuccessClose = () => {
    setShowSuccess(false);
    setCleanupResult(null);
    onSuccess();
    onClose();
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  const availableStates = [
    "Pending",
    "Streaming",
    "Completed",
    "Failed",
    "Cancelled",
  ];
  const availableProviders: ProviderType[] = [
    "Kiro",
    "Gemini",
    "Qwen",
    "Antigravity",
    "OpenAI",
    "Claude",
    "Vertex",
    "GeminiApiKey",
    "Codex",
    "ClaudeOAuth",
    "IFlow",
  ];

  // 如果显示成功提示，渲染成功对话框
  if (showSuccess && cleanupResult) {
    return (
      <Modal isOpen={isOpen} onClose={handleSuccessClose} maxWidth="max-w-md">
        <div className="px-6 py-8">
          {/* 成功图标 */}
          <div className="flex justify-center mb-6">
            <div className="relative">
              <div className="w-20 h-20 bg-green-100 dark:bg-green-900/30 rounded-full flex items-center justify-center">
                <svg
                  className="w-10 h-10 text-green-600 dark:text-green-400"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M5 13l4 4L19 7"
                  />
                </svg>
              </div>
              {/* 动画圆环 */}
              <div className="absolute inset-0 w-20 h-20 border-4 border-green-200 dark:border-green-800 rounded-full animate-ping opacity-75"></div>
            </div>
          </div>

          {/* 标题 */}
          <h3 className="text-2xl font-bold text-center mb-2">清理完成！</h3>
          <p className="text-center text-muted-foreground mb-6">
            已成功清理日志数据
          </p>

          {/* 清理统计 */}
          <div className="space-y-3 mb-6">
            <div className="flex items-center justify-between p-4 bg-muted/50 rounded-lg">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 bg-blue-100 dark:bg-blue-900/30 rounded-lg flex items-center justify-center">
                  <Hash className="w-5 h-5 text-blue-600 dark:text-blue-400" />
                </div>
                <span className="text-sm text-muted-foreground">删除记录</span>
              </div>
              <span className="text-lg font-semibold">
                {cleanupResult.cleaned_count} 条
              </span>
            </div>

            <div className="flex items-center justify-between p-4 bg-muted/50 rounded-lg">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 bg-purple-100 dark:bg-purple-900/30 rounded-lg flex items-center justify-center">
                  <svg
                    className="w-5 h-5 text-purple-600 dark:text-purple-400"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                    />
                  </svg>
                </div>
                <span className="text-sm text-muted-foreground">清理文件</span>
              </div>
              <span className="text-lg font-semibold">
                {cleanupResult.cleaned_files} 个
              </span>
            </div>

            <div className="flex items-center justify-between p-4 bg-gradient-to-r from-green-50 to-emerald-50 dark:from-green-900/20 dark:to-emerald-900/20 rounded-lg border border-green-200 dark:border-green-800">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 bg-green-100 dark:bg-green-900/50 rounded-lg flex items-center justify-center">
                  <HardDrive className="w-5 h-5 text-green-600 dark:text-green-400" />
                </div>
                <span className="text-sm font-medium text-green-900 dark:text-green-100">
                  释放空间
                </span>
              </div>
              <span className="text-lg font-bold text-green-600 dark:text-green-400">
                {formatBytes(cleanupResult.freed_bytes)}
              </span>
            </div>
          </div>

          {/* 确认按钮 */}
          <button
            onClick={handleSuccessClose}
            className="w-full py-3 bg-primary hover:bg-primary/90 text-white font-medium rounded-lg transition-colors"
          >
            完成
          </button>
        </div>
      </Modal>
    );
  }

  return (
    <Modal isOpen={isOpen} onClose={onClose} maxWidth="max-w-2xl">
      {/* 标题 */}
      <div className="flex items-center justify-between border-b px-6 py-4">
        <h3 className="text-lg font-semibold">清理日志</h3>
      </div>

      <div className="space-y-6 px-6 py-4">
        {/* 警告提示 */}
        <div className="flex items-start gap-3 p-4 bg-yellow-50 border border-yellow-200 rounded-lg dark:bg-yellow-950/20 dark:border-yellow-800">
          <AlertTriangle className="h-5 w-5 text-yellow-600 dark:text-yellow-400 shrink-0 mt-0.5" />
          <div className="text-sm">
            <p className="font-medium text-yellow-800 dark:text-yellow-200">
              注意：清理操作不可撤销
            </p>
            <p className="text-yellow-700 dark:text-yellow-300 mt-1">
              请确认清理条件，删除的日志数据无法恢复。
            </p>
          </div>
        </div>

        {/* 清理类型选择 */}
        <div>
          <label className="block text-sm font-medium mb-3">清理类型</label>
          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={() => setCleanupType("All")}
              className={`flex items-center gap-3 p-3 rounded-lg border text-left transition-all ${
                cleanupType === "All"
                  ? "border-red-500 bg-red-50 text-red-700 dark:bg-red-950/20 dark:text-red-300"
                  : "border-border hover:border-red-300 hover:bg-red-50/50"
              }`}
            >
              <Trash2 className="h-5 w-5 text-red-500" />
              <div>
                <div className="font-medium">删除所有</div>
                <div className="text-xs text-muted-foreground">
                  清空所有日志数据
                </div>
              </div>
            </button>

            <button
              onClick={() => setCleanupType("ByTime")}
              className={`flex items-center gap-3 p-3 rounded-lg border text-left transition-all ${
                cleanupType === "ByTime"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border hover:border-primary/50 hover:bg-primary/5"
              }`}
            >
              <Clock className="h-5 w-5" />
              <div>
                <div className="font-medium">按时间</div>
                <div className="text-xs text-muted-foreground">
                  保留最近的数据
                </div>
              </div>
            </button>

            <button
              onClick={() => setCleanupType("ByCount")}
              className={`flex items-center gap-3 p-3 rounded-lg border text-left transition-all ${
                cleanupType === "ByCount"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border hover:border-primary/50 hover:bg-primary/5"
              }`}
            >
              <Hash className="h-5 w-5" />
              <div>
                <div className="font-medium">按数量</div>
                <div className="text-xs text-muted-foreground">
                  只保留最近N条
                </div>
              </div>
            </button>

            <button
              onClick={() => setCleanupType("ByStatus")}
              className={`flex items-center gap-3 p-3 rounded-lg border text-left transition-all ${
                cleanupType === "ByStatus"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border hover:border-primary/50 hover:bg-primary/5"
              }`}
            >
              <Activity className="h-5 w-5" />
              <div>
                <div className="font-medium">按状态</div>
                <div className="text-xs text-muted-foreground">
                  删除特定状态
                </div>
              </div>
            </button>

            <button
              onClick={() => setCleanupType("ByProvider")}
              className={`flex items-center gap-3 p-3 rounded-lg border text-left transition-all ${
                cleanupType === "ByProvider"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border hover:border-primary/50 hover:bg-primary/5"
              }`}
            >
              <Server className="h-5 w-5" />
              <div>
                <div className="font-medium">按Provider</div>
                <div className="text-xs text-muted-foreground">
                  删除特定Provider
                </div>
              </div>
            </button>

            <button
              onClick={() => setCleanupType("BySize")}
              className={`flex items-center gap-3 p-3 rounded-lg border text-left transition-all ${
                cleanupType === "BySize"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border hover:border-primary/50 hover:bg-primary/5"
              }`}
            >
              <HardDrive className="h-5 w-5" />
              <div>
                <div className="font-medium">按大小</div>
                <div className="text-xs text-muted-foreground">
                  限制存储大小
                </div>
              </div>
            </button>
          </div>
        </div>

        {/* 清理选项配置 */}
        <div className="space-y-4">
          {cleanupType === "ByTime" && (
            <div className="space-y-4">
              <div className="flex items-center gap-4">
                <label className="flex items-center gap-2">
                  <input
                    type="radio"
                    checked={!useHours}
                    onChange={() => setUseHours(false)}
                    className="text-primary"
                  />
                  <span className="text-sm">按天数</span>
                </label>
                <label className="flex items-center gap-2">
                  <input
                    type="radio"
                    checked={useHours}
                    onChange={() => setUseHours(true)}
                    className="text-primary"
                  />
                  <span className="text-sm">按小时</span>
                </label>
              </div>

              {useHours ? (
                <div>
                  <label className="block text-sm font-medium mb-2">
                    保留最近 {retentionHours} 小时的数据
                  </label>
                  <input
                    type="range"
                    min="1"
                    max="168"
                    value={retentionHours}
                    onChange={(e) => setRetentionHours(Number(e.target.value))}
                    className="w-full"
                  />
                  <div className="flex justify-between text-xs text-muted-foreground mt-1">
                    <span>1小时</span>
                    <span>7天</span>
                  </div>
                </div>
              ) : (
                <div>
                  <label className="block text-sm font-medium mb-2">
                    保留最近 {retentionDays} 天的数据
                  </label>
                  <input
                    type="range"
                    min="1"
                    max="365"
                    value={retentionDays}
                    onChange={(e) => setRetentionDays(Number(e.target.value))}
                    className="w-full"
                  />
                  <div className="flex justify-between text-xs text-muted-foreground mt-1">
                    <span>1天</span>
                    <span>1年</span>
                  </div>
                </div>
              )}
            </div>
          )}

          {cleanupType === "ByCount" && (
            <div>
              <label className="block text-sm font-medium mb-2">
                只保留最近 {maxRecords} 条记录
              </label>
              <input
                type="range"
                min="100"
                max="10000"
                step="100"
                value={maxRecords}
                onChange={(e) => setMaxRecords(Number(e.target.value))}
                className="w-full"
              />
              <div className="flex justify-between text-xs text-muted-foreground mt-1">
                <span>100条</span>
                <span>10,000条</span>
              </div>
            </div>
          )}

          {cleanupType === "ByStatus" && (
            <div>
              <label className="block text-sm font-medium mb-2">
                选择要删除的状态
              </label>
              <div className="grid grid-cols-2 gap-2">
                {availableStates.map((state) => (
                  <label key={state} className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={targetStates.includes(state)}
                      onChange={(e) => {
                        if (e.target.checked) {
                          setTargetStates([...targetStates, state]);
                        } else {
                          setTargetStates(
                            targetStates.filter((s) => s !== state),
                          );
                        }
                      }}
                      className="text-primary"
                    />
                    <span className="text-sm">{state}</span>
                  </label>
                ))}
              </div>
            </div>
          )}

          {cleanupType === "ByProvider" && (
            <div>
              <label className="block text-sm font-medium mb-2">
                选择要删除的Provider
              </label>
              <div className="grid grid-cols-2 gap-2 max-h-48 overflow-y-auto">
                {availableProviders.map((provider) => (
                  <label key={provider} className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={targetProviders.includes(provider)}
                      onChange={(e) => {
                        if (e.target.checked) {
                          setTargetProviders([...targetProviders, provider]);
                        } else {
                          setTargetProviders(
                            targetProviders.filter((p) => p !== provider),
                          );
                        }
                      }}
                      className="text-primary"
                    />
                    <span className="text-sm">{provider}</span>
                  </label>
                ))}
              </div>
            </div>
          )}

          {cleanupType === "BySize" && (
            <div>
              <label className="block text-sm font-medium mb-2">
                存储大小限制：{maxStorageGB} GB
              </label>
              <input
                type="range"
                min="0.1"
                max="100"
                step="0.1"
                value={maxStorageGB}
                onChange={(e) => setMaxStorageGB(Number(e.target.value))}
                className="w-full"
              />
              <div className="flex justify-between text-xs text-muted-foreground mt-1">
                <span>100MB</span>
                <span>100GB</span>
              </div>
              <p className="text-xs text-muted-foreground mt-2">
                当存储超过此大小时，将删除最旧的数据
              </p>
            </div>
          )}
        </div>

        {/* 错误显示 */}
        {error && (
          <div className="p-3 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm dark:bg-red-950/20 dark:border-red-800 dark:text-red-300">
            {error}
          </div>
        )}

        {/* 操作按钮 */}
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button
            onClick={onClose}
            disabled={loading}
            className="px-4 py-2 text-sm border rounded-lg hover:bg-muted disabled:opacity-50"
          >
            取消
          </button>
          <button
            onClick={handleCleanup}
            disabled={loading}
            className={`px-4 py-2 text-sm rounded-lg text-white disabled:opacity-50 ${
              cleanupType === "All"
                ? "bg-red-600 hover:bg-red-700"
                : "bg-primary hover:bg-primary/90"
            }`}
          >
            {loading
              ? "清理中..."
              : cleanupType === "All"
                ? "删除所有"
                : "开始清理"}
          </button>
        </div>
      </div>
    </Modal>
  );
}
