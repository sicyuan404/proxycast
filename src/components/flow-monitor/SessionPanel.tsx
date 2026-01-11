/**
 * 会话管理面板组件
 *
 * 实现会话列表、会话创建/编辑/删除功能
 * **Validates: Requirements 5.1-5.7**
 */

import { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  FolderOpen,
  Plus,
  Archive,
  ArchiveRestore,
  Trash2,
  Edit2,
  Download,
  ChevronDown,
  ChevronUp,
  Loader2,
  AlertCircle,
  X,
  Search,
  FolderClosed,
  MoreVertical,
  Check,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { ExportFormat } from "@/lib/api/flowMonitor";

// ============================================================================
// 类型定义
// ============================================================================

/**
 * Flow 会话
 */
export interface FlowSession {
  id: string;
  name: string;
  description?: string;
  flow_ids: string[];
  created_at: string;
  updated_at: string;
  archived: boolean;
}

/**
 * 会话导出结果
 */
export interface SessionExportResult {
  session_id: string;
  session_name: string;
  flow_count: number;
  data: string;
  format: ExportFormat;
}

// ============================================================================
// 组件属性
// ============================================================================

interface SessionPanelProps {
  className?: string;
  /** 选中的会话 ID */
  selectedSessionId?: string;
  /** 会话选中回调 */
  onSessionSelect?: (session: FlowSession | null) => void;
  /** 查看会话详情回调 */
  onViewSessionDetail?: (session: FlowSession) => void;
}

// ============================================================================
// 主组件
// ============================================================================

export function SessionPanel({
  className,
  selectedSessionId,
  onSessionSelect,
  onViewSessionDetail,
}: SessionPanelProps) {
  // 状态
  const [sessions, setSessions] = useState<FlowSession[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [showArchived, setShowArchived] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  // 创建会话对话框状态
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createDescription, setCreateDescription] = useState("");
  const [creating, setCreating] = useState(false);

  // 编辑会话对话框状态
  const [editingSession, setEditingSession] = useState<FlowSession | null>(
    null,
  );
  const [editName, setEditName] = useState("");
  const [editDescription, setEditDescription] = useState("");
  const [saving, setSaving] = useState(false);

  // 操作菜单状态
  const [menuSessionId, setMenuSessionId] = useState<string | null>(null);

  // 加载会话列表
  const loadSessions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await safeInvoke<FlowSession[]>("list_sessions", {
        includeArchived: showArchived,
      });
      setSessions(result);
    } catch (e) {
      console.error("加载会话列表失败:", e);
      setError(e instanceof Error ? e.message : "加载会话列表失败");
    } finally {
      setLoading(false);
    }
  }, [showArchived]);

  // 初始化加载
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // 创建会话
  const handleCreate = useCallback(async () => {
    if (!createName.trim()) return;

    try {
      setCreating(true);
      const session = await safeInvoke<FlowSession>("create_session", {
        request: {
          name: createName.trim(),
          description: createDescription.trim() || null,
        },
      });
      setSessions((prev) => [session, ...prev]);
      setShowCreateDialog(false);
      setCreateName("");
      setCreateDescription("");
      onSessionSelect?.(session);
    } catch (e) {
      console.error("创建会话失败:", e);
      setError(e instanceof Error ? e.message : "创建会话失败");
    } finally {
      setCreating(false);
    }
  }, [createName, createDescription, onSessionSelect]);

  // 更新会话
  const handleUpdate = useCallback(async () => {
    if (!editingSession || !editName.trim()) return;

    try {
      setSaving(true);
      await safeInvoke("update_session", {
        request: {
          session_id: editingSession.id,
          name: editName.trim(),
          description: editDescription.trim() ? editDescription.trim() : null,
        },
      });
      setSessions((prev) =>
        prev.map((s) =>
          s.id === editingSession.id
            ? {
                ...s,
                name: editName.trim(),
                description: editDescription.trim() || undefined,
              }
            : s,
        ),
      );
      setEditingSession(null);
    } catch (e) {
      console.error("更新会话失败:", e);
      setError(e instanceof Error ? e.message : "更新会话失败");
    } finally {
      setSaving(false);
    }
  }, [editingSession, editName, editDescription]);

  // 归档会话
  const handleArchive = useCallback(async (sessionId: string) => {
    try {
      await safeInvoke("archive_session", { sessionId });
      setSessions((prev) =>
        prev.map((s) => (s.id === sessionId ? { ...s, archived: true } : s)),
      );
      setMenuSessionId(null);
    } catch (e) {
      console.error("归档会话失败:", e);
      setError(e instanceof Error ? e.message : "归档会话失败");
    }
  }, []);

  // 取消归档会话
  const handleUnarchive = useCallback(async (sessionId: string) => {
    try {
      await safeInvoke("unarchive_session", { sessionId });
      setSessions((prev) =>
        prev.map((s) => (s.id === sessionId ? { ...s, archived: false } : s)),
      );
      setMenuSessionId(null);
    } catch (e) {
      console.error("取消归档会话失败:", e);
      setError(e instanceof Error ? e.message : "取消归档会话失败");
    }
  }, []);

  // 删除会话
  const handleDelete = useCallback(
    async (sessionId: string) => {
      if (!confirm("确定要删除此会话吗？此操作不可撤销。")) return;

      try {
        await safeInvoke("delete_session", { sessionId });
        setSessions((prev) => prev.filter((s) => s.id !== sessionId));
        setMenuSessionId(null);
        if (selectedSessionId === sessionId) {
          onSessionSelect?.(null);
        }
      } catch (e) {
        console.error("删除会话失败:", e);
        setError(e instanceof Error ? e.message : "删除会话失败");
      }
    },
    [selectedSessionId, onSessionSelect],
  );

  // 导出会话
  const handleExport = useCallback(
    async (sessionId: string, format: ExportFormat = "json") => {
      try {
        const result = await safeInvoke<SessionExportResult>("export_session", {
          request: {
            session_id: sessionId,
            format,
          },
        });

        // 下载文件
        const blob = new Blob([result.data], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `session_${result.session_name}_${new Date().toISOString().slice(0, 10)}.${format}`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
        setMenuSessionId(null);
      } catch (e) {
        console.error("导出会话失败:", e);
        setError(e instanceof Error ? e.message : "导出会话失败");
      }
    },
    [],
  );

  // 过滤会话
  const filteredSessions = sessions.filter((session) => {
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      return (
        session.name.toLowerCase().includes(query) ||
        session.description?.toLowerCase().includes(query)
      );
    }
    return true;
  });

  // 分组：活跃和已归档
  const activeSessions = filteredSessions.filter((s) => !s.archived);
  const archivedSessions = filteredSessions.filter((s) => s.archived);

  return (
    <div className={cn("rounded-lg border bg-card", className)}>
      {/* 头部 */}
      <div
        className="flex items-center justify-between px-4 py-3 border-b cursor-pointer hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          <FolderOpen className="h-5 w-5 text-primary" />
          <span className="font-medium">会话管理</span>
          <span className="text-xs text-muted-foreground">
            ({activeSessions.length})
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={(e) => {
              e.stopPropagation();
              setShowCreateDialog(true);
            }}
            className="p-1.5 rounded hover:bg-muted"
            title="创建会话"
          >
            <Plus className="h-4 w-4" />
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
              <span className="flex-1">{error}</span>
              <button
                onClick={() => setError(null)}
                className="p-1 hover:bg-red-100 dark:hover:bg-red-900/30 rounded"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          )}

          {/* 搜索和过滤 */}
          <div className="flex items-center gap-2">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索会话..."
                className="w-full pl-9 pr-3 py-2 text-sm rounded-lg border bg-background"
              />
            </div>
            <label className="flex items-center gap-2 text-sm cursor-pointer">
              <input
                type="checkbox"
                checked={showArchived}
                onChange={(e) => setShowArchived(e.target.checked)}
                className="rounded border-gray-300"
              />
              <span className="text-muted-foreground">显示已归档</span>
            </label>
          </div>

          {/* 加载状态 */}
          {loading && (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          )}

          {/* 会话列表 */}
          {!loading && (
            <div className="space-y-2">
              {/* 活跃会话 */}
              {activeSessions.length > 0 ? (
                activeSessions.map((session) => (
                  <SessionItem
                    key={session.id}
                    session={session}
                    selected={selectedSessionId === session.id}
                    menuOpen={menuSessionId === session.id}
                    onSelect={() => onSessionSelect?.(session)}
                    onViewDetail={() => onViewSessionDetail?.(session)}
                    onEdit={() => {
                      setEditingSession(session);
                      setEditName(session.name);
                      setEditDescription(session.description || "");
                    }}
                    onArchive={() => handleArchive(session.id)}
                    onDelete={() => handleDelete(session.id)}
                    onExport={(format) => handleExport(session.id, format)}
                    onMenuToggle={() =>
                      setMenuSessionId(
                        menuSessionId === session.id ? null : session.id,
                      )
                    }
                  />
                ))
              ) : (
                <div className="text-center py-8 text-muted-foreground">
                  <FolderClosed className="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p className="text-sm">暂无会话</p>
                  <button
                    onClick={() => setShowCreateDialog(true)}
                    className="mt-2 text-sm text-primary hover:underline"
                  >
                    创建第一个会话
                  </button>
                </div>
              )}

              {/* 已归档会话 */}
              {showArchived && archivedSessions.length > 0 && (
                <>
                  <div className="flex items-center gap-2 pt-4 pb-2">
                    <Archive className="h-4 w-4 text-muted-foreground" />
                    <span className="text-sm text-muted-foreground">
                      已归档 ({archivedSessions.length})
                    </span>
                  </div>
                  {archivedSessions.map((session) => (
                    <SessionItem
                      key={session.id}
                      session={session}
                      selected={selectedSessionId === session.id}
                      menuOpen={menuSessionId === session.id}
                      onSelect={() => onSessionSelect?.(session)}
                      onViewDetail={() => onViewSessionDetail?.(session)}
                      onEdit={() => {
                        setEditingSession(session);
                        setEditName(session.name);
                        setEditDescription(session.description || "");
                      }}
                      onUnarchive={() => handleUnarchive(session.id)}
                      onDelete={() => handleDelete(session.id)}
                      onExport={(format) => handleExport(session.id, format)}
                      onMenuToggle={() =>
                        setMenuSessionId(
                          menuSessionId === session.id ? null : session.id,
                        )
                      }
                    />
                  ))}
                </>
              )}
            </div>
          )}
        </div>
      )}

      {/* 创建会话对话框 */}
      {showCreateDialog && (
        <CreateSessionDialog
          name={createName}
          description={createDescription}
          creating={creating}
          onNameChange={setCreateName}
          onDescriptionChange={setCreateDescription}
          onCreate={handleCreate}
          onClose={() => {
            setShowCreateDialog(false);
            setCreateName("");
            setCreateDescription("");
          }}
        />
      )}

      {/* 编辑会话对话框 */}
      {editingSession && (
        <EditSessionDialog
          session={editingSession}
          name={editName}
          description={editDescription}
          saving={saving}
          onNameChange={setEditName}
          onDescriptionChange={setEditDescription}
          onSave={handleUpdate}
          onClose={() => setEditingSession(null)}
        />
      )}
    </div>
  );
}

// ============================================================================
// 子组件
// ============================================================================

interface SessionItemProps {
  session: FlowSession;
  selected: boolean;
  menuOpen: boolean;
  onSelect: () => void;
  onViewDetail: () => void;
  onEdit: () => void;
  onArchive?: () => void;
  onUnarchive?: () => void;
  onDelete: () => void;
  onExport: (format: ExportFormat) => void;
  onMenuToggle: () => void;
}

function SessionItem({
  session,
  selected,
  menuOpen,
  onSelect,
  onViewDetail,
  onEdit,
  onArchive,
  onUnarchive,
  onDelete,
  onExport,
  onMenuToggle,
}: SessionItemProps) {
  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString("zh-CN", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  return (
    <div
      className={cn(
        "relative flex items-center justify-between p-3 rounded-lg border cursor-pointer transition-colors",
        selected
          ? "border-primary bg-primary/5"
          : "border-transparent bg-muted/30 hover:bg-muted/50",
        session.archived && "opacity-60",
      )}
      onClick={onSelect}
    >
      <div className="flex items-center gap-3 flex-1 min-w-0">
        <FolderOpen
          className={cn(
            "h-5 w-5 shrink-0",
            selected ? "text-primary" : "text-muted-foreground",
          )}
        />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium truncate">{session.name}</span>
            {session.archived && (
              <span className="px-1.5 py-0.5 text-xs rounded bg-muted text-muted-foreground">
                已归档
              </span>
            )}
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <span>{session.flow_ids.length} 个 Flow</span>
            <span>•</span>
            <span>{formatDate(session.updated_at)}</span>
          </div>
          {session.description && (
            <p className="text-xs text-muted-foreground truncate mt-1">
              {session.description}
            </p>
          )}
        </div>
      </div>

      {/* 操作按钮 */}
      <div className="relative">
        <button
          onClick={(e) => {
            e.stopPropagation();
            onMenuToggle();
          }}
          className="p-1.5 rounded hover:bg-muted"
        >
          <MoreVertical className="h-4 w-4 text-muted-foreground" />
        </button>

        {/* 下拉菜单 */}
        {menuOpen && (
          <div
            className="absolute right-0 top-full mt-1 w-40 rounded-lg border bg-card shadow-lg z-10"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              onClick={onViewDetail}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
            >
              <FolderOpen className="h-4 w-4" />
              查看详情
            </button>
            <button
              onClick={onEdit}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
            >
              <Edit2 className="h-4 w-4" />
              编辑
            </button>
            <button
              onClick={() => onExport("json")}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
            >
              <Download className="h-4 w-4" />
              导出 JSON
            </button>
            <div className="border-t my-1" />
            {session.archived ? (
              <button
                onClick={onUnarchive}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
              >
                <ArchiveRestore className="h-4 w-4" />
                取消归档
              </button>
            ) : (
              <button
                onClick={onArchive}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
              >
                <Archive className="h-4 w-4" />
                归档
              </button>
            )}
            <button
              onClick={onDelete}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left text-red-600 dark:text-red-400"
            >
              <Trash2 className="h-4 w-4" />
              删除
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// 创建会话对话框
// ============================================================================

interface CreateSessionDialogProps {
  name: string;
  description: string;
  creating: boolean;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onCreate: () => void;
  onClose: () => void;
}

function CreateSessionDialog({
  name,
  description,
  creating,
  onNameChange,
  onDescriptionChange,
  onCreate,
  onClose,
}: CreateSessionDialogProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-md mx-4">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <div className="flex items-center gap-2">
            <Plus className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">创建会话</h2>
          </div>
          <button onClick={onClose} className="p-1 rounded hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-4 space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">会话名称 *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder="输入会话名称"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">描述（可选）</label>
            <textarea
              value={description}
              onChange={(e) => onDescriptionChange(e.target.value)}
              placeholder="输入会话描述"
              rows={3}
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm resize-none"
            />
          </div>
        </div>

        {/* 底部 */}
        <div className="flex items-center justify-end gap-3 px-6 py-4 border-t bg-muted/30">
          <button
            onClick={onClose}
            disabled={creating}
            className="px-4 py-2 text-sm rounded-lg border hover:bg-muted disabled:opacity-50"
          >
            取消
          </button>
          <button
            onClick={onCreate}
            disabled={creating || !name.trim()}
            className={cn(
              "flex items-center gap-2 px-4 py-2 text-sm rounded-lg",
              "bg-primary text-primary-foreground hover:bg-primary/90",
              "disabled:opacity-50 disabled:cursor-not-allowed",
            )}
          >
            {creating ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                创建中...
              </>
            ) : (
              <>
                <Check className="h-4 w-4" />
                创建
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 编辑会话对话框
// ============================================================================

interface EditSessionDialogProps {
  session: FlowSession;
  name: string;
  description: string;
  saving: boolean;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onSave: () => void;
  onClose: () => void;
}

function EditSessionDialog({
  session,
  name,
  description,
  saving,
  onNameChange,
  onDescriptionChange,
  onSave,
  onClose,
}: EditSessionDialogProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-md mx-4">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <div className="flex items-center gap-2">
            <Edit2 className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">编辑会话</h2>
          </div>
          <button onClick={onClose} className="p-1 rounded hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-4 space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">会话名称 *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder="输入会话名称"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">描述（可选）</label>
            <textarea
              value={description}
              onChange={(e) => onDescriptionChange(e.target.value)}
              placeholder="输入会话描述"
              rows={3}
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm resize-none"
            />
          </div>
          <div className="text-xs text-muted-foreground">
            <p>会话 ID: {session.id.slice(0, 8)}...</p>
            <p>包含 {session.flow_ids.length} 个 Flow</p>
          </div>
        </div>

        {/* 底部 */}
        <div className="flex items-center justify-end gap-3 px-6 py-4 border-t bg-muted/30">
          <button
            onClick={onClose}
            disabled={saving}
            className="px-4 py-2 text-sm rounded-lg border hover:bg-muted disabled:opacity-50"
          >
            取消
          </button>
          <button
            onClick={onSave}
            disabled={saving || !name.trim()}
            className={cn(
              "flex items-center gap-2 px-4 py-2 text-sm rounded-lg",
              "bg-primary text-primary-foreground hover:bg-primary/90",
              "disabled:opacity-50 disabled:cursor-not-allowed",
            )}
          >
            {saving ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                保存中...
              </>
            ) : (
              <>
                <Check className="h-4 w-4" />
                保存
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

export default SessionPanel;
