import { useState, forwardRef, useImperativeHandle } from "react";
import {
  RefreshCw,
  Plus,
  Heart,
  HeartOff,
  RotateCcw,
  Activity,
} from "lucide-react";
import { useProviderPool } from "@/hooks/useProviderPool";
import { CredentialCard } from "./CredentialCard";
import { AddCredentialModal } from "./AddCredentialModal";
import { EditCredentialModal } from "./EditCredentialModal";
import { ErrorDisplay, useErrorDisplay } from "./ErrorDisplay";
import type {
  PoolProviderType,
  CredentialDisplay,
  UpdateCredentialRequest,
} from "@/lib/api/providerPool";

export interface ProviderPoolPageRef {
  refresh: () => void;
}

// All provider types
const allProviderTypes: PoolProviderType[] = [
  "kiro",
  "gemini",
  "qwen",
  "antigravity",
  "openai",
  "claude",
];

const providerLabels: Record<PoolProviderType, string> = {
  kiro: "Kiro (AWS)",
  gemini: "Gemini (Google)",
  qwen: "Qwen (阿里)",
  antigravity: "Antigravity (Gemini 3 Pro)",
  openai: "OpenAI",
  claude: "Claude (Anthropic)",
};

export const ProviderPoolPage = forwardRef<ProviderPoolPageRef>(
  (_props, ref) => {
    const [addModalOpen, setAddModalOpen] = useState(false);
    const [editModalOpen, setEditModalOpen] = useState(false);
    const [editingCredential, setEditingCredential] =
      useState<CredentialDisplay | null>(null);
    const [activeTab, setActiveTab] = useState<PoolProviderType>("kiro");
    const [deletingCredentials, setDeletingCredentials] = useState<Set<string>>(
      new Set(),
    );
    const { errors, showError, showSuccess, dismissError } = useErrorDisplay();

    const {
      overview,
      loading,
      error,
      checkingHealth,
      refreshingToken,
      refresh,
      deleteCredential,
      toggleCredential,
      resetCredential,
      resetHealth,
      checkCredentialHealth,
      checkTypeHealth,
      refreshCredentialToken,
      updateCredential,
    } = useProviderPool();

    useImperativeHandle(ref, () => ({
      refresh,
    }));

    const handleDelete = async (uuid: string) => {
      if (!confirm("确定要删除这个凭证吗？")) return;
      setDeletingCredentials((prev) => new Set(prev).add(uuid));
      try {
        await deleteCredential(uuid);
      } catch (e) {
        showError(e instanceof Error ? e.message : String(e), "delete", uuid);
      } finally {
        setDeletingCredentials((prev) => {
          const next = new Set(prev);
          next.delete(uuid);
          return next;
        });
      }
    };

    const handleToggle = async (credential: CredentialDisplay) => {
      try {
        await toggleCredential(credential.uuid, !credential.is_disabled);
      } catch (e) {
        showError(
          e instanceof Error ? e.message : String(e),
          "toggle",
          credential.uuid,
        );
      }
    };

    const handleReset = async (uuid: string) => {
      try {
        await resetCredential(uuid);
      } catch (e) {
        showError(e instanceof Error ? e.message : String(e), "reset", uuid);
      }
    };

    const handleCheckHealth = async (uuid: string) => {
      try {
        const result = await checkCredentialHealth(uuid);
        if (result.success) {
          showSuccess("健康检查通过！", uuid);
        } else {
          showError(result.message || "健康检查未通过", "health_check", uuid);
        }
      } catch (e) {
        showError(
          e instanceof Error ? e.message : String(e),
          "health_check",
          uuid,
        );
      }
    };

    const handleCheckTypeHealth = async (providerType: PoolProviderType) => {
      try {
        await checkTypeHealth(providerType);
      } catch (e) {
        showError(e instanceof Error ? e.message : String(e), "health_check");
      }
    };

    const handleResetTypeHealth = async (providerType: PoolProviderType) => {
      try {
        await resetHealth(providerType);
      } catch (e) {
        showError(e instanceof Error ? e.message : String(e), "reset");
      }
    };

    const handleRefreshToken = async (uuid: string) => {
      try {
        await refreshCredentialToken(uuid);
        showSuccess("Token 刷新成功！", uuid);
      } catch (e) {
        showError(
          e instanceof Error ? e.message : String(e),
          "refresh_token",
          uuid,
        );
      }
    };

    const handleEdit = (credential: CredentialDisplay) => {
      setEditingCredential(credential);
      setEditModalOpen(true);
    };

    const handleEditSubmit = async (
      uuid: string,
      request: UpdateCredentialRequest,
    ) => {
      try {
        await updateCredential(uuid, request);
      } catch (e) {
        throw new Error(
          `编辑失败: ${e instanceof Error ? e.message : String(e)}`,
        );
      }
    };

    const closeEditModal = () => {
      setEditModalOpen(false);
      setEditingCredential(null);
    };

    const openAddModal = () => {
      setAddModalOpen(true);
    };

    const getProviderOverview = (providerType: PoolProviderType) => {
      return overview.find((p) => p.provider_type === providerType);
    };

    const getCredentialCount = (providerType: PoolProviderType) => {
      const pool = getProviderOverview(providerType);
      return pool?.credentials?.length || 0;
    };

    // Current tab data
    const currentPool = getProviderOverview(activeTab);
    const currentStats = currentPool?.stats;
    const currentCredentials = currentPool?.credentials || [];

    return (
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold">凭证池</h2>
            <p className="text-muted-foreground">
              管理多个凭证，支持负载均衡和健康检测
            </p>
          </div>
          <button
            onClick={refresh}
            disabled={loading}
            className="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm hover:bg-muted disabled:opacity-50"
          >
            <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
            刷新
          </button>
        </div>

        {error && (
          <div className="rounded-lg border border-red-500 bg-red-50 p-4 text-red-700 dark:bg-red-950/30">
            {error}
          </div>
        )}

        {/* Tabs */}
        <div className="flex gap-2 border-b overflow-x-auto">
          {allProviderTypes.map((providerType) => {
            const count = getCredentialCount(providerType);
            return (
              <button
                key={providerType}
                onClick={() => setActiveTab(providerType)}
                className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px whitespace-nowrap flex items-center gap-2 ${
                  activeTab === providerType
                    ? "border-primary text-primary"
                    : "border-transparent text-muted-foreground hover:text-foreground"
                }`}
              >
                {providerLabels[providerType]}
                {count > 0 && (
                  <span className="rounded-full bg-muted px-1.5 py-0.5 text-xs">
                    {count}
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <div className="space-y-4">
            {/* Stats and Actions Bar */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-4">
                {currentStats && currentStats.total > 0 && (
                  <div className="flex items-center gap-3 text-sm text-muted-foreground">
                    <span className="flex items-center gap-1">
                      <Heart className="h-4 w-4 text-green-500" />
                      健康: {currentStats.healthy}
                    </span>
                    <span className="flex items-center gap-1">
                      <HeartOff className="h-4 w-4 text-red-500" />
                      不健康: {currentStats.unhealthy}
                    </span>
                    <span>总计: {currentStats.total}</span>
                  </div>
                )}
              </div>
              <div className="flex items-center gap-2">
                {currentCredentials.length > 0 && (
                  <>
                    <button
                      onClick={() => handleCheckTypeHealth(activeTab)}
                      disabled={checkingHealth === activeTab}
                      className="flex items-center gap-1 rounded-lg border px-3 py-1.5 text-sm hover:bg-muted disabled:opacity-50"
                    >
                      <Activity
                        className={`h-4 w-4 ${checkingHealth === activeTab ? "animate-pulse" : ""}`}
                      />
                      检测全部
                    </button>
                    <button
                      onClick={() => handleResetTypeHealth(activeTab)}
                      className="flex items-center gap-1 rounded-lg border px-3 py-1.5 text-sm hover:bg-muted"
                    >
                      <RotateCcw className="h-4 w-4" />
                      重置状态
                    </button>
                  </>
                )}
                <button
                  onClick={openAddModal}
                  className="flex items-center gap-1 rounded-lg bg-primary px-3 py-1.5 text-sm text-primary-foreground hover:bg-primary/90"
                >
                  <Plus className="h-4 w-4" />
                  添加凭证
                </button>
              </div>
            </div>

            {/* Credentials List */}
            {currentCredentials.length === 0 ? (
              <div className="flex flex-col items-center justify-center rounded-lg border border-dashed py-12 text-muted-foreground">
                <p className="text-lg">暂无 {providerLabels[activeTab]} 凭证</p>
                <p className="mt-1 text-sm">点击上方"添加凭证"按钮添加</p>
                <button
                  onClick={openAddModal}
                  className="mt-4 flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90"
                >
                  <Plus className="h-4 w-4" />
                  添加第一个凭证
                </button>
              </div>
            ) : (
              <div className="flex flex-col gap-4">
                {currentCredentials.map((credential) => (
                  <CredentialCard
                    key={credential.uuid}
                    credential={credential}
                    onToggle={() => handleToggle(credential)}
                    onDelete={() => handleDelete(credential.uuid)}
                    onReset={() => handleReset(credential.uuid)}
                    onCheckHealth={() => handleCheckHealth(credential.uuid)}
                    onRefreshToken={() => handleRefreshToken(credential.uuid)}
                    onEdit={() => handleEdit(credential)}
                    deleting={deletingCredentials.has(credential.uuid)}
                    checkingHealth={checkingHealth === credential.uuid}
                    refreshingToken={refreshingToken === credential.uuid}
                  />
                ))}
              </div>
            )}
          </div>
        )}

        {/* Add Credential Modal */}
        {addModalOpen && (
          <AddCredentialModal
            providerType={activeTab}
            onClose={() => {
              setAddModalOpen(false);
            }}
            onSuccess={() => {
              setAddModalOpen(false);
              refresh();
            }}
          />
        )}

        {/* Edit Credential Modal */}
        <EditCredentialModal
          credential={editingCredential}
          isOpen={editModalOpen}
          onClose={closeEditModal}
          onEdit={handleEditSubmit}
        />

        {/* Error Display */}
        <ErrorDisplay
          errors={errors}
          onDismiss={dismissError}
          onRetry={(error) => {
            // 根据错误类型提供重试功能
            switch (error.type) {
              case "health_check":
                if (error.uuid) {
                  handleCheckHealth(error.uuid);
                }
                break;
              case "refresh_token":
                if (error.uuid) {
                  handleRefreshToken(error.uuid);
                }
                break;
              case "reset":
                if (error.uuid) {
                  handleReset(error.uuid);
                }
                break;
            }
            dismissError(error.id);
          }}
        />
      </div>
    );
  },
);

ProviderPoolPage.displayName = "ProviderPoolPage";
