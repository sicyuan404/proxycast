import { useState, useEffect, useRef } from "react";
import {
  Check,
  X,
  RefreshCw,
  FolderOpen,
  AlertCircle,
  CheckCircle2,
  Eye,
  EyeOff,
  Copy,
  FileText,
} from "lucide-react";
import {
  reloadCredentials,
  refreshKiroToken,
  getKiroCredentials,
  getEnvVariables,
  getTokenFileHash,
  checkAndReloadCredentials,
  KiroCredentialStatus,
  EnvVariable,
  // Gemini
  getGeminiCredentials,
  reloadGeminiCredentials,
  refreshGeminiToken,
  getGeminiEnvVariables,
  getGeminiTokenFileHash,
  checkAndReloadGeminiCredentials,
  GeminiCredentialStatus,
  // Qwen
  getQwenCredentials,
  reloadQwenCredentials,
  refreshQwenToken,
  getQwenEnvVariables,
  getQwenTokenFileHash,
  checkAndReloadQwenCredentials,
  QwenCredentialStatus,
  // OpenAI/Claude Custom
  getOpenAICustomStatus,
  setOpenAICustomConfig,
  getClaudeCustomStatus,
  setClaudeCustomConfig,
  OpenAICustomStatus,
  ClaudeCustomStatus,
  // Default Provider
  getDefaultProvider,
  setDefaultProvider,
} from "@/hooks/useTauri";

interface Provider {
  id: string;
  name: string;
  enabled: boolean;
  status: "connected" | "disconnected" | "error" | "loading";
  description: string;
}

const defaultProviders: Provider[] = [
  {
    id: "kiro",
    name: "Kiro Claude",
    enabled: true,
    status: "disconnected",
    description: "通过 Kiro OAuth 访问 Claude Sonnet 4.5",
  },
  {
    id: "gemini",
    name: "Gemini CLI",
    enabled: true,
    status: "disconnected",
    description: "通过 Gemini CLI OAuth 访问 Gemini 模型",
  },
  {
    id: "qwen",
    name: "通义千问",
    enabled: true,
    status: "disconnected",
    description: "通过 Qwen OAuth 访问通义千问",
  },
  {
    id: "openai",
    name: "OpenAI 自定义",
    enabled: false,
    status: "disconnected",
    description: "自定义 OpenAI 兼容 API",
  },
  {
    id: "claude",
    name: "Claude 自定义",
    enabled: false,
    status: "disconnected",
    description: "自定义 Claude API",
  },
];

export function Providers() {
  const [providers, setProviders] = useState<Provider[]>(defaultProviders);
  const [activeProvider, setActiveProvider] = useState<string>("kiro");

  // Kiro state
  const [kiroStatus, setKiroStatus] = useState<KiroCredentialStatus | null>(
    null,
  );
  const [kiroEnvVars, setKiroEnvVars] = useState<EnvVariable[]>([]);
  const kiroHashRef = useRef<string>("");
  const [kiroLastSync, setKiroLastSync] = useState<Date | null>(null);

  // Gemini state
  const [geminiStatus, setGeminiStatus] =
    useState<GeminiCredentialStatus | null>(null);
  const [geminiEnvVars, setGeminiEnvVars] = useState<EnvVariable[]>([]);
  const geminiHashRef = useRef<string>("");
  const [geminiLastSync, setGeminiLastSync] = useState<Date | null>(null);

  // Qwen state
  const [qwenStatus, setQwenStatus] = useState<QwenCredentialStatus | null>(
    null,
  );
  const [qwenEnvVars, setQwenEnvVars] = useState<EnvVariable[]>([]);
  const qwenHashRef = useRef<string>("");
  const [qwenLastSync, setQwenLastSync] = useState<Date | null>(null);

  // Last check time (used for display)
  const [_lastCheckTime, setLastCheckTime] = useState<Date | null>(null);

  // OpenAI Custom state
  const [openaiStatus, setOpenaiStatus] = useState<OpenAICustomStatus | null>(
    null,
  );
  const [openaiApiKey, setOpenaiApiKey] = useState("");
  const [openaiBaseUrl, setOpenaiBaseUrl] = useState("");

  // Claude Custom state
  const [claudeStatus, setClaudeStatus] = useState<ClaudeCustomStatus | null>(
    null,
  );
  const [claudeApiKey, setClaudeApiKey] = useState("");
  const [claudeBaseUrl, setClaudeBaseUrl] = useState("");

  // Default provider state
  const [defaultProvider, setDefaultProviderState] = useState<string>("kiro");

  // Common state
  const [showEnv, setShowEnv] = useState(false);
  const [showValues, setShowValues] = useState(false);
  const [loading, setLoading] = useState<string | null>(null);
  const [message, setMessage] = useState<{
    type: "success" | "error";
    text: string;
  } | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    const init = async () => {
      // Load default provider
      try {
        const dp = await getDefaultProvider();
        setDefaultProviderState(dp);
      } catch (e) {
        console.error("Failed to get default provider:", e);
      }

      await loadKiroStatus();
      await loadGeminiStatus();
      await loadQwenStatus();
      await loadOpenAICustomStatus();
      await loadClaudeCustomStatus();
      try {
        kiroHashRef.current = await getTokenFileHash();
        geminiHashRef.current = await getGeminiTokenFileHash();
        qwenHashRef.current = await getQwenTokenFileHash();
      } catch (e) {
        console.error("Failed to get initial hash:", e);
      }
    };
    init();

    const interval = setInterval(checkFileChanges, 5000);
    return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const checkFileChanges = async () => {
    setLastCheckTime(new Date());

    // Check Kiro
    try {
      const kiroResult = await checkAndReloadCredentials(kiroHashRef.current);
      kiroHashRef.current = kiroResult.new_hash;
      if (kiroResult.changed && kiroResult.reloaded) {
        await loadKiroStatus();
        setKiroLastSync(new Date());
        setMessage({
          type: "success",
          text: "[Kiro] 检测到凭证文件变化，已自动重新加载",
        });
        setTimeout(() => setMessage(null), 5000);
      }
    } catch (e) {
      console.error("Kiro check error:", e);
    }

    // Check Gemini
    try {
      const geminiResult = await checkAndReloadGeminiCredentials(
        geminiHashRef.current,
      );
      geminiHashRef.current = geminiResult.new_hash;
      if (geminiResult.changed && geminiResult.reloaded) {
        await loadGeminiStatus();
        setGeminiLastSync(new Date());
        setMessage({
          type: "success",
          text: "[Gemini] 检测到凭证文件变化，已自动重新加载",
        });
        setTimeout(() => setMessage(null), 5000);
      }
    } catch (e) {
      console.error("Gemini check error:", e);
    }

    // Check Qwen
    try {
      const qwenResult = await checkAndReloadQwenCredentials(
        qwenHashRef.current,
      );
      qwenHashRef.current = qwenResult.new_hash;
      if (qwenResult.changed && qwenResult.reloaded) {
        await loadQwenStatus();
        setQwenLastSync(new Date());
        setMessage({
          type: "success",
          text: "[Qwen] 检测到凭证文件变化，已自动重新加载",
        });
        setTimeout(() => setMessage(null), 5000);
      }
    } catch (e) {
      console.error("Qwen check error:", e);
    }
  };

  const loadKiroStatus = async () => {
    try {
      const status = await getKiroCredentials();
      setKiroStatus(status);
      setKiroEnvVars(await getEnvVariables());
      setProviders((prev) =>
        prev.map((p) =>
          p.id === "kiro"
            ? { ...p, status: status.loaded ? "connected" : "disconnected" }
            : p,
        ),
      );
    } catch (e) {
      console.error("Failed to load Kiro status:", e);
    }
  };

  const loadGeminiStatus = async () => {
    try {
      const status = await getGeminiCredentials();
      setGeminiStatus(status);
      setGeminiEnvVars(await getGeminiEnvVariables());
      setProviders((prev) =>
        prev.map((p) =>
          p.id === "gemini"
            ? { ...p, status: status.loaded ? "connected" : "disconnected" }
            : p,
        ),
      );
    } catch (e) {
      console.error("Failed to load Gemini status:", e);
    }
  };

  const loadQwenStatus = async () => {
    try {
      const status = await getQwenCredentials();
      setQwenStatus(status);
      setQwenEnvVars(await getQwenEnvVariables());
      setProviders((prev) =>
        prev.map((p) =>
          p.id === "qwen"
            ? { ...p, status: status.loaded ? "connected" : "disconnected" }
            : p,
        ),
      );
    } catch (e) {
      console.error("Failed to load Qwen status:", e);
    }
  };

  const loadOpenAICustomStatus = async () => {
    try {
      const status = await getOpenAICustomStatus();
      setOpenaiStatus(status);
      setOpenaiBaseUrl(status.base_url);
      setProviders((prev) =>
        prev.map((p) =>
          p.id === "openai"
            ? {
                ...p,
                status:
                  status.enabled && status.has_api_key
                    ? "connected"
                    : "disconnected",
                enabled: status.enabled,
              }
            : p,
        ),
      );
    } catch (e) {
      console.error("Failed to load OpenAI Custom status:", e);
    }
  };

  const loadClaudeCustomStatus = async () => {
    try {
      const status = await getClaudeCustomStatus();
      setClaudeStatus(status);
      setClaudeBaseUrl(status.base_url);
      setProviders((prev) =>
        prev.map((p) =>
          p.id === "claude"
            ? {
                ...p,
                status:
                  status.enabled && status.has_api_key
                    ? "connected"
                    : "disconnected",
                enabled: status.enabled,
              }
            : p,
        ),
      );
    } catch (e) {
      console.error("Failed to load Claude Custom status:", e);
    }
  };

  const handleLoadCredentials = async (provider: string) => {
    setLoading(`load-${provider}`);
    setMessage(null);
    try {
      if (provider === "kiro") {
        await reloadCredentials();
        await loadKiroStatus();
        kiroHashRef.current = await getTokenFileHash();
        setKiroLastSync(new Date());
      } else if (provider === "gemini") {
        await reloadGeminiCredentials();
        await loadGeminiStatus();
        geminiHashRef.current = await getGeminiTokenFileHash();
        setGeminiLastSync(new Date());
      } else if (provider === "qwen") {
        await reloadQwenCredentials();
        await loadQwenStatus();
        qwenHashRef.current = await getQwenTokenFileHash();
        setQwenLastSync(new Date());
      }
      setMessage({ type: "success", text: `[${provider}] 凭证加载成功！` });
    } catch (e: any) {
      setMessage({ type: "error", text: `加载失败: ${e.toString()}` });
    }
    setLoading(null);
  };

  const handleRefreshToken = async (provider: string) => {
    setLoading(`refresh-${provider}`);
    setMessage(null);
    try {
      if (provider === "kiro") {
        await refreshKiroToken();
        await loadKiroStatus();
      } else if (provider === "gemini") {
        await refreshGeminiToken();
        await loadGeminiStatus();
      } else if (provider === "qwen") {
        await refreshQwenToken();
        await loadQwenStatus();
      }
      setMessage({ type: "success", text: `[${provider}] Token 刷新成功！` });
    } catch (e: any) {
      setMessage({ type: "error", text: `刷新失败: ${e.toString()}` });
    }
    setLoading(null);
  };

  const handleSaveOpenAIConfig = async () => {
    setLoading("save-openai");
    try {
      await setOpenAICustomConfig(
        openaiApiKey || null,
        openaiBaseUrl || null,
        true,
      );
      await loadOpenAICustomStatus();
      setMessage({ type: "success", text: "[OpenAI] 配置保存成功！" });
    } catch (e: any) {
      setMessage({ type: "error", text: `保存失败: ${e.toString()}` });
    }
    setLoading(null);
  };

  const handleSaveClaudeConfig = async () => {
    setLoading("save-claude");
    try {
      await setClaudeCustomConfig(
        claudeApiKey || null,
        claudeBaseUrl || null,
        true,
      );
      await loadClaudeCustomStatus();
      setMessage({ type: "success", text: "[Claude] 配置保存成功！" });
    } catch (e: any) {
      setMessage({ type: "error", text: `保存失败: ${e.toString()}` });
    }
    setLoading(null);
  };

  const toggleProvider = (id: string) => {
    setProviders((prev) =>
      prev.map((p) => (p.id === id ? { ...p, enabled: !p.enabled } : p)),
    );
  };

  const handleSetDefaultProvider = async (providerId: string) => {
    setLoading(`default-${providerId}`);
    try {
      await setDefaultProvider(providerId);
      setDefaultProviderState(providerId);
      setMessage({
        type: "success",
        text: `默认 Provider 已切换为: ${getProviderName(providerId)}`,
      });
    } catch (e: any) {
      setMessage({ type: "error", text: `切换失败: ${e.toString()}` });
    }
    setLoading(null);
  };

  const getProviderName = (id: string) => {
    switch (id) {
      case "kiro":
        return "Kiro Claude";
      case "gemini":
        return "Gemini CLI";
      case "qwen":
        return "通义千问";
      case "openai":
        return "OpenAI 自定义";
      case "claude":
        return "Claude 自定义";
      default:
        return id;
    }
  };

  const copyValue = (key: string, value: string) => {
    navigator.clipboard.writeText(value);
    setCopied(key);
    setTimeout(() => setCopied(null), 2000);
  };

  const copyAllEnv = (vars: EnvVariable[]) => {
    navigator.clipboard.writeText(
      vars.map((v) => `${v.key}=${v.value}`).join("\n"),
    );
    setCopied("all");
    setTimeout(() => setCopied(null), 2000);
  };

  const getStatusColor = (status: Provider["status"]) => {
    switch (status) {
      case "connected":
        return "bg-green-500";
      case "error":
        return "bg-red-500";
      case "loading":
        return "bg-yellow-500 animate-pulse";
      default:
        return "bg-gray-400";
    }
  };

  const formatTime = (date: Date | null) => {
    if (!date) return "从未同步";
    return date.toLocaleTimeString();
  };

  const currentEnvVars =
    activeProvider === "kiro"
      ? kiroEnvVars
      : activeProvider === "gemini"
        ? geminiEnvVars
        : qwenEnvVars;

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-bold">Provider 管理</h2>
        <p className="text-muted-foreground">配置和管理 AI 模型提供商</p>
      </div>

      {message && (
        <div
          className={`flex items-center gap-2 rounded-lg border p-3 text-sm ${
            message.type === "success"
              ? "border-green-500 bg-green-50 text-green-700"
              : "border-red-500 bg-red-50 text-red-700"
          }`}
        >
          {message.type === "success" ? (
            <CheckCircle2 className="h-4 w-4" />
          ) : (
            <AlertCircle className="h-4 w-4" />
          )}
          {message.text}
        </div>
      )}

      {/* Provider Tabs */}
      <div className="flex gap-2 border-b overflow-x-auto">
        {["kiro", "gemini", "qwen", "openai", "claude"].map((id) => (
          <button
            key={id}
            onClick={() => setActiveProvider(id)}
            className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px whitespace-nowrap ${
              activeProvider === id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground"
            }`}
          >
            {id === "kiro"
              ? "Kiro Claude"
              : id === "gemini"
                ? "Gemini CLI"
                : id === "qwen"
                  ? "通义千问"
                  : id === "openai"
                    ? "OpenAI 自定义"
                    : "Claude 自定义"}
          </button>
        ))}
      </div>

      {/* Kiro Panel */}
      {activeProvider === "kiro" && (
        <div className="rounded-lg border bg-card p-4">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">Kiro 凭证状态</h3>
            <div className="flex items-center gap-4 text-xs text-muted-foreground">
              <span>
                最后同步:{" "}
                <span className="text-foreground">
                  {formatTime(kiroLastSync)}
                </span>
              </span>
              <span className="flex items-center gap-1">
                <span className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                监测中 (5s)
              </span>
            </div>
          </div>
          <div className="mb-4 grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">凭证路径:</span>
              <code className="ml-2 rounded bg-muted px-2 py-0.5 text-xs break-all">
                {kiroStatus?.creds_path ||
                  "~/.aws/sso/cache/kiro-auth-token.json"}
              </code>
            </div>
            <div>
              <span className="text-muted-foreground">区域:</span>
              <span className="ml-2">{kiroStatus?.region || "未设置"}</span>
            </div>
            <div>
              <span className="text-muted-foreground">Access Token:</span>
              <span
                className={`ml-2 ${kiroStatus?.has_access_token ? "text-green-600" : "text-red-500"}`}
              >
                {kiroStatus?.has_access_token ? "✓ 已加载" : "✗ 未加载"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Refresh Token:</span>
              <span
                className={`ml-2 ${kiroStatus?.has_refresh_token ? "text-green-600" : "text-red-500"}`}
              >
                {kiroStatus?.has_refresh_token ? "✓ 已加载" : "✗ 未加载"}
              </span>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => handleLoadCredentials("kiro")}
              disabled={loading !== null}
              className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              <FolderOpen className="h-4 w-4" />
              {loading === "load-kiro" ? "加载中..." : "一键读取凭证"}
            </button>
            <button
              onClick={() => handleRefreshToken("kiro")}
              disabled={loading !== null || !kiroStatus?.has_refresh_token}
              className="flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
            >
              <RefreshCw
                className={`h-4 w-4 ${loading === "refresh-kiro" ? "animate-spin" : ""}`}
              />
              刷新 Token
            </button>
            <button
              onClick={() => setShowEnv(!showEnv)}
              className="flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium hover:bg-muted"
            >
              <FileText className="h-4 w-4" />
              {showEnv ? "隐藏" : "查看"} .env 变量
            </button>
          </div>
        </div>
      )}

      {/* Gemini Panel */}
      {activeProvider === "gemini" && (
        <div className="rounded-lg border bg-card p-4">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">Gemini CLI 凭证状态</h3>
            <div className="flex items-center gap-4 text-xs text-muted-foreground">
              <span>
                最后同步:{" "}
                <span className="text-foreground">
                  {formatTime(geminiLastSync)}
                </span>
              </span>
              <span className="flex items-center gap-1">
                <span className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                监测中 (5s)
              </span>
            </div>
          </div>
          <div className="mb-4 grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">凭证路径:</span>
              <code className="ml-2 rounded bg-muted px-2 py-0.5 text-xs break-all">
                {geminiStatus?.creds_path || "~/.gemini/oauth_creds.json"}
              </code>
            </div>
            <div>
              <span className="text-muted-foreground">Token 有效:</span>
              <span
                className={`ml-2 ${geminiStatus?.is_valid ? "text-green-600" : "text-red-500"}`}
              >
                {geminiStatus?.is_valid ? "✓ 有效" : "✗ 无效/过期"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Access Token:</span>
              <span
                className={`ml-2 ${geminiStatus?.has_access_token ? "text-green-600" : "text-red-500"}`}
              >
                {geminiStatus?.has_access_token ? "✓ 已加载" : "✗ 未加载"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Refresh Token:</span>
              <span
                className={`ml-2 ${geminiStatus?.has_refresh_token ? "text-green-600" : "text-red-500"}`}
              >
                {geminiStatus?.has_refresh_token ? "✓ 已加载" : "✗ 未加载"}
              </span>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => handleLoadCredentials("gemini")}
              disabled={loading !== null}
              className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              <FolderOpen className="h-4 w-4" />
              {loading === "load-gemini" ? "加载中..." : "一键读取凭证"}
            </button>
            <button
              onClick={() => handleRefreshToken("gemini")}
              disabled={loading !== null || !geminiStatus?.has_refresh_token}
              className="flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
            >
              <RefreshCw
                className={`h-4 w-4 ${loading === "refresh-gemini" ? "animate-spin" : ""}`}
              />
              刷新 Token
            </button>
            <button
              onClick={() => setShowEnv(!showEnv)}
              className="flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium hover:bg-muted"
            >
              <FileText className="h-4 w-4" />
              {showEnv ? "隐藏" : "查看"} .env 变量
            </button>
          </div>
        </div>
      )}

      {/* Qwen Panel */}
      {activeProvider === "qwen" && (
        <div className="rounded-lg border bg-card p-4">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">通义千问凭证状态</h3>
            <div className="flex items-center gap-4 text-xs text-muted-foreground">
              <span>
                最后同步:{" "}
                <span className="text-foreground">
                  {formatTime(qwenLastSync)}
                </span>
              </span>
              <span className="flex items-center gap-1">
                <span className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                监测中 (5s)
              </span>
            </div>
          </div>
          <div className="mb-4 grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">凭证路径:</span>
              <code className="ml-2 rounded bg-muted px-2 py-0.5 text-xs break-all">
                {qwenStatus?.creds_path || "~/.qwen/oauth_creds.json"}
              </code>
            </div>
            <div>
              <span className="text-muted-foreground">Token 有效:</span>
              <span
                className={`ml-2 ${qwenStatus?.is_valid ? "text-green-600" : "text-red-500"}`}
              >
                {qwenStatus?.is_valid ? "✓ 有效" : "✗ 无效/过期"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Access Token:</span>
              <span
                className={`ml-2 ${qwenStatus?.has_access_token ? "text-green-600" : "text-red-500"}`}
              >
                {qwenStatus?.has_access_token ? "✓ 已加载" : "✗ 未加载"}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Refresh Token:</span>
              <span
                className={`ml-2 ${qwenStatus?.has_refresh_token ? "text-green-600" : "text-red-500"}`}
              >
                {qwenStatus?.has_refresh_token ? "✓ 已加载" : "✗ 未加载"}
              </span>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => handleLoadCredentials("qwen")}
              disabled={loading !== null}
              className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              <FolderOpen className="h-4 w-4" />
              {loading === "load-qwen" ? "加载中..." : "一键读取凭证"}
            </button>
            <button
              onClick={() => handleRefreshToken("qwen")}
              disabled={loading !== null || !qwenStatus?.has_refresh_token}
              className="flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
            >
              <RefreshCw
                className={`h-4 w-4 ${loading === "refresh-qwen" ? "animate-spin" : ""}`}
              />
              刷新 Token
            </button>
            <button
              onClick={() => setShowEnv(!showEnv)}
              className="flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium hover:bg-muted"
            >
              <FileText className="h-4 w-4" />
              {showEnv ? "隐藏" : "查看"} .env 变量
            </button>
          </div>
        </div>
      )}

      {/* OpenAI Custom Panel */}
      {activeProvider === "openai" && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-3 font-semibold">OpenAI 自定义配置</h3>
          <div className="mb-4 space-y-4">
            <div>
              <label className="block text-sm text-muted-foreground mb-1">
                API Key
              </label>
              <input
                type="password"
                value={openaiApiKey}
                onChange={(e) => setOpenaiApiKey(e.target.value)}
                placeholder="sk-..."
                className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="block text-sm text-muted-foreground mb-1">
                Base URL
              </label>
              <input
                type="text"
                value={openaiBaseUrl}
                onChange={(e) => setOpenaiBaseUrl(e.target.value)}
                placeholder="https://api.openai.com/v1"
                className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              />
            </div>
            <div className="flex items-center gap-2 text-sm">
              <span className="text-muted-foreground">状态:</span>
              <span
                className={
                  openaiStatus?.has_api_key ? "text-green-600" : "text-red-500"
                }
              >
                {openaiStatus?.has_api_key ? "✓ 已配置" : "✗ 未配置"}
              </span>
            </div>
          </div>
          <button
            onClick={handleSaveOpenAIConfig}
            disabled={loading !== null}
            className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading === "save-openai" ? "保存中..." : "保存配置"}
          </button>
        </div>
      )}

      {/* Claude Custom Panel */}
      {activeProvider === "claude" && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-3 font-semibold">Claude 自定义配置</h3>
          <div className="mb-4 space-y-4">
            <div>
              <label className="block text-sm text-muted-foreground mb-1">
                API Key
              </label>
              <input
                type="password"
                value={claudeApiKey}
                onChange={(e) => setClaudeApiKey(e.target.value)}
                placeholder="sk-ant-..."
                className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="block text-sm text-muted-foreground mb-1">
                Base URL
              </label>
              <input
                type="text"
                value={claudeBaseUrl}
                onChange={(e) => setClaudeBaseUrl(e.target.value)}
                placeholder="https://api.anthropic.com"
                className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
              />
            </div>
            <div className="flex items-center gap-2 text-sm">
              <span className="text-muted-foreground">状态:</span>
              <span
                className={
                  claudeStatus?.has_api_key ? "text-green-600" : "text-red-500"
                }
              >
                {claudeStatus?.has_api_key ? "✓ 已配置" : "✗ 未配置"}
              </span>
            </div>
          </div>
          <button
            onClick={handleSaveClaudeConfig}
            disabled={loading !== null}
            className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading === "save-claude" ? "保存中..." : "保存配置"}
          </button>
        </div>
      )}

      {/* .env 变量展示 */}
      {showEnv && (
        <div className="rounded-lg border bg-card p-4">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">.env 环境变量 ({activeProvider})</h3>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setShowValues(!showValues)}
                className="flex items-center gap-1 rounded px-2 py-1 text-xs hover:bg-muted"
              >
                {showValues ? (
                  <EyeOff className="h-3 w-3" />
                ) : (
                  <Eye className="h-3 w-3" />
                )}
                {showValues ? "隐藏值" : "显示值"}
              </button>
              <button
                onClick={() => copyAllEnv(currentEnvVars)}
                className="flex items-center gap-1 rounded px-2 py-1 text-xs hover:bg-muted"
              >
                {copied === "all" ? (
                  <CheckCircle2 className="h-3 w-3 text-green-500" />
                ) : (
                  <Copy className="h-3 w-3" />
                )}
                复制全部
              </button>
            </div>
          </div>
          {currentEnvVars.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              暂无环境变量，请先加载凭证
            </p>
          ) : (
            <div className="space-y-2 font-mono text-sm">
              {currentEnvVars.map((v) => (
                <div
                  key={v.key}
                  className="flex items-center gap-2 rounded bg-muted p-2"
                >
                  <span className="text-blue-600 shrink-0">{v.key}</span>
                  <span>=</span>
                  <span className="flex-1 truncate text-muted-foreground">
                    {showValues ? v.value : v.masked}
                  </span>
                  <button
                    onClick={() => copyValue(v.key, v.value)}
                    className="rounded p-1 hover:bg-background shrink-0"
                  >
                    {copied === v.key ? (
                      <CheckCircle2 className="h-3 w-3 text-green-500" />
                    ) : (
                      <Copy className="h-3 w-3" />
                    )}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Provider 列表 */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold">Provider 列表</h3>
          <span className="text-sm text-muted-foreground">
            当前默认:{" "}
            <span className="font-medium text-primary">
              {getProviderName(defaultProvider)}
            </span>
          </span>
        </div>
        {providers.map((provider) => (
          <div
            key={provider.id}
            className={`flex items-center justify-between rounded-lg border bg-card p-4 transition-all ${
              defaultProvider === provider.id
                ? "border-primary ring-1 ring-primary"
                : ""
            }`}
          >
            <div className="flex items-center gap-4">
              <div
                className={`h-3 w-3 rounded-full ${getStatusColor(provider.status)}`}
              />
              <div>
                <div className="flex items-center gap-2">
                  <h3 className="font-medium">{provider.name}</h3>
                  {defaultProvider === provider.id && (
                    <span className="rounded bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
                      默认
                    </span>
                  )}
                </div>
                <p className="text-sm text-muted-foreground">
                  {provider.description}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {defaultProvider !== provider.id && (
                <button
                  onClick={() => handleSetDefaultProvider(provider.id)}
                  disabled={loading !== null}
                  className="rounded-lg border px-3 py-1.5 text-xs font-medium hover:bg-muted disabled:opacity-50"
                  title="设为默认"
                >
                  {loading === `default-${provider.id}`
                    ? "切换中..."
                    : "设为默认"}
                </button>
              )}
              {(provider.id === "kiro" ||
                provider.id === "gemini" ||
                provider.id === "qwen") && (
                <button
                  onClick={() => handleRefreshToken(provider.id)}
                  disabled={loading !== null}
                  className="rounded p-2 hover:bg-muted"
                  title="刷新 Token"
                >
                  <RefreshCw
                    className={`h-4 w-4 ${loading === `refresh-${provider.id}` ? "animate-spin" : ""}`}
                  />
                </button>
              )}
              <button
                onClick={() => toggleProvider(provider.id)}
                className={`rounded-full p-1 ${provider.enabled ? "bg-green-100 text-green-600" : "bg-gray-100 text-gray-400"}`}
              >
                {provider.enabled ? (
                  <Check className="h-4 w-4" />
                ) : (
                  <X className="h-4 w-4" />
                )}
              </button>
            </div>
          </div>
        ))}
      </div>

      <p className="text-xs text-muted-foreground">
        💡 提示：系统每 5
        秒自动检查凭证文件变化，如有更新会自动重新加载并记录日志
      </p>
    </div>
  );
}
