import { useState } from "react";
import { X, Key, FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { providerPoolApi, PoolProviderType } from "@/lib/api/providerPool";

interface AddCredentialModalProps {
  providerType: PoolProviderType;
  onClose: () => void;
  onSuccess: () => void;
}

// Default credential paths
const defaultCredsPath: Record<string, string> = {
  kiro: "~/.aws/sso/cache/kiro-auth-token.json",
  gemini: "~/.gemini/oauth_creds.json",
  qwen: "~/.qwen/oauth_creds.json",
  antigravity: "~/.antigravity/oauth_creds.json",
};

export function AddCredentialModal({
  providerType,
  onClose,
  onSuccess,
}: AddCredentialModalProps) {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // OAuth fields - initialize with default path
  const [credsFilePath, setCredsFilePath] = useState(
    defaultCredsPath[providerType] || "",
  );
  const [projectId, setProjectId] = useState("");

  // API Key fields
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");

  const isOAuth = ["kiro", "gemini", "qwen", "antigravity"].includes(
    providerType,
  );

  const providerLabels: Record<PoolProviderType, string> = {
    kiro: "Kiro (AWS)",
    gemini: "Gemini (Google)",
    qwen: "Qwen (阿里)",
    openai: "OpenAI",
    claude: "Claude (Anthropic)",
    antigravity: "Antigravity (Gemini 3 Pro)",
  };

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (selected) {
        setCredsFilePath(selected as string);
      }
    } catch (e) {
      console.error("Failed to open file dialog:", e);
    }
  };

  const handleSubmit = async () => {
    setLoading(true);
    setError(null);

    try {
      const trimmedName = name.trim() || undefined;

      if (isOAuth) {
        if (!credsFilePath) {
          setError("请选择凭证文件");
          return;
        }

        switch (providerType) {
          case "kiro":
            await providerPoolApi.addKiroOAuth(credsFilePath, trimmedName);
            break;
          case "gemini":
            await providerPoolApi.addGeminiOAuth(
              credsFilePath,
              projectId.trim() || undefined,
              trimmedName,
            );
            break;
          case "qwen":
            await providerPoolApi.addQwenOAuth(credsFilePath, trimmedName);
            break;
        }
      } else {
        if (!apiKey) {
          setError("请输入 API Key");
          return;
        }

        switch (providerType) {
          case "openai":
            await providerPoolApi.addOpenAIKey(
              apiKey,
              baseUrl.trim() || undefined,
              trimmedName,
            );
            break;
          case "claude":
            await providerPoolApi.addClaudeKey(
              apiKey,
              baseUrl.trim() || undefined,
              trimmedName,
            );
            break;
        }
      }

      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg bg-background p-6 shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b pb-4">
          <h3 className="text-lg font-semibold">
            添加 {providerLabels[providerType]} 凭证
          </h3>
          <button onClick={onClose} className="rounded-lg p-1 hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="mt-4 space-y-4">
          {/* Name field */}
          <div>
            <label className="mb-1 block text-sm font-medium">
              名称 (可选)
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="给这个凭证起个名字..."
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
            />
          </div>

          {isOAuth ? (
            <>
              {/* Credential File */}
              <div>
                <label className="mb-1 block text-sm font-medium">
                  凭证文件路径 <span className="text-red-500">*</span>
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={credsFilePath}
                    onChange={(e) => setCredsFilePath(e.target.value)}
                    placeholder="输入凭证文件的完整路径..."
                    className="flex-1 rounded-lg border bg-background px-3 py-2 text-sm"
                  />
                  <button
                    type="button"
                    onClick={handleSelectFile}
                    className="flex items-center gap-1 rounded-lg border px-3 py-2 text-sm hover:bg-muted"
                  >
                    <FolderOpen className="h-4 w-4" />
                    浏览
                  </button>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {providerType === "kiro" &&
                    "默认路径: ~/.aws/sso/cache/kiro-auth-token.json"}
                  {providerType === "gemini" &&
                    "默认路径: ~/.gemini/oauth_creds.json"}
                  {providerType === "qwen" &&
                    "默认路径: ~/.qwen/oauth_creds.json"}
                </p>
              </div>

              {/* Gemini specific: Project ID */}
              {providerType === "gemini" && (
                <div>
                  <label className="mb-1 block text-sm font-medium">
                    Project ID (可选)
                  </label>
                  <input
                    type="text"
                    value={projectId}
                    onChange={(e) => setProjectId(e.target.value)}
                    placeholder="Google Cloud Project ID..."
                    className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
                  />
                </div>
              )}
            </>
          ) : (
            <>
              {/* API Key */}
              <div>
                <label className="mb-1 block text-sm font-medium">
                  API Key <span className="text-red-500">*</span>
                </label>
                <div className="relative">
                  <Key className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <input
                    type="password"
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="sk-..."
                    className="w-full rounded-lg border bg-background pl-10 pr-3 py-2 text-sm"
                  />
                </div>
              </div>

              {/* Base URL */}
              <div>
                <label className="mb-1 block text-sm font-medium">
                  Base URL (可选)
                </label>
                <input
                  type="text"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder={
                    providerType === "openai"
                      ? "https://api.openai.com/v1"
                      : "https://api.anthropic.com/v1"
                  }
                  className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
                />
                <p className="mt-1 text-xs text-muted-foreground">
                  留空使用默认 URL，或输入自定义代理地址
                </p>
              </div>
            </>
          )}

          {/* Error */}
          {error && (
            <div className="rounded-lg border border-red-500 bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950/30">
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="mt-6 flex justify-end gap-2">
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
            {loading ? "添加中..." : "添加凭证"}
          </button>
        </div>
      </div>
    </div>
  );
}
