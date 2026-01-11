/**
 * 书签管理面板组件
 *
 * 实现书签列表、书签导航功能
 * **Validates: Requirements 8.1-8.6**
 */

import { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  Bookmark,
  Trash2,
  Edit2,
  Download,
  Upload,
  ChevronDown,
  ChevronUp,
  Loader2,
  AlertCircle,
  X,
  Search,
  MoreVertical,
  Check,
  FolderOpen,
  Navigation,
} from "lucide-react";
import { cn } from "@/lib/utils";

// ============================================================================
// 类型定义
// ============================================================================

/**
 * Flow 书签
 */
export interface FlowBookmark {
  id: string;
  flow_id: string;
  name?: string;
  group?: string;
  created_at: string;
}

/**
 * 更新书签请求
 */
interface UpdateBookmarkRequest {
  bookmark_id: string;
  name?: string | null;
  group?: string | null;
}

/**
 * 导入书签请求
 */
interface ImportBookmarksRequest {
  data: string;
  overwrite: boolean;
}

// ============================================================================
// 组件属性
// ============================================================================

interface BookmarkPanelProps {
  className?: string;
  /** 导航到 Flow 回调 */
  onNavigateToFlow?: (flowId: string) => void;
  /** 当前选中的 Flow ID */
  currentFlowId?: string;
}

// ============================================================================
// 主组件
// ============================================================================

export function BookmarkPanel({
  className,
  onNavigateToFlow,
  currentFlowId,
}: BookmarkPanelProps) {
  // 状态
  const [bookmarks, setBookmarks] = useState<FlowBookmark[]>([]);
  const [groups, setGroups] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

  // 编辑书签对话框状态
  const [editingBookmark, setEditingBookmark] = useState<FlowBookmark | null>(
    null,
  );
  const [editName, setEditName] = useState("");
  const [editGroup, setEditGroup] = useState("");
  const [saving, setSaving] = useState(false);

  // 操作菜单状态
  const [menuBookmarkId, setMenuBookmarkId] = useState<string | null>(null);

  // 导入/导出状态
  const [importing, setImporting] = useState(false);
  const [exporting, setExporting] = useState(false);

  // 加载书签列表
  const loadBookmarks = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [bookmarkList, groupList] = await Promise.all([
        safeInvoke<FlowBookmark[]>("list_bookmarks", { group: null }),
        safeInvoke<string[]>("list_bookmark_groups"),
      ]);
      setBookmarks(bookmarkList);
      setGroups(groupList);
      // 默认展开所有分组
      setExpandedGroups(new Set(groupList));
    } catch (e) {
      console.error("加载书签失败:", e);
      setError(e instanceof Error ? e.message : "加载书签失败");
    } finally {
      setLoading(false);
    }
  }, []);

  // 初始化加载
  useEffect(() => {
    loadBookmarks();
  }, [loadBookmarks]);

  // 更新书签
  const handleUpdate = useCallback(async () => {
    if (!editingBookmark) return;

    try {
      setSaving(true);
      const updated = await safeInvoke<FlowBookmark>("update_bookmark", {
        request: {
          bookmark_id: editingBookmark.id,
          name: editName.trim() || null,
          group: editGroup.trim() || null,
        } as UpdateBookmarkRequest,
      });
      setBookmarks((prev) =>
        prev.map((b) => (b.id === editingBookmark.id ? updated : b)),
      );
      if (updated.group && !groups.includes(updated.group)) {
        setGroups((prev) => [...prev, updated.group!].sort());
      }
      setEditingBookmark(null);
    } catch (e) {
      console.error("更新书签失败:", e);
      setError(e instanceof Error ? e.message : "更新书签失败");
    } finally {
      setSaving(false);
    }
  }, [editingBookmark, editName, editGroup, groups]);

  // 删除书签
  const handleDelete = useCallback(async (bookmarkId: string) => {
    if (!confirm("确定要删除此书签吗？")) return;

    try {
      await safeInvoke("remove_bookmark", { bookmarkId });
      setBookmarks((prev) => prev.filter((b) => b.id !== bookmarkId));
      setMenuBookmarkId(null);
    } catch (e) {
      console.error("删除书签失败:", e);
      setError(e instanceof Error ? e.message : "删除书签失败");
    }
  }, []);

  // 导出书签
  const handleExport = useCallback(async () => {
    try {
      setExporting(true);
      const data = await safeInvoke<string>("export_bookmarks");

      // 下载文件
      const blob = new Blob([data], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `bookmarks_${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error("导出书签失败:", e);
      setError(e instanceof Error ? e.message : "导出书签失败");
    } finally {
      setExporting(false);
    }
  }, []);

  // 导入书签
  const handleImport = useCallback(async () => {
    try {
      // 创建文件输入
      const input = document.createElement("input");
      input.type = "file";
      input.accept = ".json";
      input.onchange = async (e) => {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;

        try {
          setImporting(true);
          const data = await file.text();
          const imported = await safeInvoke<FlowBookmark[]>(
            "import_bookmarks",
            {
              request: {
                data,
                overwrite: false,
              } as ImportBookmarksRequest,
            },
          );
          if (imported.length > 0) {
            await loadBookmarks();
            setError(null);
          } else {
            setError("没有导入任何书签（可能已存在相同的书签）");
          }
        } catch (err) {
          console.error("导入书签失败:", err);
          setError(err instanceof Error ? err.message : "导入书签失败");
        } finally {
          setImporting(false);
        }
      };
      input.click();
    } catch (e) {
      console.error("导入书签失败:", e);
      setError(e instanceof Error ? e.message : "导入书签失败");
    }
  }, [loadBookmarks]);

  // 导航到 Flow
  const handleNavigate = useCallback(
    (bookmark: FlowBookmark) => {
      onNavigateToFlow?.(bookmark.flow_id);
      setMenuBookmarkId(null);
    },
    [onNavigateToFlow],
  );

  // 切换分组展开状态
  const toggleGroup = useCallback((group: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(group)) {
        next.delete(group);
      } else {
        next.add(group);
      }
      return next;
    });
  }, []);

  // 过滤书签
  const filteredBookmarks = bookmarks.filter((bookmark) => {
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      return (
        bookmark.name?.toLowerCase().includes(query) ||
        bookmark.flow_id.toLowerCase().includes(query) ||
        bookmark.group?.toLowerCase().includes(query)
      );
    }
    return true;
  });

  // 按分组组织书签
  const bookmarksByGroup = filteredBookmarks.reduce(
    (acc, bookmark) => {
      const group = bookmark.group || "未分组";
      if (!acc[group]) {
        acc[group] = [];
      }
      acc[group].push(bookmark);
      return acc;
    },
    {} as Record<string, FlowBookmark[]>,
  );

  // 排序分组（未分组在最后）
  const sortedGroups = Object.keys(bookmarksByGroup).sort((a, b) => {
    if (a === "未分组") return 1;
    if (b === "未分组") return -1;
    return a.localeCompare(b);
  });

  return (
    <div className={cn("rounded-lg border bg-card", className)}>
      {/* 头部 */}
      <div
        className="flex items-center justify-between px-4 py-3 border-b cursor-pointer hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          <Bookmark className="h-5 w-5 text-primary" />
          <span className="font-medium">书签</span>
          <span className="text-xs text-muted-foreground">
            ({bookmarks.length})
          </span>
        </div>
        <div className="flex items-center gap-2">
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

          {/* 搜索和操作 */}
          <div className="flex items-center gap-2">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索书签..."
                className="w-full pl-9 pr-3 py-2 text-sm rounded-lg border bg-background"
              />
            </div>
            <button
              onClick={handleExport}
              disabled={exporting || bookmarks.length === 0}
              className="p-2 rounded-lg border hover:bg-muted disabled:opacity-50"
              title="导出书签"
            >
              {exporting ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Download className="h-4 w-4" />
              )}
            </button>
            <button
              onClick={handleImport}
              disabled={importing}
              className="p-2 rounded-lg border hover:bg-muted disabled:opacity-50"
              title="导入书签"
            >
              {importing ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Upload className="h-4 w-4" />
              )}
            </button>
          </div>

          {/* 加载状态 */}
          {loading && (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          )}

          {/* 书签列表 */}
          {!loading && (
            <div className="space-y-3">
              {sortedGroups.length > 0 ? (
                sortedGroups.map((group) => (
                  <BookmarkGroup
                    key={group}
                    group={group}
                    bookmarks={bookmarksByGroup[group]}
                    expanded={expandedGroups.has(group)}
                    currentFlowId={currentFlowId}
                    menuBookmarkId={menuBookmarkId}
                    onToggle={() => toggleGroup(group)}
                    onNavigate={handleNavigate}
                    onEdit={(bookmark) => {
                      setEditingBookmark(bookmark);
                      setEditName(bookmark.name || "");
                      setEditGroup(bookmark.group || "");
                    }}
                    onDelete={handleDelete}
                    onMenuToggle={(id) =>
                      setMenuBookmarkId(menuBookmarkId === id ? null : id)
                    }
                  />
                ))
              ) : (
                <div className="text-center py-8 text-muted-foreground">
                  <Bookmark className="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p className="text-sm">暂无书签</p>
                  <p className="text-xs mt-1">在 Flow 详情中点击书签图标添加</p>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* 编辑书签对话框 */}
      {editingBookmark && (
        <EditBookmarkDialog
          bookmark={editingBookmark}
          name={editName}
          group={editGroup}
          groups={groups}
          saving={saving}
          onNameChange={setEditName}
          onGroupChange={setEditGroup}
          onSave={handleUpdate}
          onClose={() => setEditingBookmark(null)}
        />
      )}
    </div>
  );
}

// ============================================================================
// 子组件
// ============================================================================

interface BookmarkGroupProps {
  group: string;
  bookmarks: FlowBookmark[];
  expanded: boolean;
  currentFlowId?: string;
  menuBookmarkId: string | null;
  onToggle: () => void;
  onNavigate: (bookmark: FlowBookmark) => void;
  onEdit: (bookmark: FlowBookmark) => void;
  onDelete: (bookmarkId: string) => void;
  onMenuToggle: (bookmarkId: string) => void;
}

function BookmarkGroup({
  group,
  bookmarks,
  expanded,
  currentFlowId,
  menuBookmarkId,
  onToggle,
  onNavigate,
  onEdit,
  onDelete,
  onMenuToggle,
}: BookmarkGroupProps) {
  return (
    <div className="rounded-lg border bg-muted/20">
      {/* 分组头部 */}
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-muted/50 rounded-t-lg"
      >
        <div className="flex items-center gap-2">
          <FolderOpen className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-medium">{group}</span>
          <span className="text-xs text-muted-foreground">
            ({bookmarks.length})
          </span>
        </div>
        {expanded ? (
          <ChevronUp className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        )}
      </button>

      {/* 书签列表 */}
      {expanded && (
        <div className="px-2 pb-2 space-y-1">
          {bookmarks.map((bookmark) => (
            <BookmarkItem
              key={bookmark.id}
              bookmark={bookmark}
              active={currentFlowId === bookmark.flow_id}
              menuOpen={menuBookmarkId === bookmark.id}
              onNavigate={() => onNavigate(bookmark)}
              onEdit={() => onEdit(bookmark)}
              onDelete={() => onDelete(bookmark.id)}
              onMenuToggle={() => onMenuToggle(bookmark.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

interface BookmarkItemProps {
  bookmark: FlowBookmark;
  active: boolean;
  menuOpen: boolean;
  onNavigate: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onMenuToggle: () => void;
}

function BookmarkItem({
  bookmark,
  active,
  menuOpen,
  onNavigate,
  onEdit,
  onDelete,
  onMenuToggle,
}: BookmarkItemProps) {
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
        "relative flex items-center justify-between p-2 rounded-lg cursor-pointer transition-colors",
        active
          ? "bg-primary/10 border border-primary"
          : "hover:bg-muted/50 border border-transparent",
      )}
      onClick={onNavigate}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <Bookmark
            className={cn(
              "h-4 w-4 shrink-0",
              active ? "text-primary fill-primary" : "text-muted-foreground",
            )}
          />
          <span className="text-sm font-medium truncate">
            {bookmark.name || `Flow ${bookmark.flow_id.slice(0, 8)}...`}
          </span>
        </div>
        <div className="flex items-center gap-2 mt-0.5 ml-6">
          <code className="text-xs text-muted-foreground font-mono truncate">
            {bookmark.flow_id.slice(0, 12)}...
          </code>
          <span className="text-xs text-muted-foreground">
            {formatDate(bookmark.created_at)}
          </span>
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
            className="absolute right-0 top-full mt-1 w-32 rounded-lg border bg-card shadow-lg z-10"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              onClick={onNavigate}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
            >
              <Navigation className="h-4 w-4" />
              跳转
            </button>
            <button
              onClick={onEdit}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted text-left"
            >
              <Edit2 className="h-4 w-4" />
              编辑
            </button>
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
// 编辑书签对话框
// ============================================================================

interface EditBookmarkDialogProps {
  bookmark: FlowBookmark;
  name: string;
  group: string;
  groups: string[];
  saving: boolean;
  onNameChange: (name: string) => void;
  onGroupChange: (group: string) => void;
  onSave: () => void;
  onClose: () => void;
}

function EditBookmarkDialog({
  bookmark,
  name,
  group,
  groups,
  saving,
  onNameChange,
  onGroupChange,
  onSave,
  onClose,
}: EditBookmarkDialogProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-md mx-4">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <div className="flex items-center gap-2">
            <Edit2 className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">编辑书签</h2>
          </div>
          <button onClick={onClose} className="p-1 rounded hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-4 space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">书签名称</label>
            <input
              type="text"
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder="输入书签名称（可选）"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">分组</label>
            <input
              type="text"
              value={group}
              onChange={(e) => onGroupChange(e.target.value)}
              placeholder="输入或选择分组（可选）"
              list="bookmark-groups"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
            />
            <datalist id="bookmark-groups">
              {groups.map((g) => (
                <option key={g} value={g} />
              ))}
            </datalist>
          </div>
          <div className="text-xs text-muted-foreground">
            <p>书签 ID: {bookmark.id.slice(0, 8)}...</p>
            <p>Flow ID: {bookmark.flow_id.slice(0, 12)}...</p>
            <p>
              创建时间: {new Date(bookmark.created_at).toLocaleString("zh-CN")}
            </p>
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
            disabled={saving}
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

export default BookmarkPanel;
