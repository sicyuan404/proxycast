import { useState, useEffect } from "react";
import {
  X,
  Eye,
  EyeOff,
  Settings,
  Upload,
  CheckCircle,
  Ban,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CredentialDisplay,
  UpdateCredentialRequest,
  PoolProviderType,
} from "@/lib/api/providerPool";

interface EditCredentialModalProps {
  credential: CredentialDisplay | null;
  isOpen: boolean;
  onClose: () => void;
  onEdit: (uuid: string, request: UpdateCredentialRequest) => Promise<void>;
}

// 各 Provider 支持的模型列表 (参考 AIClient-2-API/src/provider-models.js)
const providerModels: Record<PoolProviderType, string[]> = {
  kiro: [
    "claude-opus-4-5",
    "claude-opus-4-5-20251101",
    "claude-haiku-4-5",
    "claude-sonnet-4-5",
    "claude-sonnet-4-5-20250929",
    "claude-sonnet-4-20250514",
    "claude-3-7-sonnet-20250219",
  ],
  gemini: [
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
    "gemini-2.5-pro",
    "gemini-2.5-pro-preview-06-05",
    "gemini-2.5-flash-preview-09-2025",
    "gemini-3-pro-preview",
  ],
  qwen: ["qwen3-coder-plus", "qwen3-coder-flash"],
  antigravity: [
    "gemini-3-pro-preview",
    "gemini-3-pro-image-preview",
    "gemini-2.5-computer-use-preview-10-2025",
    "gemini-claude-sonnet-4-5",
    "gemini-claude-sonnet-4-5-thinking",
  ],
  openai: [], // 自定义 API，无预设模型
  claude: [], // 自定义 API，无预设模型
};

export function EditCredentialModal({
  credential,
  isOpen,
  onClose,
  onEdit,
}: EditCredentialModalProps) {
  const [name, setName] = useState("");
  const [checkHealth, setCheckHealth] = useState(true);
  const [checkModelName, setCheckModelName] = useState("");
  const [notSupportedModels, setNotSupportedModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showCredentialDetails, setShowCredentialDetails] = useState(false);

  // 重新上传文件相关状态
  const [newCredFilePath, setNewCredFilePath] = useState("");
  const [newProjectId, setNewProjectId] = useState("");

  // 初始化表单数据
  useEffect(() => {
    if (credential) {
      setName(credential.name || "");
      setCheckHealth(credential.check_health);
      setCheckModelName(credential.check_model_name || "");
      setNotSupportedModels(credential.not_supported_models || []);
      setNewCredFilePath("");
      setNewProjectId("");
      setError(null);
    }
  }, [credential]);

  if (!isOpen || !credential) {
    return null;
  }

  const isOAuth = credential.credential_type.includes("oauth");

  // 获取当前 provider 类型
  const getProviderType = (): PoolProviderType => {
    if (credential.credential_type.includes("kiro")) return "kiro";
    if (credential.credential_type.includes("gemini")) return "gemini";
    if (credential.credential_type.includes("qwen")) return "qwen";
    if (credential.credential_type.includes("openai")) return "openai";
    if (credential.credential_type.includes("claude")) return "claude";
    return "kiro";
  };

  const currentProviderModels = providerModels[getProviderType()] || [];

  const handleSelectNewFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (selected) {
        setNewCredFilePath(selected as string);
      }
    } catch (e) {
      console.error("Failed to open file dialog:", e);
    }
  };

  const getMaskedCredentialInfo = () => {
    if (isOAuth) {
      const path = credential.display_credential;
      const parts = path.split("/");
      if (parts.length > 1) {
        const fileName = parts[parts.length - 1];
        const dirPath = parts.slice(0, -1).join("/");
        return `${dirPath}/***${fileName.slice(-8)}`;
      }
      return `***${path.slice(-12)}`;
    } else {
      return credential.display_credential;
    }
  };

  const toggleModelSupport = (model: string) => {
    setNotSupportedModels((prev) =>
      prev.includes(model) ? prev.filter((m) => m !== model) : [...prev, model],
    );
  };

  const handleSubmit = async () => {
    setLoading(true);
    setError(null);

    try {
      const updateRequest: UpdateCredentialRequest = {
        name: name.trim() || undefined,
        check_health: checkHealth,
        check_model_name: checkModelName.trim() || undefined,
        // 始终传递 not_supported_models，即使为空数组（用于清除选择）
        not_supported_models: notSupportedModels,
        new_creds_file_path: newCredFilePath.trim() || undefined,
        new_project_id: newProjectId.trim() || undefined,
      };

      await onEdit(credential.uuid, updateRequest);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="w-full max-w-2xl max-h-[85vh] rounded-lg bg-background shadow-xl flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between border-b pb-4 px-6 pt-6 shrink-0">
          <h3 className="text-lg font-semibold flex items-center gap-2">
            <Settings className="h-5 w-5" />
            编辑凭证
          </h3>
          <button onClick={onClose} className="rounded-lg p-1 hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content - Scrollable */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          <div className="space-y-5">
            {/* 名称 + 健康检查 */}
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="mb-1 block text-sm font-medium">
                  名称 (选填)
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="给这个凭证起个名字..."
                  className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
                />
              </div>
              <div>
                <label className="mb-1 block text-sm font-medium">
                  健康检查
                </label>
                <select
                  value={checkHealth ? "enabled" : "disabled"}
                  onChange={(e) => setCheckHealth(e.target.value === "enabled")}
                  className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
                >
                  <option value="enabled">启用</option>
                  <option value="disabled">禁用</option>
                </select>
              </div>
            </div>

            {/* 检查模型名称 */}
            <div>
              <label className="mb-1 block text-sm font-medium">
                检查模型名称 (选填)
              </label>
              <input
                type="text"
                value={checkModelName}
                onChange={(e) => setCheckModelName(e.target.value)}
                placeholder="用于健康检查的模型名称..."
                className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              />
            </div>

            {/* OAuth凭据文件路径 */}
            {isOAuth && (
              <div>
                <label className="mb-1 block text-sm font-medium">
                  OAuth凭据文件路径
                </label>
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={
                      showCredentialDetails
                        ? credential.display_credential
                        : getMaskedCredentialInfo()
                    }
                    readOnly
                    className="flex-1 rounded-lg border bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
                  />
                  <button
                    type="button"
                    onClick={() =>
                      setShowCredentialDetails(!showCredentialDetails)
                    }
                    className="rounded-lg border p-2 hover:bg-muted"
                    title={showCredentialDetails ? "隐藏" : "显示"}
                  >
                    {showCredentialDetails ? (
                      <EyeOff className="h-4 w-4" />
                    ) : (
                      <Eye className="h-4 w-4" />
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={handleSelectNewFile}
                    className="rounded-lg border p-2 hover:bg-muted"
                    title="上传新文件"
                  >
                    <Upload className="h-4 w-4" />
                  </button>
                </div>
                {newCredFilePath && (
                  <div className="mt-2 text-xs text-green-600 dark:text-green-400 flex items-center gap-1">
                    <CheckCircle className="h-3 w-3" />
                    新文件已选择: {newCredFilePath.split("/").pop()}
                  </div>
                )}
              </div>
            )}

            {/* Gemini Project ID */}
            {credential.credential_type === "gemini_oauth" &&
              newCredFilePath && (
                <div>
                  <label className="mb-1 block text-sm font-medium">
                    项目ID（可选）
                  </label>
                  <input
                    type="text"
                    value={newProjectId}
                    onChange={(e) => setNewProjectId(e.target.value)}
                    placeholder="留空保持当前项目ID..."
                    className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
                  />
                </div>
              )}

            {/* 不支持的模型 - Checkbox Grid */}
            <div>
              <div className="flex items-center gap-2 mb-3">
                <Ban className="h-4 w-4 text-muted-foreground" />
                <label className="text-sm font-medium">不支持的模型</label>
                <span className="text-xs text-muted-foreground">
                  选择此提供商不支持的模型，系统会自动排除这些模型
                </span>
              </div>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
                {currentProviderModels.map((model) => (
                  <label
                    key={model}
                    className={`flex items-center gap-2 rounded-lg border px-3 py-2 cursor-pointer transition-colors ${
                      notSupportedModels.includes(model)
                        ? "border-red-300 bg-red-50 dark:border-red-800 dark:bg-red-950/30"
                        : "border-border hover:bg-muted/50"
                    }`}
                  >
                    <input
                      type="checkbox"
                      checked={notSupportedModels.includes(model)}
                      onChange={() => toggleModelSupport(model)}
                      className="rounded border-gray-300"
                    />
                    <span className="text-sm truncate">{model}</span>
                  </label>
                ))}
              </div>
            </div>

            {/* 使用统计（只读） */}
            <div className="rounded-lg bg-muted/50 p-4">
              <label className="mb-3 block text-sm font-medium">使用统计</label>
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>
                  <span className="text-muted-foreground block text-xs">
                    使用次数
                  </span>
                  <span className="font-semibold">
                    {credential.usage_count}
                  </span>
                </div>
                <div>
                  <span className="text-muted-foreground block text-xs">
                    错误次数
                  </span>
                  <span className="font-semibold">
                    {credential.error_count}
                  </span>
                </div>
                <div>
                  <span className="text-muted-foreground block text-xs">
                    最后使用
                  </span>
                  <span className="text-xs">
                    {credential.last_used || "从未"}
                  </span>
                </div>
              </div>
            </div>

            {/* Error */}
            {error && (
              <div className="rounded-lg border border-red-500 bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950/30">
                {error}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="border-t px-6 py-4 flex justify-end gap-2 shrink-0">
          <button
            onClick={onClose}
            className="rounded-lg border px-4 py-2 text-sm hover:bg-muted"
          >
            取消
          </button>
          <button
            onClick={handleSubmit}
            disabled={loading}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "保存中..." : "保存更改"}
          </button>
        </div>
      </div>
    </div>
  );
}
