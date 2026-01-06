import { useState, useEffect } from "react";
import {
  Play,
  Copy,
  Check,
  ChevronDown,
  ChevronUp,
  RefreshCw,
} from "lucide-react";
import { LogsTab } from "./LogsTab";
import { RoutesTab } from "./RoutesTab";
import { ProviderIcon } from "@/icons/providers";
import {
  startServer,
  stopServer,
  getServerStatus,
  getConfig,
  saveConfig,
  reloadCredentials,
  testApi,
  ServerStatus,
  Config,
  TestResult,
  getDefaultProvider,
  setDefaultProvider,
  getNetworkInfo,
  NetworkInfo,
} from "@/hooks/useTauri";
import { providerPoolApi, ProviderPoolOverview } from "@/lib/api/providerPool";
import {
  apiKeyProviderApi,
  ProviderWithKeysDisplay,
} from "@/lib/api/apiKeyProvider";

interface TestState {
  endpoint: string;
  status: "idle" | "loading" | "success" | "error";
  response?: string;
  time?: number;
  httpStatus?: number;
}

type TabId = "server" | "routes" | "logs";

// 可用的 Provider 信息（合并 OAuth 凭证池和 API Key Provider）
interface AvailableProvider {
  id: string;
  label: string;
  iconType: string;
  source: "oauth" | "api_key" | "both";
  oauthCount: number;
  apiKeyCount: number;
  totalCount: number;
}

export function ApiServerPage() {
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [loading, setLoading] = useState(false);
  const [_error, setError] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, TestState>>({});
  const [copiedCmd, setCopiedCmd] = useState<string | null>(null);
  const [expandedTest, setExpandedTest] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("server");

  // Config editing
  const [editPort, setEditPort] = useState<string>("");
  const [editApiKey, setEditApiKey] = useState<string>("");
  const [defaultProvider, setDefaultProviderState] = useState<string>("kiro");

  const [message, setMessage] = useState<{
    type: "success" | "error";
    text: string;
  } | null>(null);

  // 网络信息
  const [networkInfo, setNetworkInfo] = useState<NetworkInfo | null>(null);

  // 自动清除消息
  useEffect(() => {
    if (message) {
      const timer = setTimeout(() => {
        setMessage(null);
      }, 5000);
      return () => clearTimeout(timer);
    }
  }, [message]);

  const fetchStatus = async () => {
    try {
      const s = await getServerStatus();
      setStatus(s);
    } catch (e) {
      console.error(e);
    }
  };

  const fetchConfig = async () => {
    try {
      const c = await getConfig();
      setConfig(c);
      setEditPort(c.server.port.toString());
      setEditApiKey(c.server.api_key);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    fetchStatus();
    fetchConfig();
    loadDefaultProvider();
    loadNetworkInfo();

    const statusInterval = setInterval(fetchStatus, 3000);
    return () => clearInterval(statusInterval);
  }, []);

  const loadNetworkInfo = async () => {
    try {
      const info = await getNetworkInfo();
      setNetworkInfo(info);
    } catch (e) {
      console.error("Failed to get network info:", e);
    }
  };

  const loadDefaultProvider = async () => {
    try {
      const dp = await getDefaultProvider();
      setDefaultProviderState(dp);
    } catch (e) {
      console.error("Failed to get default provider:", e);
    }
  };

  const handleStart = async () => {
    setLoading(true);
    setError(null);
    try {
      await reloadCredentials();
      await startServer();
      await fetchStatus();
      setMessage({ type: "success", text: "服务已启动" });
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setError(errMsg);
      setMessage({ type: "error", text: `启动失败: ${errMsg}` });
    }
    setLoading(false);
  };

  const handleStop = async () => {
    setLoading(true);
    try {
      await stopServer();
      await fetchStatus();
      setMessage({ type: "success", text: "服务已停止" });
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setError(errMsg);
      setMessage({ type: "error", text: `停止失败: ${errMsg}` });
    }
    setLoading(false);
  };

  const handleSaveServerConfig = async () => {
    if (!config) return;
    setLoading(true);
    try {
      const newConfig = {
        ...config,
        server: {
          ...config.server,
          port: parseInt(editPort) || 8999,
          api_key: editApiKey,
        },
      };
      await saveConfig(newConfig);
      await fetchConfig();
      setMessage({ type: "success", text: "服务器配置已保存" });
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setMessage({ type: "error", text: `保存失败: ${errMsg}` });
    }
    setLoading(false);
  };

  const providerLabels: Record<string, string> = {
    // OAuth 凭证池类型
    kiro: "Kiro",
    gemini: "Gemini",
    qwen: "Qwen",
    antigravity: "Antigravity",
    claude: "Claude",
    codex: "Codex",
    iflow: "iFlow",
    claude_oauth: "Claude OAuth",
    vertex: "Vertex AI",
    gemini_api_key: "Gemini API Key",
    // API Key Provider 类型
    openai: "OpenAI",
    anthropic: "Anthropic",
    azure_openai: "Azure OpenAI",
    aws_bedrock: "AWS Bedrock",
    ollama: "Ollama",
  };

  // Provider ID 到图标类型的映射
  const providerIconMap: Record<string, string> = {
    // OAuth 凭证池类型
    kiro: "kiro",
    gemini: "gemini",
    qwen: "qwen",
    antigravity: "gemini",
    claude: "claude",
    codex: "openai",
    iflow: "iflow",
    claude_oauth: "claude",
    vertex: "gemini",
    gemini_api_key: "gemini",
    // API Key Provider 类型
    openai: "openai",
    anthropic: "claude",
    azure_openai: "openai",
    aws_bedrock: "claude",
    ollama: "ollama",
  };

  // 根据 Provider type 获取图标类型（用于自定义 Provider）
  const getIconTypeFromProviderType = (providerType: string): string => {
    const typeIconMap: Record<string, string> = {
      openai: "openai",
      anthropic: "claude",
      gemini: "gemini",
      "azure-openai": "openai",
      vertexai: "gemini",
      ollama: "ollama",
    };
    return typeIconMap[providerType.toLowerCase()] || "openai";
  };

  const [poolOverview, setPoolOverview] = useState<ProviderPoolOverview[]>([]);
  const [apiKeyProviders, setApiKeyProviders] = useState<
    ProviderWithKeysDisplay[]
  >([]);
  const [availableProviders, setAvailableProviders] = useState<
    AvailableProvider[]
  >([]);
  const [providerSwitchMsg, setProviderSwitchMsg] = useState<string | null>(
    null,
  );

  // 加载凭证池概览
  const loadPoolOverview = async () => {
    try {
      const overview = await providerPoolApi.getOverview();
      setPoolOverview(overview);
    } catch (e) {
      console.error("Failed to load pool overview:", e);
    }
  };

  // 加载 API Key Provider 数据
  const loadApiKeyProviders = async () => {
    try {
      const providers = await apiKeyProviderApi.getProviders();
      setApiKeyProviders(providers);
    } catch (e) {
      console.error("Failed to load API Key providers:", e);
    }
  };

  // 合并 OAuth 凭证池和 API Key Provider，生成可用 Provider 列表
  const buildAvailableProviders = () => {
    const providerMap = new Map<string, AvailableProvider>();

    // 添加 OAuth 凭证池中有凭证的 Provider
    poolOverview.forEach((overview) => {
      const enabledCredentials = overview.credentials.filter(
        (c) => !c.is_disabled,
      );
      if (enabledCredentials.length > 0) {
        const id = overview.provider_type;
        const existing = providerMap.get(id);
        if (existing) {
          existing.oauthCount = enabledCredentials.length;
          existing.totalCount = existing.oauthCount + existing.apiKeyCount;
          existing.source =
            existing.apiKeyCount > 0 && existing.oauthCount > 0
              ? "both"
              : "oauth";
        } else {
          providerMap.set(id, {
            id,
            label: providerLabels[id] || id,
            iconType: providerIconMap[id] || "openai",
            source: "oauth",
            oauthCount: enabledCredentials.length,
            apiKeyCount: 0,
            totalCount: enabledCredentials.length,
          });
        }
      }
    });

    // 添加 API Key Provider 中有 API Key 的 Provider
    // 使用 provider.id 作为 key，确保每个 Provider 单独显示
    apiKeyProviders.forEach((provider) => {
      const enabledKeys = provider.api_keys.filter((k) => k.enabled);
      if (enabledKeys.length > 0 && provider.enabled) {
        // 使用 provider.id 而不是 type 映射，确保自定义 Provider 单独显示
        const id = provider.id;
        const existing = providerMap.get(id);
        if (existing) {
          existing.apiKeyCount = enabledKeys.length;
          existing.totalCount = existing.oauthCount + existing.apiKeyCount;
          existing.source =
            existing.oauthCount > 0 && existing.apiKeyCount > 0
              ? "both"
              : "api_key";
        } else {
          // 根据 provider.type 确定图标类型（优先使用 id 映射，否则使用 type 映射）
          const iconType =
            providerIconMap[id] ||
            providerIconMap[provider.type] ||
            getIconTypeFromProviderType(provider.type);
          providerMap.set(id, {
            id,
            label: providerLabels[id] || provider.name,
            iconType,
            source: "api_key",
            oauthCount: 0,
            apiKeyCount: enabledKeys.length,
            totalCount: enabledKeys.length,
          });
        }
      }
    });

    // 转换为数组并按凭证数量排序
    const providers = Array.from(providerMap.values()).sort(
      (a, b) => b.totalCount - a.totalCount,
    );
    setAvailableProviders(providers);
  };

  // 将 API Key Provider 类型映射到 Provider ID
  // API Key Provider 类型直接使用自己的 ID，不合并到 OAuth 凭证池类型
  const mapApiKeyProviderToId = (providerType: string): string => {
    switch (providerType.toLowerCase()) {
      case "openai":
      case "openai-response":
        return "openai";
      case "anthropic":
        return "anthropic";
      case "gemini":
        return "gemini_api_key";
      case "azure-openai":
        return "azure_openai";
      case "vertexai":
        return "vertex";
      case "aws-bedrock":
        return "aws_bedrock";
      case "ollama":
        return "ollama";
      default:
        return providerType.toLowerCase();
    }
  };

  // 当 poolOverview 或 apiKeyProviders 变化时，重新构建可用 Provider 列表
  useEffect(() => {
    buildAvailableProviders();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [poolOverview, apiKeyProviders]);

  useEffect(() => {
    loadPoolOverview();
    loadApiKeyProviders();
    // 定时刷新凭证池数据，以便使用次数能够更新
    const poolInterval = setInterval(() => {
      loadPoolOverview();
      loadApiKeyProviders();
    }, 5000);
    return () => clearInterval(poolInterval);
  }, []);

  // 自动清除 provider 切换提示
  useEffect(() => {
    if (providerSwitchMsg) {
      const timer = setTimeout(() => setProviderSwitchMsg(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [providerSwitchMsg]);

  const handleSetDefaultProvider = async (providerId: string) => {
    try {
      await setDefaultProvider(providerId);
      setDefaultProviderState(providerId);

      // 获取最新的凭证池数据
      const freshOverview = await providerPoolApi.getOverview();
      setPoolOverview(freshOverview);

      // 获取该 Provider 的凭证信息
      const provider = availableProviders.find((p) => p.id === providerId);
      const label = providerLabels[providerId] || providerId;

      if (provider) {
        const parts = [];
        if (provider.oauthCount > 0) {
          parts.push(`${provider.oauthCount} OAuth`);
        }
        if (provider.apiKeyCount > 0) {
          parts.push(`${provider.apiKeyCount} API Key`);
        }
        setProviderSwitchMsg(
          `已切换到 ${label}` +
            (parts.length > 0 ? `（${parts.join(", ")}）` : ""),
        );
      } else {
        setProviderSwitchMsg(`已切换到 ${label}`);
      }
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setProviderSwitchMsg(`切换失败: ${errMsg}`);
    }
  };

  const serverUrl = status
    ? `http://${status.host}:${status.port}`
    : `http://localhost:${config?.server.port ?? 8999}`;
  const apiKey = config?.server.api_key ?? "";

  // 根据 Provider 类型获取测试模型
  const getTestModel = (provider: string): string => {
    switch (provider) {
      case "antigravity":
        return "gemini-3-pro-preview";
      case "gemini":
        return "gemini-2.0-flash";
      case "qwen":
        return "qwen-max";
      case "openai":
        return "gpt-4o";
      case "claude":
        return "claude-sonnet-4-20250514";
      case "kiro":
      default:
        return "claude-opus-4-5-20251101";
    }
  };

  const testModel = getTestModel(defaultProvider);

  // 根据 Provider 类型获取 Gemini 测试模型列表
  const getGeminiTestModels = (provider: string): string[] => {
    switch (provider) {
      case "antigravity":
        return [
          "gemini-3-pro-preview",
          "gemini-3-pro-image-preview",
          "gemini-3-flash-preview",
          "gemini-claude-sonnet-4-5",
        ];
      case "gemini":
        return ["gemini-2.0-flash", "gemini-2.5-flash", "gemini-2.5-pro"];
      default:
        return ["gemini-2.0-flash"];
    }
  };

  const geminiTestModels = getGeminiTestModels(defaultProvider);

  // 是否显示 Gemini 测试端点
  const showGeminiTest =
    defaultProvider === "antigravity" || defaultProvider === "gemini";

  // Test endpoints
  const testEndpoints = [
    {
      id: "health",
      name: "健康检查",
      method: "GET",
      path: "/health",
      needsAuth: false,
      body: null,
    },
    {
      id: "models",
      name: "模型列表",
      method: "GET",
      path: "/v1/models",
      needsAuth: true,
      body: null,
    },
    {
      id: "chat",
      name: "OpenAI Chat",
      method: "POST",
      path: "/v1/chat/completions",
      needsAuth: true,
      body: JSON.stringify({
        model: testModel,
        messages: [{ role: "user", content: "Say hi in one word" }],
      }),
    },
    {
      id: "anthropic",
      name: "Anthropic Messages",
      method: "POST",
      path: "/v1/messages",
      needsAuth: true,
      body: JSON.stringify({
        model: testModel,
        max_tokens: 100,
        messages: [
          {
            role: "user",
            content: "What is 1+1? Answer with just the number.",
          },
        ],
      }),
    },
    // Gemini 原生协议测试（仅在 Antigravity 或 Gemini Provider 时显示）
    ...(showGeminiTest
      ? geminiTestModels.map((model, index) => ({
          id: `gemini-${index}`,
          name: `Gemini ${model}`,
          method: "POST",
          path: `/v1/gemini/${model}:generateContent`,
          needsAuth: true,
          body: JSON.stringify({
            contents: [
              {
                role: "user",
                parts: [{ text: "What is 2+2? Answer with just the number." }],
              },
            ],
            generationConfig: {
              maxOutputTokens: 100,
            },
          }),
        }))
      : []),
  ];

  const runTest = async (endpoint: (typeof testEndpoints)[0]) => {
    setTestResults((prev) => ({
      ...prev,
      [endpoint.id]: { endpoint: endpoint.path, status: "loading" },
    }));

    try {
      const result: TestResult = await testApi(
        endpoint.method,
        endpoint.path,
        endpoint.body,
        endpoint.needsAuth,
      );

      setTestResults((prev) => ({
        ...prev,
        [endpoint.id]: {
          endpoint: endpoint.path,
          status: result.success ? "success" : "error",
          response: result.body || `HTTP ${result.status}: 无响应内容`,
          time: result.time_ms,
          httpStatus: result.status,
        },
      }));

      // 测试成功后立即刷新凭证池数据，更新使用次数
      if (result.success) {
        await loadPoolOverview();
      }
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setTestResults((prev) => ({
        ...prev,
        [endpoint.id]: {
          endpoint: endpoint.path,
          status: "error",
          response: `请求失败: ${errMsg}`,
        },
      }));
    }
  };

  const runAllTests = async () => {
    for (const endpoint of testEndpoints) {
      await runTest(endpoint);
    }
  };

  const getCurlCommand = (endpoint: (typeof testEndpoints)[0]) => {
    let cmd = `curl -s ${serverUrl}${endpoint.path}`;
    if (endpoint.needsAuth) {
      cmd += ` \\\n  -H "Authorization: Bearer ${apiKey}"`;
    }
    if (endpoint.body) {
      cmd += ` \\\n  -H "Content-Type: application/json"`;
      cmd += ` \\\n  -d '${endpoint.body}'`;
    }
    return cmd;
  };

  const copyCommand = (id: string, cmd: string) => {
    navigator.clipboard.writeText(cmd);
    setCopiedCmd(id);
    setTimeout(() => setCopiedCmd(null), 2000);
  };

  const getStatusBadge = (result?: TestState) => {
    if (!result || result.status === "idle") {
      return <span className="text-xs text-gray-400">未测试</span>;
    }
    if (result.status === "loading") {
      return <span className="text-xs text-blue-500">测试中...</span>;
    }
    if (result.status === "success") {
      return <span className="text-xs text-green-600">{result.time}ms</span>;
    }
    return (
      <span className="text-xs text-red-500">
        失败 {result.httpStatus ? `(${result.httpStatus})` : ""}
      </span>
    );
  };

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <h2 className="text-2xl font-bold">API Server</h2>
          <p className="text-muted-foreground text-sm">
            本地代理服务器，支持 OpenAI/Anthropic 格式
            {networkInfo && (
              <>
                {" "}
                <code className="px-1 py-0.5 rounded bg-muted text-xs">
                  {networkInfo.localhost}:{config?.server.port ?? 8999}
                </code>
                {networkInfo.lan_ip && (
                  <>
                    {" | "}
                    <code className="px-1 py-0.5 rounded bg-muted text-xs">
                      {networkInfo.lan_ip}:{config?.server.port ?? 8999}
                    </code>
                    <span className="text-xs"> (局域网)</span>
                  </>
                )}
              </>
            )}
            <span className="ml-2">
              <span
                className={`inline-block h-2 w-2 rounded-full ${status?.running ? "bg-green-500" : "bg-red-500"}`}
              />{" "}
              {status?.running ? "运行中" : "已停止"}
              {" · "}
              {status?.requests || 0} 请求
              {" · "}
              <span className="capitalize">{defaultProvider}</span>
            </span>
          </p>
        </div>
      </div>

      {message && (
        <div
          className={`flex items-center gap-2 rounded-lg border p-3 text-sm ${
            message.type === "success"
              ? "border-green-500 bg-green-50 text-green-700 dark:bg-green-950/30"
              : "border-red-500 bg-red-50 text-red-700 dark:bg-red-950/30"
          }`}
        >
          {message.type === "success" ? (
            <Check className="h-4 w-4" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
          {message.text}
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-2 border-b overflow-x-auto">
        {[
          { id: "server" as TabId, name: "服务器控制" },
          { id: "routes" as TabId, name: "路由端点" },
          { id: "logs" as TabId, name: "系统日志" },
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px whitespace-nowrap ${
              activeTab === tab.id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground"
            }`}
          >
            {tab.name}
          </button>
        ))}
      </div>

      {/* Server Control Tab */}
      {activeTab === "server" && (
        <div className="space-y-4">
          {/* Server Control - 紧凑版 */}
          <div className="rounded-lg border bg-card p-4">
            <div className="flex items-center gap-4">
              <button
                className={`rounded-lg px-4 py-1.5 text-sm font-medium text-white disabled:opacity-50 ${
                  status?.running
                    ? "bg-red-600 hover:bg-red-700"
                    : "bg-green-600 hover:bg-green-700"
                }`}
                onClick={status?.running ? handleStop : handleStart}
                disabled={loading}
              >
                {loading
                  ? "处理中..."
                  : status?.running
                    ? "停止服务"
                    : "启动服务"}
              </button>
              <div className="flex items-center gap-3 text-sm">
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground">端口:</span>
                  <input
                    type="number"
                    value={editPort}
                    onChange={(e) => setEditPort(e.target.value)}
                    className="w-20 rounded border bg-background px-2 py-1 text-sm"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground">API Key:</span>
                  <input
                    type="text"
                    value={editApiKey}
                    onChange={(e) => setEditApiKey(e.target.value)}
                    className="w-40 rounded border bg-background px-2 py-1 text-sm"
                  />
                </div>
                <button
                  onClick={handleSaveServerConfig}
                  disabled={loading}
                  className="rounded border px-3 py-1 text-sm hover:bg-muted disabled:opacity-50"
                >
                  保存
                </button>
              </div>
            </div>
          </div>

          {/* Default Provider - 动态显示有凭证的 Provider */}
          <div className="rounded-lg border bg-card p-4">
            <div className="flex items-center justify-between mb-3">
              <span className="font-medium text-sm">默认 Provider</span>
              {providerSwitchMsg && (
                <span className="text-xs text-green-600 flex items-center gap-1">
                  <Check className="h-3 w-3" />
                  {providerSwitchMsg}
                </span>
              )}
            </div>

            {availableProviders.length === 0 ? (
              <div className="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground">
                暂无可用凭证，请先在凭证池或 API Key 设置中添加凭证
              </div>
            ) : (
              <div className="flex flex-wrap gap-2">
                {availableProviders.map((p) => (
                  <button
                    key={p.id}
                    onClick={() => handleSetDefaultProvider(p.id)}
                    disabled={loading}
                    className={`flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-sm transition-colors ${
                      defaultProvider === p.id
                        ? "border-primary bg-primary/10 text-primary"
                        : "border-border bg-card hover:bg-muted text-muted-foreground hover:text-foreground"
                    } disabled:opacity-50`}
                  >
                    <ProviderIcon
                      providerType={
                        p.iconType as Parameters<
                          typeof ProviderIcon
                        >[0]["providerType"]
                      }
                      size={14}
                    />
                    {p.label}
                    <span className="text-xs opacity-70">
                      ({p.totalCount}
                      {p.source === "both" && (
                        <span className="ml-0.5">混合</span>
                      )}
                      {p.source === "api_key" && (
                        <span className="ml-0.5">Key</span>
                      )}
                      )
                    </span>
                  </button>
                ))}
              </div>
            )}

            {/* 当前选中类型的凭证列表 */}
            {(() => {
              const _currentProvider = availableProviders.find(
                (p) => p.id === defaultProvider,
              );
              const currentOverview = poolOverview.find(
                (o) => o.provider_type === defaultProvider,
              );
              const oauthCredentials = (
                currentOverview?.credentials || []
              ).filter((cred) => !cred.is_disabled);

              // 获取 API Key 凭证 - 查找所有映射到当前 defaultProvider 的 API Key Provider
              // 支持两种匹配方式：
              // 1. 通过 provider.id 直接匹配（用于自定义 Provider）
              // 2. 通过 type 映射匹配（用于内置 Provider）
              const matchingApiKeyProviders = apiKeyProviders.filter((p) => {
                // 首先尝试直接通过 id 匹配
                if (p.id === defaultProvider && p.enabled) {
                  return true;
                }
                // 然后尝试通过 type 映射匹配
                const mappedId = mapApiKeyProviderToId(p.type);
                return mappedId === defaultProvider && p.enabled;
              });
              const apiKeys = matchingApiKeyProviders.flatMap((p) =>
                p.api_keys.filter((k) => k.enabled),
              );

              if (oauthCredentials.length === 0 && apiKeys.length === 0) {
                // 只有当没有任何凭证时才显示提示
                return (
                  <div className="mt-4 rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground">
                    当前类型无可用凭证，请先在凭证池中添加
                  </div>
                );
              }

              return (
                <div className="mt-4 space-y-3">
                  {/* OAuth 凭证 */}
                  {oauthCredentials.length > 0 && (
                    <div className="space-y-2">
                      <p className="text-xs text-muted-foreground">
                        OAuth 凭证 ({oauthCredentials.length}):
                      </p>
                      <div className="space-y-1">
                        {oauthCredentials.map((cred) => (
                          <div
                            key={cred.uuid}
                            className="flex items-center justify-between rounded-lg border border-border bg-muted/30 px-3 py-2 text-sm"
                          >
                            <div className="flex items-center gap-2">
                              <span
                                className={`h-2 w-2 rounded-full ${
                                  cred.is_healthy
                                    ? "bg-green-500"
                                    : "bg-yellow-500"
                                }`}
                              />
                              <span>{cred.name || cred.uuid.slice(0, 8)}</span>
                              <span className="text-xs px-1.5 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
                                OAuth
                              </span>
                            </div>
                            <span className="text-xs text-muted-foreground">
                              使用 {cred.usage_count} 次
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* API Key 凭证 */}
                  {apiKeys.length > 0 && (
                    <div className="space-y-2">
                      <p className="text-xs text-muted-foreground">
                        API Key ({apiKeys.length}):
                      </p>
                      <div className="space-y-1">
                        {apiKeys.map((key) => (
                          <div
                            key={key.id}
                            className="flex items-center justify-between rounded-lg border border-border bg-muted/30 px-3 py-2 text-sm"
                          >
                            <div className="flex items-center gap-2">
                              <span className="h-2 w-2 rounded-full bg-green-500" />
                              <span>{key.alias || key.api_key_masked}</span>
                              <span className="text-xs px-1.5 py-0.5 rounded bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400">
                                API Key
                              </span>
                            </div>
                            <span className="text-xs text-muted-foreground">
                              使用 {key.usage_count} 次
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              );
            })()}
          </div>

          {/* API Testing */}
          <div className="rounded-lg border bg-card p-6">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="font-semibold">API 测试</h3>
              <button
                onClick={runAllTests}
                disabled={!status?.running}
                className="flex items-center gap-2 rounded-lg bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
              >
                <Play className="h-4 w-4" />
                测试全部
              </button>
            </div>

            <div className="space-y-3">
              {testEndpoints.map((endpoint) => {
                const result = testResults[endpoint.id];
                const isExpanded = expandedTest === endpoint.id;
                const curlCmd = getCurlCommand(endpoint);

                return (
                  <div
                    key={endpoint.id}
                    className="rounded-lg border bg-background"
                  >
                    <div className="flex items-center justify-between p-3">
                      <div className="flex items-center gap-3">
                        <span
                          className={`rounded px-2 py-0.5 text-xs font-medium ${
                            endpoint.method === "GET"
                              ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
                              : "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
                          }`}
                        >
                          {endpoint.method}
                        </span>
                        <span className="font-medium">{endpoint.name}</span>
                        <code className="text-xs text-muted-foreground">
                          {endpoint.path}
                        </code>
                        {getStatusBadge(result)}
                      </div>
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => copyCommand(endpoint.id, curlCmd)}
                          className="rounded p-1.5 hover:bg-muted"
                          title="复制 curl 命令"
                        >
                          {copiedCmd === endpoint.id ? (
                            <Check className="h-4 w-4 text-green-500" />
                          ) : (
                            <Copy className="h-4 w-4" />
                          )}
                        </button>
                        <button
                          onClick={() => runTest(endpoint)}
                          disabled={
                            !status?.running || result?.status === "loading"
                          }
                          className="rounded bg-primary/10 px-2 py-1 text-xs font-medium text-primary hover:bg-primary/20 disabled:opacity-50"
                        >
                          测试
                        </button>
                        <button
                          onClick={() =>
                            setExpandedTest(isExpanded ? null : endpoint.id)
                          }
                          className="rounded p-1.5 hover:bg-muted"
                        >
                          {isExpanded ? (
                            <ChevronUp className="h-4 w-4" />
                          ) : (
                            <ChevronDown className="h-4 w-4" />
                          )}
                        </button>
                      </div>
                    </div>

                    {isExpanded && (
                      <div className="border-t p-3 space-y-3">
                        <div>
                          <p className="mb-1 text-xs font-medium text-muted-foreground">
                            curl 命令
                          </p>
                          <pre className="rounded bg-muted p-2 text-xs overflow-x-auto">
                            {curlCmd}
                          </pre>
                        </div>
                        {result?.response && (
                          <div>
                            <p className="mb-1 text-xs font-medium text-muted-foreground">
                              响应{" "}
                              {result.httpStatus &&
                                `(HTTP ${result.httpStatus})`}
                            </p>
                            <pre
                              className={`rounded p-2 text-xs overflow-x-auto max-h-40 ${
                                result.status === "success"
                                  ? "bg-green-50 dark:bg-green-950/30"
                                  : "bg-red-50 dark:bg-red-950/30"
                              }`}
                            >
                              {(() => {
                                try {
                                  return JSON.stringify(
                                    JSON.parse(result.response),
                                    null,
                                    2,
                                  );
                                } catch {
                                  return result.response || "(空响应)";
                                }
                              })()}
                            </pre>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}

      {/* Routes Tab */}
      {activeTab === "routes" && <RoutesTab />}

      {/* Logs Tab */}
      {activeTab === "logs" && <LogsTab />}
    </div>
  );
}
