/**
 * 批量操作组件
 * 实现批量选择、批量操作菜单、操作进度显示
 * **Validates: Requirements 11.1-11.7**
 */

import React, { useState, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  CheckSquare,
  Square,
  Star,
  StarOff,
  Tag,
  Download,
  Trash2,
  FolderPlus,
  X,
  Loader2,
  AlertCircle,
  Check,
  ChevronDown,
  MinusSquare,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { ExportFormat, LLMFlow } from "@/lib/api/flowMonitor";

export interface BatchResult {
  total: number;
  success: number;
  failed: number;
  errors: [string, string][];
  export_data?: string;
}

export interface SessionInfo {
  id: string;
  name: string;
}

export type BatchOperationType =
  | "star"
  | "unstar"
  | "addTags"
  | "removeTags"
  | "export"
  | "delete"
  | "addToSession";

interface BatchOperationsProps {
  flows: LLMFlow[];
  selectedIds: Set<string>;
  onSelectionChange: (selectedIds: Set<string>) => void;
  onOperationComplete?: (
    result: BatchResult,
    operation: BatchOperationType,
  ) => void;
  sessions?: SessionInfo[];
  availableTags?: string[];
  onRefresh?: () => void;
  className?: string;
}

export function BatchOperations({
  flows,
  selectedIds,
  onSelectionChange,
  onOperationComplete,
  sessions = [],
  availableTags = [],
  onRefresh,
  className,
}: BatchOperationsProps) {
  const [showMenu, setShowMenu] = useState(false);
  const [operating, setOperating] = useState(false);
  const [currentOp, setCurrentOp] = useState<BatchOperationType | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showTagDialog, setShowTagDialog] = useState(false);
  const [tagMode, setTagMode] = useState<"add" | "remove">("add");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [newTag, setNewTag] = useState("");
  const [showSessionDialog, setShowSessionDialog] = useState(false);
  const [selectedSessionId, setSelectedSessionId] = useState("");
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [exportFormat, setExportFormat] = useState<ExportFormat>("json");
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const selectedCount = selectedIds.size;
  const totalCount = flows.length;
  const allSelected = totalCount > 0 && selectedCount === totalCount;
  const someSelected = selectedCount > 0 && selectedCount < totalCount;

  const handleSelectAll = useCallback(() => {
    onSelectionChange(
      allSelected ? new Set() : new Set(flows.map((f) => f.id)),
    );
  }, [allSelected, flows, onSelectionChange]);

  const handleClearSelection = useCallback(() => {
    onSelectionChange(new Set());
    setShowMenu(false);
  }, [onSelectionChange]);

  const runBatchOp = useCallback(
    async (
      op: BatchOperationType,
      command: string,
      request: Record<string, unknown>,
      onSuccess?: () => void,
    ) => {
      if (selectedCount === 0) return;
      try {
        setOperating(true);
        setCurrentOp(op);
        setError(null);
        const result = await safeInvoke<BatchResult>(command, { request });
        onOperationComplete?.(result, op);
        onSuccess?.();
        onRefresh?.();
        setShowMenu(false);
      } catch (e) {
        setError(e instanceof Error ? e.message : `批量${op}失败`);
      } finally {
        setOperating(false);
        setCurrentOp(null);
      }
    },
    [selectedCount, onOperationComplete, onRefresh],
  );

  const handleBatchStar = () =>
    runBatchOp("star", "batch_star_flows", {
      flow_ids: Array.from(selectedIds),
    });

  const handleBatchUnstar = () =>
    runBatchOp("unstar", "batch_unstar_flows", {
      flow_ids: Array.from(selectedIds),
    });

  const handleBatchAddTags = () => {
    if (selectedTags.length === 0) return;
    runBatchOp(
      "addTags",
      "batch_add_tags",
      { flow_ids: Array.from(selectedIds), tags: selectedTags },
      () => {
        setShowTagDialog(false);
        setSelectedTags([]);
      },
    );
  };

  const handleBatchRemoveTags = () => {
    if (selectedTags.length === 0) return;
    runBatchOp(
      "removeTags",
      "batch_remove_tags",
      { flow_ids: Array.from(selectedIds), tags: selectedTags },
      () => {
        setShowTagDialog(false);
        setSelectedTags([]);
      },
    );
  };

  const handleBatchExport = async () => {
    if (selectedCount === 0) return;
    try {
      setOperating(true);
      setCurrentOp("export");
      setError(null);
      const result = await safeInvoke<BatchResult>("batch_export_flows", {
        request: { flow_ids: Array.from(selectedIds), format: exportFormat },
      });
      if (result.export_data) {
        const blob = new Blob([result.export_data], {
          type: "application/json",
        });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `flows_${new Date().toISOString().slice(0, 10)}.${exportFormat}`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      }
      onOperationComplete?.(result, "export");
      setShowExportDialog(false);
      setShowMenu(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "批量导出失败");
    } finally {
      setOperating(false);
      setCurrentOp(null);
    }
  };

  const handleBatchDelete = () =>
    runBatchOp(
      "delete",
      "batch_delete_flows",
      { flow_ids: Array.from(selectedIds) },
      () => {
        handleClearSelection();
        setShowDeleteConfirm(false);
      },
    );

  const handleBatchAddToSession = () => {
    if (!selectedSessionId) return;
    runBatchOp(
      "addToSession",
      "batch_add_to_session",
      { flow_ids: Array.from(selectedIds), session_id: selectedSessionId },
      () => {
        setShowSessionDialog(false);
        setSelectedSessionId("");
      },
    );
  };

  const toggleTag = (tag: string) => {
    setSelectedTags((prev) =>
      prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag],
    );
  };

  const handleAddNewTag = () => {
    const t = newTag.trim();
    if (t && !selectedTags.includes(t)) {
      setSelectedTags([...selectedTags, t]);
      setNewTag("");
    }
  };

  const getOpLabel = (op: BatchOperationType | null) => {
    switch (op) {
      case "star":
        return "收藏";
      case "unstar":
        return "取消收藏";
      case "addTags":
        return "添加标签";
      case "removeTags":
        return "移除标签";
      case "export":
        return "导出";
      case "delete":
        return "删除";
      case "addToSession":
        return "添加到会话";
      default:
        return "";
    }
  };

  if (selectedCount === 0) return null;

  return (
    <div className={cn("space-y-0", className)}>
      {/* 批量操作工具栏 */}
      <div className="flex items-center justify-between px-4 py-2 bg-primary/10 border-b border-primary/20">
        <div className="flex items-center gap-3">
          <button
            onClick={handleSelectAll}
            className="flex items-center gap-2 text-sm hover:text-primary"
            title={allSelected ? "取消全选" : "全选"}
          >
            {allSelected ? (
              <CheckSquare className="h-4 w-4 text-primary" />
            ) : someSelected ? (
              <MinusSquare className="h-4 w-4 text-primary" />
            ) : (
              <Square className="h-4 w-4" />
            )}
            <span className="font-medium">
              已选择 {selectedCount} / {totalCount}
            </span>
          </button>
          <button
            onClick={handleClearSelection}
            className="p-1 rounded hover:bg-muted"
            title="清除选择"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* 操作按钮 */}
        <div className="flex items-center gap-1">
          <button
            onClick={handleBatchStar}
            disabled={operating}
            className="flex items-center gap-1 px-2 py-1 text-sm rounded hover:bg-muted disabled:opacity-50"
            title="批量收藏"
          >
            <Star className="h-4 w-4" />
            <span className="hidden sm:inline">收藏</span>
          </button>
          <button
            onClick={handleBatchUnstar}
            disabled={operating}
            className="flex items-center gap-1 px-2 py-1 text-sm rounded hover:bg-muted disabled:opacity-50"
            title="批量取消收藏"
          >
            <StarOff className="h-4 w-4" />
            <span className="hidden sm:inline">取消收藏</span>
          </button>
          <div className="relative">
            <button
              onClick={() => setShowMenu(!showMenu)}
              disabled={operating}
              className="flex items-center gap-1 px-2 py-1 text-sm rounded hover:bg-muted disabled:opacity-50"
            >
              <span>更多</span>
              <ChevronDown className="h-3 w-3" />
            </button>
            {showMenu && (
              <div className="absolute right-0 top-full mt-1 w-48 rounded-lg border bg-card shadow-lg z-20">
                <button
                  onClick={() => {
                    setTagMode("add");
                    setShowTagDialog(true);
                    setShowMenu(false);
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
                >
                  <Tag className="h-4 w-4" /> 添加标签
                </button>
                <button
                  onClick={() => {
                    setTagMode("remove");
                    setShowTagDialog(true);
                    setShowMenu(false);
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
                >
                  <Tag className="h-4 w-4" /> 移除标签
                </button>
                {sessions.length > 0 && (
                  <button
                    onClick={() => {
                      setShowSessionDialog(true);
                      setShowMenu(false);
                    }}
                    className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
                  >
                    <FolderPlus className="h-4 w-4" /> 添加到会话
                  </button>
                )}
                <button
                  onClick={() => {
                    setShowExportDialog(true);
                    setShowMenu(false);
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
                >
                  <Download className="h-4 w-4" /> 导出
                </button>
                <div className="border-t my-1" />
                <button
                  onClick={() => {
                    setShowDeleteConfirm(true);
                    setShowMenu(false);
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left text-red-600"
                >
                  <Trash2 className="h-4 w-4" /> 删除
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 操作进度/错误提示 */}
      {operating && (
        <div className="flex items-center gap-2 px-4 py-2 bg-blue-50 dark:bg-blue-950/20 text-blue-600 dark:text-blue-400 text-sm">
          <Loader2 className="h-4 w-4 animate-spin" />
          <span>正在执行批量{getOpLabel(currentOp)}...</span>
        </div>
      )}
      {error && (
        <div className="flex items-center gap-2 px-4 py-2 bg-red-50 dark:bg-red-950/20 text-red-600 dark:text-red-400 text-sm">
          <AlertCircle className="h-4 w-4" />
          <span>{error}</span>
          <button
            onClick={() => setError(null)}
            className="ml-auto p-1 hover:bg-red-100 dark:hover:bg-red-900/30 rounded"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      )}

      {/* 标签对话框 */}
      {showTagDialog && (
        <Dialog
          title={tagMode === "add" ? "添加标签" : "移除标签"}
          onClose={() => {
            setShowTagDialog(false);
            setSelectedTags([]);
          }}
        >
          <div className="space-y-4">
            {tagMode === "add" && (
              <div className="flex gap-2">
                <input
                  type="text"
                  value={newTag}
                  onChange={(e) => setNewTag(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleAddNewTag()}
                  placeholder="输入新标签"
                  className="flex-1 rounded-lg border bg-background px-3 py-2 text-sm"
                />
                <button
                  onClick={handleAddNewTag}
                  className="px-3 py-2 rounded-lg border hover:bg-muted"
                >
                  添加
                </button>
              </div>
            )}
            {availableTags.length > 0 && (
              <div className="space-y-2">
                <p className="text-sm text-muted-foreground">可用标签:</p>
                <div className="flex flex-wrap gap-2">
                  {availableTags.map((tag) => (
                    <button
                      key={tag}
                      onClick={() => toggleTag(tag)}
                      className={cn(
                        "px-2 py-1 text-sm rounded-full border",
                        selectedTags.includes(tag)
                          ? "bg-primary text-primary-foreground"
                          : "hover:bg-muted",
                      )}
                    >
                      {tag}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {selectedTags.length > 0 && (
              <div className="space-y-2">
                <p className="text-sm text-muted-foreground">已选标签:</p>
                <div className="flex flex-wrap gap-2">
                  {selectedTags.map((tag) => (
                    <span
                      key={tag}
                      className="flex items-center gap-1 px-2 py-1 text-sm rounded-full bg-primary/10"
                    >
                      {tag}
                      <button
                        onClick={() => toggleTag(tag)}
                        className="hover:text-red-500"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
          <div className="flex justify-end gap-2 mt-4">
            <button
              onClick={() => {
                setShowTagDialog(false);
                setSelectedTags([]);
              }}
              className="px-4 py-2 text-sm rounded-lg border hover:bg-muted"
            >
              取消
            </button>
            <button
              onClick={
                tagMode === "add" ? handleBatchAddTags : handleBatchRemoveTags
              }
              disabled={selectedTags.length === 0 || operating}
              className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {operating ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Check className="h-4 w-4" />
              )}
              确定
            </button>
          </div>
        </Dialog>
      )}

      {/* 会话对话框 */}
      {showSessionDialog && (
        <Dialog
          title="添加到会话"
          onClose={() => {
            setShowSessionDialog(false);
            setSelectedSessionId("");
          }}
        >
          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">选择要添加到的会话:</p>
            <select
              value={selectedSessionId}
              onChange={(e) => setSelectedSessionId(e.target.value)}
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
            >
              <option value="">请选择会话</option>
              {sessions.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </div>
          <div className="flex justify-end gap-2 mt-4">
            <button
              onClick={() => {
                setShowSessionDialog(false);
                setSelectedSessionId("");
              }}
              className="px-4 py-2 text-sm rounded-lg border hover:bg-muted"
            >
              取消
            </button>
            <button
              onClick={handleBatchAddToSession}
              disabled={!selectedSessionId || operating}
              className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {operating ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Check className="h-4 w-4" />
              )}
              添加
            </button>
          </div>
        </Dialog>
      )}

      {/* 导出对话框 */}
      {showExportDialog && (
        <Dialog title="导出 Flow" onClose={() => setShowExportDialog(false)}>
          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">选择导出格式:</p>
            <select
              value={exportFormat}
              onChange={(e) => setExportFormat(e.target.value as ExportFormat)}
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
            >
              <option value="json">JSON</option>
              <option value="jsonl">JSONL</option>
              <option value="har">HAR</option>
              <option value="markdown">Markdown</option>
              <option value="csv">CSV</option>
            </select>
            <p className="text-xs text-muted-foreground">
              将导出 {selectedCount} 个 Flow
            </p>
          </div>
          <div className="flex justify-end gap-2 mt-4">
            <button
              onClick={() => setShowExportDialog(false)}
              className="px-4 py-2 text-sm rounded-lg border hover:bg-muted"
            >
              取消
            </button>
            <button
              onClick={handleBatchExport}
              disabled={operating}
              className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {operating ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Download className="h-4 w-4" />
              )}
              导出
            </button>
          </div>
        </Dialog>
      )}

      {/* 删除确认对话框 */}
      {showDeleteConfirm && (
        <Dialog title="确认删除" onClose={() => setShowDeleteConfirm(false)}>
          <div className="space-y-4">
            <div className="flex items-center gap-3 p-4 rounded-lg bg-red-50 dark:bg-red-950/20">
              <AlertCircle className="h-6 w-6 text-red-500 shrink-0" />
              <div>
                <p className="font-medium text-red-600 dark:text-red-400">
                  此操作不可撤销
                </p>
                <p className="text-sm text-red-600/80 dark:text-red-400/80">
                  确定要删除选中的 {selectedCount} 个 Flow 吗？
                </p>
              </div>
            </div>
          </div>
          <div className="flex justify-end gap-2 mt-4">
            <button
              onClick={() => setShowDeleteConfirm(false)}
              className="px-4 py-2 text-sm rounded-lg border hover:bg-muted"
            >
              取消
            </button>
            <button
              onClick={handleBatchDelete}
              disabled={operating}
              className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg bg-red-600 text-white hover:bg-red-700 disabled:opacity-50"
            >
              {operating ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Trash2 className="h-4 w-4" />
              )}
              删除
            </button>
          </div>
        </Dialog>
      )}
    </div>
  );
}

// ============================================================================
// 对话框组件
// ============================================================================

interface DialogProps {
  title: string;
  children: React.ReactNode;
  onClose: () => void;
}

function Dialog({ title, children, onClose }: DialogProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-md mx-4">
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <h2 className="text-lg font-semibold">{title}</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>
        <div className="px-6 py-4">{children}</div>
      </div>
    </div>
  );
}

export default BatchOperations;
