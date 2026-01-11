/**
 * 快速过滤器面板组件
 *
 * 实现快速过滤器列表、保存/编辑/删除功能
 * **Validates: Requirements 6.1-6.7**
 */

import { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  Filter,
  Plus,
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
  Zap,
  FolderOpen,
  Star,
} from "lucide-react";
import { cn } from "@/lib/utils";

// ============================================================================
// 类型定义
// ============================================================================

/**
 * 快速过滤器
 */
export interface QuickFilter {
  id: string;
  name: string;
  description?: string;
  filter_expr: string;
  group?: string;
  order: number;
  is_preset: boolean;
  created_at: string;
}

// ============================================================================
// 组件属性
// ============================================================================

interface QuickFilterPanelProps {
  className?: string;
  /** 应用过滤器回调 */
  onApplyFilter?: (filterExpr: string) => void;
  /** 当前过滤表达式（用于高亮匹配的过滤器） */
  currentFilterExpr?: string;
}

// ============================================================================
// 主组件
// ============================================================================

export function QuickFilterPanel({
  className,
  onApplyFilter,
  currentFilterExpr,
}: QuickFilterPanelProps) {
  // 状态
  const [filters, setFilters] = useState<QuickFilter[]>([]);
  const [groups, setGroups] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(
    new Set(["预设"]),
  );

  // 创建过滤器对话框状态
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createFilterExpr, setCreateFilterExpr] = useState("");
  const [createDescription, setCreateDescription] = useState("");
  const [createGroup, setCreateGroup] = useState("");
  const [creating, setCreating] = useState(false);

  // 编辑过滤器对话框状态
  const [editingFilter, setEditingFilter] = useState<QuickFilter | null>(null);
  const [editName, setEditName] = useState("");
  const [editFilterExpr, setEditFilterExpr] = useState("");
  const [editDescription, setEditDescription] = useState("");
  const [editGroup, setEditGroup] = useState("");
  const [saving, setSaving] = useState(false);

  // 操作菜单状态
  const [menuFilterId, setMenuFilterId] = useState<string | null>(null);

  // 导入/导出状态
  const [importing, setImporting] = useState(false);
  const [exporting, setExporting] = useState(false);

  // 加载过滤器列表
  const loadFilters = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [filterList, groupList] = await Promise.all([
        safeInvoke<QuickFilter[]>("list_quick_filters"),
        safeInvoke<string[]>("list_quick_filter_groups"),
      ]);
      setFilters(filterList);
      setGroups(groupList);
    } catch (e) {
      console.error("加载快速过滤器失败:", e);
      setError(e instanceof Error ? e.message : "加载快速过滤器失败");
    } finally {
      setLoading(false);
    }
  }, []);

  // 初始化加载
  useEffect(() => {
    loadFilters();
  }, [loadFilters]);

  // 创建过滤器
  const handleCreate = useCallback(async () => {
    if (!createName.trim() || !createFilterExpr.trim()) return;

    try {
      setCreating(true);
      const filter = await safeInvoke<QuickFilter>("save_quick_filter", {
        request: {
          name: createName.trim(),
          filter_expr: createFilterExpr.trim(),
          description: createDescription.trim() || null,
          group: createGroup.trim() || null,
        },
      });
      setFilters((prev) => [...prev, filter]);
      if (filter.group && !groups.includes(filter.group)) {
        setGroups((prev) => [...prev, filter.group!].sort());
      }
      setShowCreateDialog(false);
      setCreateName("");
      setCreateFilterExpr("");
      setCreateDescription("");
      setCreateGroup("");
    } catch (e) {
      console.error("创建快速过滤器失败:", e);
      setError(e instanceof Error ? e.message : "创建快速过滤器失败");
    } finally {
      setCreating(false);
    }
  }, [createName, createFilterExpr, createDescription, createGroup, groups]);

  // 更新过滤器
  const handleUpdate = useCallback(async () => {
    if (!editingFilter || !editName.trim() || !editFilterExpr.trim()) return;

    try {
      setSaving(true);
      const updated = await safeInvoke<QuickFilter>("update_quick_filter", {
        request: {
          id: editingFilter.id,
          name: editName.trim(),
          filter_expr: editFilterExpr.trim(),
          description: editDescription.trim() ? editDescription.trim() : null,
          group: editGroup.trim() ? editGroup.trim() : null,
        },
      });
      setFilters((prev) =>
        prev.map((f) => (f.id === editingFilter.id ? updated : f)),
      );
      if (updated.group && !groups.includes(updated.group)) {
        setGroups((prev) => [...prev, updated.group!].sort());
      }
      setEditingFilter(null);
    } catch (e) {
      console.error("更新快速过滤器失败:", e);
      setError(e instanceof Error ? e.message : "更新快速过滤器失败");
    } finally {
      setSaving(false);
    }
  }, [
    editingFilter,
    editName,
    editFilterExpr,
    editDescription,
    editGroup,
    groups,
  ]);

  // 删除过滤器
  const handleDelete = useCallback(
    async (filterId: string) => {
      const filter = filters.find((f) => f.id === filterId);
      if (filter?.is_preset) {
        setError("无法删除预设过滤器");
        return;
      }
      if (!confirm("确定要删除此过滤器吗？")) return;

      try {
        await safeInvoke("delete_quick_filter", { id: filterId });
        setFilters((prev) => prev.filter((f) => f.id !== filterId));
        setMenuFilterId(null);
      } catch (e) {
        console.error("删除快速过滤器失败:", e);
        setError(e instanceof Error ? e.message : "删除快速过滤器失败");
      }
    },
    [filters],
  );

  // 导出过滤器
  const handleExport = useCallback(async () => {
    try {
      setExporting(true);
      const data = await safeInvoke<string>("export_quick_filters", {
        includePresets: false,
      });

      // 下载文件
      const blob = new Blob([data], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `quick_filters_${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error("导出快速过滤器失败:", e);
      setError(e instanceof Error ? e.message : "导出快速过滤器失败");
    } finally {
      setExporting(false);
    }
  }, []);

  // 导入过滤器
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
          const imported = await safeInvoke<QuickFilter[]>(
            "import_quick_filters",
            {
              request: {
                data,
                overwrite: false,
              },
            },
          );
          if (imported.length > 0) {
            await loadFilters();
            setError(null);
          } else {
            setError("没有导入任何过滤器（可能已存在同名过滤器）");
          }
        } catch (err) {
          console.error("导入快速过滤器失败:", err);
          setError(err instanceof Error ? err.message : "导入快速过滤器失败");
        } finally {
          setImporting(false);
        }
      };
      input.click();
    } catch (e) {
      console.error("导入快速过滤器失败:", e);
      setError(e instanceof Error ? e.message : "导入快速过滤器失败");
    }
  }, [loadFilters]);

  // 应用过滤器
  const handleApplyFilter = useCallback(
    (filter: QuickFilter) => {
      onApplyFilter?.(filter.filter_expr);
      setMenuFilterId(null);
    },
    [onApplyFilter],
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

  // 过滤和分组过滤器
  const filteredFilters = filters.filter((filter) => {
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      return (
        filter.name.toLowerCase().includes(query) ||
        filter.description?.toLowerCase().includes(query) ||
        filter.filter_expr.toLowerCase().includes(query)
      );
    }
    return true;
  });

  // 按分组组织过滤器
  const filtersByGroup = filteredFilters.reduce(
    (acc, filter) => {
      const group = filter.group || "未分组";
      if (!acc[group]) {
        acc[group] = [];
      }
      acc[group].push(filter);
      return acc;
    },
    {} as Record<string, QuickFilter[]>,
  );

  // 排序分组（预设在前）
  const sortedGroups = Object.keys(filtersByGroup).sort((a, b) => {
    if (a === "预设") return -1;
    if (b === "预设") return 1;
    if (a === "未分组") return 1;
    if (b === "未分组") return -1;
    return a.localeCompare(b);
  });

  const customFilterCount = filters.filter((f) => !f.is_preset).length;

  return (
    <div className={cn("rounded-lg border bg-card", className)}>
      {/* 头部 */}
      <div
        className="flex items-center justify-between px-4 py-3 border-b cursor-pointer hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          <Zap className="h-5 w-5 text-primary" />
          <span className="font-medium">快速过滤器</span>
          <span className="text-xs text-muted-foreground">
            ({customFilterCount})
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={(e) => {
              e.stopPropagation();
              setShowCreateDialog(true);
            }}
            className="p-1.5 rounded hover:bg-muted"
            title="创建过滤器"
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

          {/* 搜索和操作 */}
          <div className="flex items-center gap-2">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索过滤器..."
                className="w-full pl-9 pr-3 py-2 text-sm rounded-lg border bg-background"
              />
            </div>
            <button
              onClick={handleExport}
              disabled={exporting || customFilterCount === 0}
              className="p-2 rounded-lg border hover:bg-muted disabled:opacity-50"
              title="导出过滤器"
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
              title="导入过滤器"
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

          {/* 过滤器列表 */}
          {!loading && (
            <div className="space-y-3">
              {sortedGroups.length > 0 ? (
                sortedGroups.map((group) => (
                  <FilterGroup
                    key={group}
                    group={group}
                    filters={filtersByGroup[group]}
                    expanded={expandedGroups.has(group)}
                    currentFilterExpr={currentFilterExpr}
                    menuFilterId={menuFilterId}
                    onToggle={() => toggleGroup(group)}
                    onApply={handleApplyFilter}
                    onEdit={(filter) => {
                      setEditingFilter(filter);
                      setEditName(filter.name);
                      setEditFilterExpr(filter.filter_expr);
                      setEditDescription(filter.description || "");
                      setEditGroup(filter.group || "");
                    }}
                    onDelete={handleDelete}
                    onMenuToggle={(id) =>
                      setMenuFilterId(menuFilterId === id ? null : id)
                    }
                  />
                ))
              ) : (
                <div className="text-center py-8 text-muted-foreground">
                  <Filter className="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p className="text-sm">暂无过滤器</p>
                  <button
                    onClick={() => setShowCreateDialog(true)}
                    className="mt-2 text-sm text-primary hover:underline"
                  >
                    创建第一个过滤器
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* 创建过滤器对话框 */}
      {showCreateDialog && (
        <CreateFilterDialog
          name={createName}
          filterExpr={createFilterExpr}
          description={createDescription}
          group={createGroup}
          groups={groups}
          creating={creating}
          onNameChange={setCreateName}
          onFilterExprChange={setCreateFilterExpr}
          onDescriptionChange={setCreateDescription}
          onGroupChange={setCreateGroup}
          onCreate={handleCreate}
          onClose={() => {
            setShowCreateDialog(false);
            setCreateName("");
            setCreateFilterExpr("");
            setCreateDescription("");
            setCreateGroup("");
          }}
        />
      )}

      {/* 编辑过滤器对话框 */}
      {editingFilter && (
        <EditFilterDialog
          filter={editingFilter}
          name={editName}
          filterExpr={editFilterExpr}
          description={editDescription}
          group={editGroup}
          groups={groups}
          saving={saving}
          onNameChange={setEditName}
          onFilterExprChange={setEditFilterExpr}
          onDescriptionChange={setEditDescription}
          onGroupChange={setEditGroup}
          onSave={handleUpdate}
          onClose={() => setEditingFilter(null)}
        />
      )}
    </div>
  );
}

// ============================================================================
// 子组件
// ============================================================================

interface FilterGroupProps {
  group: string;
  filters: QuickFilter[];
  expanded: boolean;
  currentFilterExpr?: string;
  menuFilterId: string | null;
  onToggle: () => void;
  onApply: (filter: QuickFilter) => void;
  onEdit: (filter: QuickFilter) => void;
  onDelete: (filterId: string) => void;
  onMenuToggle: (filterId: string) => void;
}

function FilterGroup({
  group,
  filters,
  expanded,
  currentFilterExpr,
  menuFilterId,
  onToggle,
  onApply,
  onEdit,
  onDelete,
  onMenuToggle,
}: FilterGroupProps) {
  const isPreset = group === "预设";

  return (
    <div className="rounded-lg border bg-muted/20">
      {/* 分组头部 */}
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-muted/50 rounded-t-lg"
      >
        <div className="flex items-center gap-2">
          {isPreset ? (
            <Star className="h-4 w-4 text-yellow-500" />
          ) : (
            <FolderOpen className="h-4 w-4 text-muted-foreground" />
          )}
          <span className="text-sm font-medium">{group}</span>
          <span className="text-xs text-muted-foreground">
            ({filters.length})
          </span>
        </div>
        {expanded ? (
          <ChevronUp className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        )}
      </button>

      {/* 过滤器列表 */}
      {expanded && (
        <div className="px-2 pb-2 space-y-1">
          {filters.map((filter) => (
            <FilterItem
              key={filter.id}
              filter={filter}
              active={currentFilterExpr === filter.filter_expr}
              menuOpen={menuFilterId === filter.id}
              onApply={() => onApply(filter)}
              onEdit={() => onEdit(filter)}
              onDelete={() => onDelete(filter.id)}
              onMenuToggle={() => onMenuToggle(filter.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

interface FilterItemProps {
  filter: QuickFilter;
  active: boolean;
  menuOpen: boolean;
  onApply: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onMenuToggle: () => void;
}

function FilterItem({
  filter,
  active,
  menuOpen,
  onApply,
  onEdit,
  onDelete,
  onMenuToggle,
}: FilterItemProps) {
  return (
    <div
      className={cn(
        "relative flex items-center justify-between p-2 rounded-lg cursor-pointer transition-colors",
        active
          ? "bg-primary/10 border border-primary"
          : "hover:bg-muted/50 border border-transparent",
      )}
      onClick={onApply}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <Filter
            className={cn(
              "h-4 w-4 shrink-0",
              active ? "text-primary" : "text-muted-foreground",
            )}
          />
          <span className="text-sm font-medium truncate">{filter.name}</span>
          {filter.is_preset && (
            <span className="px-1.5 py-0.5 text-xs rounded bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300">
              预设
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 mt-0.5 ml-6">
          <code className="text-xs text-muted-foreground font-mono truncate">
            {filter.filter_expr}
          </code>
        </div>
        {filter.description && (
          <p className="text-xs text-muted-foreground truncate mt-0.5 ml-6">
            {filter.description}
          </p>
        )}
      </div>

      {/* 操作按钮 */}
      {!filter.is_preset && (
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
      )}
    </div>
  );
}

// ============================================================================
// 创建过滤器对话框
// ============================================================================

interface CreateFilterDialogProps {
  name: string;
  filterExpr: string;
  description: string;
  group: string;
  groups: string[];
  creating: boolean;
  onNameChange: (name: string) => void;
  onFilterExprChange: (expr: string) => void;
  onDescriptionChange: (description: string) => void;
  onGroupChange: (group: string) => void;
  onCreate: () => void;
  onClose: () => void;
}

function CreateFilterDialog({
  name,
  filterExpr,
  description,
  group,
  groups,
  creating,
  onNameChange,
  onFilterExprChange,
  onDescriptionChange,
  onGroupChange,
  onCreate,
  onClose,
}: CreateFilterDialogProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-md mx-4">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <div className="flex items-center gap-2">
            <Plus className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">创建快速过滤器</h2>
          </div>
          <button onClick={onClose} className="p-1 rounded hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-4 space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">名称 *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder="输入过滤器名称"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">过滤表达式 *</label>
            <input
              type="text"
              value={filterExpr}
              onChange={(e) => onFilterExprChange(e.target.value)}
              placeholder="例如: ~e 或 ~p kiro & ~m claude"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm font-mono"
            />
            <p className="text-xs text-muted-foreground">
              支持 ~m, ~p, ~s, ~e, ~t, ~k, ~b, ~starred, ~tag, ~tokens, ~latency
              等过滤器
            </p>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">描述（可选）</label>
            <textarea
              value={description}
              onChange={(e) => onDescriptionChange(e.target.value)}
              placeholder="输入过滤器描述"
              rows={2}
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm resize-none"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">分组（可选）</label>
            <input
              type="text"
              value={group}
              onChange={(e) => onGroupChange(e.target.value)}
              placeholder="输入或选择分组"
              list="filter-groups"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
            />
            <datalist id="filter-groups">
              {groups
                .filter((g) => g !== "预设")
                .map((g) => (
                  <option key={g} value={g} />
                ))}
            </datalist>
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
            disabled={creating || !name.trim() || !filterExpr.trim()}
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
// 编辑过滤器对话框
// ============================================================================

interface EditFilterDialogProps {
  filter: QuickFilter;
  name: string;
  filterExpr: string;
  description: string;
  group: string;
  groups: string[];
  saving: boolean;
  onNameChange: (name: string) => void;
  onFilterExprChange: (expr: string) => void;
  onDescriptionChange: (description: string) => void;
  onGroupChange: (group: string) => void;
  onSave: () => void;
  onClose: () => void;
}

function EditFilterDialog({
  filter,
  name,
  filterExpr,
  description,
  group,
  groups,
  saving,
  onNameChange,
  onFilterExprChange,
  onDescriptionChange,
  onGroupChange,
  onSave,
  onClose,
}: EditFilterDialogProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />
      <div className="relative bg-card rounded-lg shadow-xl w-full max-w-md mx-4">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <div className="flex items-center gap-2">
            <Edit2 className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">编辑快速过滤器</h2>
          </div>
          <button onClick={onClose} className="p-1 rounded hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-4 space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">名称 *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder="输入过滤器名称"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">过滤表达式 *</label>
            <input
              type="text"
              value={filterExpr}
              onChange={(e) => onFilterExprChange(e.target.value)}
              placeholder="例如: ~e 或 ~p kiro & ~m claude"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm font-mono"
            />
            <p className="text-xs text-muted-foreground">
              支持 ~m, ~p, ~s, ~e, ~t, ~k, ~b, ~starred, ~tag, ~tokens, ~latency
              等过滤器
            </p>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">描述（可选）</label>
            <textarea
              value={description}
              onChange={(e) => onDescriptionChange(e.target.value)}
              placeholder="输入过滤器描述"
              rows={2}
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm resize-none"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">分组（可选）</label>
            <input
              type="text"
              value={group}
              onChange={(e) => onGroupChange(e.target.value)}
              placeholder="输入或选择分组"
              list="edit-filter-groups"
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
            />
            <datalist id="edit-filter-groups">
              {groups
                .filter((g) => g !== "预设")
                .map((g) => (
                  <option key={g} value={g} />
                ))}
            </datalist>
          </div>
          <div className="text-xs text-muted-foreground">
            <p>过滤器 ID: {filter.id.slice(0, 8)}...</p>
            <p>
              创建时间: {new Date(filter.created_at).toLocaleString("zh-CN")}
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
            disabled={saving || !name.trim() || !filterExpr.trim()}
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

export default QuickFilterPanel;
