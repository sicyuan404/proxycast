/**
 * 会话详情组件
 *
 * 实现会话详情视图和 Flow 添加/移除功能
 * **Validates: Requirements 5.2, 5.3**
 */

import { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  FolderOpen,
  ArrowLeft,
  Plus,
  Minus,
  Download,
  Trash2,
  Archive,
  ArchiveRestore,
  Edit2,
  Loader2,
  AlertCircle,
  X,
  Search,
  Clock,
  FileText,
  Check,
  ChevronDown,
  ChevronUp,
  ExternalLink,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { LLMFlow, ExportFormat } from "@/lib/api/flowMonitor";
import type { FlowSession, SessionExportResult } from "./SessionPanel";

// ============================================================================
// 组件属性
// ============================================================================

interface SessionDetailProps {
  /** 会话对象 */
  session: FlowSession;
  /** 返回列表回调 */
  onBack: () => void;
  /** 会话更新回调 */
  onSessionUpdate?: (session: FlowSession) => void;
  /** 会话删除回调 */
  onSessionDelete?: () => void;
  /** 查看 Flow 详情回调 */
  onViewFlow?: (flowId: string) => void;
  className?: string;
}

// ============================================================================
// 主组件
// ============================================================================

export function SessionDetail({
  session: initialSession,
  onBack,
  onSessionUpdate,
  onSessionDelete,
  onViewFlow,
  className,
}: SessionDetailProps) {
  // 状态
  const [session, setSession] = useState<FlowSession>(initialSession);
  const [flows, setFlows] = useState<LLMFlow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // 编辑状态
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState(session.name);
  const [editDescription, setEditDescription] = useState(
    session.description || "",
  );
  const [saving, setSaving] = useState(false);

  // 添加 Flow 状态
  const [showAddFlow, setShowAddFlow] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<LLMFlow[]>([]);
  const [searching, setSearching] = useState(false);
  const [addingFlowId, setAddingFlowId] = useState<string | null>(null);

  // 移除 Flow 状态
  const [removingFlowId, setRemovingFlowId] = useState<string | null>(null);

  // 展开/折叠状态
  const [expandedFlowId, setExpandedFlowId] = useState<string | null>(null);

  // 加载会话中的 Flow
  const loadFlows = useCallback(async () => {
    if (session.flow_ids.length === 0) {
      setFlows([]);
      setLoading(false);
      return;
    }

    try {
      setLoading(true);
      setError(null);

      // 逐个获取 Flow 详情
      const flowPromises = session.flow_ids.map((id) =>
        safeInvoke<LLMFlow | null>("get_flow_detail", { flowId: id }),
      );
      const results = await Promise.all(flowPromises);
      const validFlows = results.filter((f): f is LLMFlow => f !== null);
      setFlows(validFlows);
    } catch (e) {
      console.error("加载 Flow 失败:", e);
      setError(e instanceof Error ? e.message : "加载 Flow 失败");
    } finally {
      setLoading(false);
    }
  }, [session.flow_ids]);

  // 初始化加载
  useEffect(() => {
    loadFlows();
  }, [loadFlows]);

  // 更新会话信息
  const handleSave = useCallback(async () => {
    if (!editName.trim()) return;

    try {
      setSaving(true);
      await safeInvoke("update_session", {
        request: {
          session_id: session.id,
          name: editName.trim(),
          description: editDescription.trim() ? editDescription.trim() : null,
        },
      });
      const updatedSession = {
        ...session,
        name: editName.trim(),
        description: editDescription.trim() || undefined,
      };
      setSession(updatedSession);
      onSessionUpdate?.(updatedSession);
      setEditing(false);
    } catch (e) {
      console.error("更新会话失败:", e);
      setError(e instanceof Error ? e.message : "更新会话失败");
    } finally {
      setSaving(false);
    }
  }, [session, editName, editDescription, onSessionUpdate]);

  // 搜索 Flow
  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim()) {
      setSearchResults([]);
      return;
    }

    try {
      setSearching(true);
      // 使用查询 API 搜索 Flow
      const result = await safeInvoke<{ flows: LLMFlow[] }>("query_flows", {
        request: {
          filter: {
            content_search: searchQuery.trim(),
          },
          sort_by: "created_at",
          sort_desc: true,
          page: 1,
          page_size: 20,
        },
      });
      // 过滤掉已在会话中的 Flow
      const filtered = result.flows.filter(
        (f) => !session.flow_ids.includes(f.id),
      );
      setSearchResults(filtered);
    } catch (e) {
      console.error("搜索 Flow 失败:", e);
    } finally {
      setSearching(false);
    }
  }, [searchQuery, session.flow_ids]);

  // 添加 Flow 到会话
  const handleAddFlow = useCallback(
    async (flowId: string) => {
      try {
        setAddingFlowId(flowId);
        await safeInvoke("add_flow_to_session", {
          sessionId: session.id,
          flowId,
        });

        // 更新本地状态
        const updatedSession = {
          ...session,
          flow_ids: [...session.flow_ids, flowId],
        };
        setSession(updatedSession);
        onSessionUpdate?.(updatedSession);

        // 获取新添加的 Flow 详情
        const flow = await safeInvoke<LLMFlow | null>("get_flow_detail", {
          flowId,
        });
        if (flow) {
          setFlows((prev) => [...prev, flow]);
        }

        // 从搜索结果中移除
        setSearchResults((prev) => prev.filter((f) => f.id !== flowId));
      } catch (e) {
        console.error("添加 Flow 失败:", e);
        setError(e instanceof Error ? e.message : "添加 Flow 失败");
      } finally {
        setAddingFlowId(null);
      }
    },
    [session, onSessionUpdate],
  );

  // 从会话移除 Flow
  const handleRemoveFlow = useCallback(
    async (flowId: string) => {
      try {
        setRemovingFlowId(flowId);
        await safeInvoke("remove_flow_from_session", {
          sessionId: session.id,
          flowId,
        });

        // 更新本地状态
        const updatedSession = {
          ...session,
          flow_ids: session.flow_ids.filter((id) => id !== flowId),
        };
        setSession(updatedSession);
        onSessionUpdate?.(updatedSession);
        setFlows((prev) => prev.filter((f) => f.id !== flowId));
      } catch (e) {
        console.error("移除 Flow 失败:", e);
        setError(e instanceof Error ? e.message : "移除 Flow 失败");
      } finally {
        setRemovingFlowId(null);
      }
    },
    [session, onSessionUpdate],
  );

  // 归档/取消归档会话
  const handleToggleArchive = useCallback(async () => {
    try {
      if (session.archived) {
        await safeInvoke("unarchive_session", { sessionId: session.id });
      } else {
        await safeInvoke("archive_session", { sessionId: session.id });
      }
      const updatedSession = { ...session, archived: !session.archived };
      setSession(updatedSession);
      onSessionUpdate?.(updatedSession);
    } catch (e) {
      console.error("归档操作失败:", e);
      setError(e instanceof Error ? e.message : "归档操作失败");
    }
  }, [session, onSessionUpdate]);

  // 删除会话
  const handleDelete = useCallback(async () => {
    if (!confirm("确定要删除此会话吗？此操作不可撤销。")) return;

    try {
      await safeInvoke("delete_session", { sessionId: session.id });
      onSessionDelete?.();
    } catch (e) {
      console.error("删除会话失败:", e);
      setError(e instanceof Error ? e.message : "删除会话失败");
    }
  }, [session.id, onSessionDelete]);

  // 导出会话
  const handleExport = useCallback(
    async (format: ExportFormat = "json") => {
      try {
        const result = await safeInvoke<SessionExportResult>("export_session", {
          request: {
            session_id: session.id,
            format,
          },
        });

        // 下载文件
        const blob = new Blob([result.data], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `session_${session.name}_${new Date().toISOString().slice(0, 10)}.${format}`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      } catch (e) {
        console.error("导出会话失败:", e);
        setError(e instanceof Error ? e.message : "导出会话失败");
      }
    },
    [session],
  );

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleString("zh-CN");
  };

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {/* 头部 */}
      <div className="flex items-center justify-between px-4 py-3 border-b bg-card">
        <div className="flex items-center gap-3">
          <button
            onClick={onBack}
            className="p-1.5 rounded hover:bg-muted"
            title="返回列表"
          >
            <ArrowLeft className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-2">
            <FolderOpen className="h-5 w-5 text-primary" />
            {editing ? (
              <input
                type="text"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                className="text-lg font-semibold bg-transparent border-b border-primary focus:outline-none"
                autoFocus
              />
            ) : (
              <h2 className="text-lg font-semibold">{session.name}</h2>
            )}
            {session.archived && (
              <span className="px-2 py-0.5 text-xs rounded bg-muted text-muted-foreground">
                已归档
              </span>
            )}
          </div>
        </div>

        {/* 操作按钮 */}
        <div className="flex items-center gap-2">
          {editing ? (
            <>
              <button
                onClick={() => {
                  setEditing(false);
                  setEditName(session.name);
                  setEditDescription(session.description || "");
                }}
                disabled={saving}
                className="px-3 py-1.5 text-sm rounded-lg border hover:bg-muted"
              >
                取消
              </button>
              <button
                onClick={handleSave}
                disabled={saving || !editName.trim()}
                className={cn(
                  "flex items-center gap-1 px-3 py-1.5 text-sm rounded-lg",
                  "bg-primary text-primary-foreground hover:bg-primary/90",
                  "disabled:opacity-50",
                )}
              >
                {saving ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Check className="h-4 w-4" />
                )}
                保存
              </button>
            </>
          ) : (
            <>
              <button
                onClick={() => setEditing(true)}
                className="p-1.5 rounded hover:bg-muted"
                title="编辑"
              >
                <Edit2 className="h-4 w-4" />
              </button>
              <button
                onClick={() => handleExport("json")}
                className="p-1.5 rounded hover:bg-muted"
                title="导出"
              >
                <Download className="h-4 w-4" />
              </button>
              <button
                onClick={handleToggleArchive}
                className="p-1.5 rounded hover:bg-muted"
                title={session.archived ? "取消归档" : "归档"}
              >
                {session.archived ? (
                  <ArchiveRestore className="h-4 w-4" />
                ) : (
                  <Archive className="h-4 w-4" />
                )}
              </button>
              <button
                onClick={handleDelete}
                className="p-1.5 rounded hover:bg-muted text-red-600 dark:text-red-400"
                title="删除"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </>
          )}
        </div>
      </div>

      {/* 会话信息 */}
      <div className="px-4 py-3 border-b bg-muted/30">
        {editing ? (
          <textarea
            value={editDescription}
            onChange={(e) => setEditDescription(e.target.value)}
            placeholder="输入会话描述（可选）"
            rows={2}
            className="w-full text-sm bg-transparent border rounded-lg px-3 py-2 resize-none"
          />
        ) : (
          session.description && (
            <p className="text-sm text-muted-foreground">
              {session.description}
            </p>
          )
        )}
        <div className="flex items-center gap-4 mt-2 text-xs text-muted-foreground">
          <span className="flex items-center gap-1">
            <FileText className="h-3 w-3" />
            {session.flow_ids.length} 个 Flow
          </span>
          <span className="flex items-center gap-1">
            <Clock className="h-3 w-3" />
            创建于 {formatDate(session.created_at)}
          </span>
          <span className="flex items-center gap-1">
            <Clock className="h-3 w-3" />
            更新于 {formatDate(session.updated_at)}
          </span>
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="mx-4 mt-4 flex items-center gap-2 p-3 rounded-lg bg-red-50 text-red-600 dark:bg-red-950/20 dark:text-red-400 text-sm">
          <AlertCircle className="h-4 w-4 shrink-0" />
          <span className="flex-1">{error}</span>
          <button
            onClick={() => setError(null)}
            className="p-1 hover:bg-red-100 dark:hover:bg-red-900/30 rounded"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      )}

      {/* 添加 Flow 区域 */}
      <div className="px-4 py-3 border-b">
        <button
          onClick={() => setShowAddFlow(!showAddFlow)}
          className="flex items-center gap-2 text-sm text-primary hover:underline"
        >
          <Plus className="h-4 w-4" />
          添加 Flow 到会话
          {showAddFlow ? (
            <ChevronUp className="h-4 w-4" />
          ) : (
            <ChevronDown className="h-4 w-4" />
          )}
        </button>

        {showAddFlow && (
          <div className="mt-3 space-y-3">
            <div className="flex items-center gap-2">
              <div className="relative flex-1">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                  placeholder="搜索 Flow..."
                  className="w-full pl-9 pr-3 py-2 text-sm rounded-lg border bg-background"
                />
              </div>
              <button
                onClick={handleSearch}
                disabled={searching || !searchQuery.trim()}
                className={cn(
                  "px-4 py-2 text-sm rounded-lg",
                  "bg-primary text-primary-foreground hover:bg-primary/90",
                  "disabled:opacity-50",
                )}
              >
                {searching ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  "搜索"
                )}
              </button>
            </div>

            {/* 搜索结果 */}
            {searchResults.length > 0 && (
              <div className="space-y-2 max-h-60 overflow-y-auto">
                {searchResults.map((flow) => (
                  <div
                    key={flow.id}
                    className="flex items-center justify-between p-2 rounded-lg border bg-muted/30"
                  >
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">
                        {flow.request.model}
                      </div>
                      <div className="text-xs text-muted-foreground">
                        {new Date(flow.timestamps.created).toLocaleString(
                          "zh-CN",
                        )}
                      </div>
                    </div>
                    <button
                      onClick={() => handleAddFlow(flow.id)}
                      disabled={addingFlowId === flow.id}
                      className="p-1.5 rounded hover:bg-primary/10 text-primary"
                    >
                      {addingFlowId === flow.id ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Plus className="h-4 w-4" />
                      )}
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Flow 列表 */}
      <div className="flex-1 overflow-y-auto p-4">
        {loading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : flows.length === 0 ? (
          <div className="text-center py-8 text-muted-foreground">
            <FileText className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p className="text-sm">此会话暂无 Flow</p>
            <button
              onClick={() => setShowAddFlow(true)}
              className="mt-2 text-sm text-primary hover:underline"
            >
              添加第一个 Flow
            </button>
          </div>
        ) : (
          <div className="space-y-2">
            {flows.map((flow) => (
              <FlowItem
                key={flow.id}
                flow={flow}
                expanded={expandedFlowId === flow.id}
                removing={removingFlowId === flow.id}
                onToggleExpand={() =>
                  setExpandedFlowId(expandedFlowId === flow.id ? null : flow.id)
                }
                onView={() => onViewFlow?.(flow.id)}
                onRemove={() => handleRemoveFlow(flow.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// 子组件
// ============================================================================

interface FlowItemProps {
  flow: LLMFlow;
  expanded: boolean;
  removing: boolean;
  onToggleExpand: () => void;
  onView: () => void;
  onRemove: () => void;
}

function FlowItem({
  flow,
  expanded,
  removing,
  onToggleExpand,
  onView,
  onRemove,
}: FlowItemProps) {
  const getStateColor = (state: string) => {
    switch (state) {
      case "Completed":
        return "text-green-600 bg-green-100 dark:bg-green-900/30";
      case "Failed":
        return "text-red-600 bg-red-100 dark:bg-red-900/30";
      case "Streaming":
        return "text-blue-600 bg-blue-100 dark:bg-blue-900/30";
      case "Pending":
        return "text-yellow-600 bg-yellow-100 dark:bg-yellow-900/30";
      default:
        return "text-gray-600 bg-gray-100 dark:bg-gray-900/30";
    }
  };

  const formatDuration = (ms: number) => {
    if (ms >= 1000) {
      return `${(ms / 1000).toFixed(2)}s`;
    }
    return `${ms}ms`;
  };

  const getContentPreview = () => {
    if (flow.response?.content) {
      const content = flow.response.content;
      return content.length > 100 ? content.slice(0, 100) + "..." : content;
    }
    return null;
  };

  return (
    <div className="rounded-lg border bg-card overflow-hidden">
      {/* 头部 */}
      <div
        className="flex items-center justify-between p-3 cursor-pointer hover:bg-muted/50"
        onClick={onToggleExpand}
      >
        <div className="flex items-center gap-3 flex-1 min-w-0">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="font-medium truncate">{flow.request.model}</span>
              <span
                className={cn(
                  "px-1.5 py-0.5 text-xs rounded",
                  getStateColor(flow.state),
                )}
              >
                {flow.state}
              </span>
              {flow.annotations.starred && (
                <span className="text-yellow-500">⭐</span>
              )}
            </div>
            <div className="flex items-center gap-2 text-xs text-muted-foreground mt-1">
              <span>{flow.metadata.provider}</span>
              <span>•</span>
              <span>{formatDuration(flow.timestamps.duration_ms)}</span>
              {flow.response?.usage && (
                <>
                  <span>•</span>
                  <span>
                    {flow.response.usage.input_tokens} /{" "}
                    {flow.response.usage.output_tokens} tokens
                  </span>
                </>
              )}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={(e) => {
              e.stopPropagation();
              onView();
            }}
            className="p-1.5 rounded hover:bg-muted"
            title="查看详情"
          >
            <ExternalLink className="h-4 w-4 text-muted-foreground" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              onRemove();
            }}
            disabled={removing}
            className="p-1.5 rounded hover:bg-red-100 dark:hover:bg-red-900/30 text-red-600 dark:text-red-400"
            title="从会话移除"
          >
            {removing ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Minus className="h-4 w-4" />
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
        <div className="px-3 pb-3 border-t bg-muted/30">
          <div className="pt-3 space-y-3">
            {/* 时间信息 */}
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-muted-foreground">创建时间:</span>
                <span className="ml-2">
                  {new Date(flow.timestamps.created).toLocaleString("zh-CN")}
                </span>
              </div>
              <div>
                <span className="text-muted-foreground">Flow ID:</span>
                <span className="ml-2 font-mono text-xs">
                  {flow.id.slice(0, 16)}...
                </span>
              </div>
            </div>

            {/* 标签 */}
            {flow.annotations.tags.length > 0 && (
              <div className="flex items-center gap-2 flex-wrap">
                {flow.annotations.tags.map((tag) => (
                  <span
                    key={tag}
                    className="px-2 py-0.5 text-xs rounded-full bg-primary/10 text-primary"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            )}

            {/* 内容预览 */}
            {getContentPreview() && (
              <div className="text-sm">
                <span className="text-muted-foreground">响应预览:</span>
                <p className="mt-1 text-xs bg-muted/50 rounded p-2 font-mono whitespace-pre-wrap">
                  {getContentPreview()}
                </p>
              </div>
            )}

            {/* 错误信息 */}
            {flow.error && (
              <div className="text-sm">
                <span className="text-red-600 dark:text-red-400">错误:</span>
                <p className="mt-1 text-xs bg-red-50 dark:bg-red-950/20 rounded p-2 text-red-600 dark:text-red-400">
                  {flow.error.message}
                </p>
              </div>
            )}

            {/* 评论 */}
            {flow.annotations.comment && (
              <div className="text-sm">
                <span className="text-muted-foreground">评论:</span>
                <p className="mt-1 text-xs bg-muted/50 rounded p-2">
                  {flow.annotations.comment}
                </p>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default SessionDetail;
