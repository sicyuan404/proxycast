import { useState, useEffect, useRef } from "react";
import {
  Activity,
  Server,
  Zap,
  Clock,
  Play,
  Copy,
  Check,
  ChevronDown,
  ChevronUp,
  RefreshCw,
} from "lucide-react";
import {
  startServer,
  stopServer,
  getServerStatus,
  getConfig,
  reloadCredentials,
  testApi,
  ServerStatus,
  Config,
  TestResult,
  getKiroCredentials,
  checkAndReloadCredentials,
  getTokenFileHash,
  KiroCredentialStatus,
} from "@/hooks/useTauri";

interface TestState {
  endpoint: string;
  status: "idle" | "loading" | "success" | "error";
  response?: string;
  time?: number;
  httpStatus?: number;
}

export function Dashboard() {
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, TestState>>({});
  const [copiedCmd, setCopiedCmd] = useState<string | null>(null);
  const [expandedTest, setExpandedTest] = useState<string | null>(null);

  // Token sync state
  const [kiroStatus, setKiroStatus] = useState<KiroCredentialStatus | null>(
    null,
  );
  const [lastSyncTime, setLastSyncTime] = useState<Date | null>(null);
  const [lastCheckTime, setLastCheckTime] = useState<Date | null>(null);
  const kiroHashRef = useRef<string>("");

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
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    fetchStatus();
    fetchConfig();
    loadKiroStatus();
    initTokenHash();

    const statusInterval = setInterval(fetchStatus, 2000);
    const tokenInterval = setInterval(checkTokenFileChanges, 5000);

    return () => {
      clearInterval(statusInterval);
      clearInterval(tokenInterval);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const initTokenHash = async () => {
    try {
      kiroHashRef.current = await getTokenFileHash();
    } catch (e) {
      console.error("Failed to get initial hash:", e);
    }
  };

  const loadKiroStatus = async () => {
    try {
      const status = await getKiroCredentials();
      setKiroStatus(status);
    } catch (e) {
      console.error("Failed to load Kiro status:", e);
    }
  };

  const checkTokenFileChanges = async () => {
    setLastCheckTime(new Date());
    try {
      const result = await checkAndReloadCredentials(kiroHashRef.current);
      kiroHashRef.current = result.new_hash;
      if (result.changed && result.reloaded) {
        await loadKiroStatus();
        setLastSyncTime(new Date());
      }
    } catch (e) {
      console.error("Token check error:", e);
    }
  };

  const handleStart = async () => {
    setLoading(true);
    setError(null);
    try {
      await reloadCredentials();
      await startServer();
      await fetchStatus();
    } catch (e: any) {
      setError(e.toString());
    }
    setLoading(false);
  };

  const handleStop = async () => {
    setLoading(true);
    try {
      await stopServer();
      await fetchStatus();
    } catch (e: any) {
      setError(e.toString());
    }
    setLoading(false);
  };

  const formatUptime = (secs: number) => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  };

  const serverUrl = status
    ? `http://${status.host}:${status.port}`
    : "http://localhost:3001";
  const apiKey = config?.server.api_key || "proxycast-key";
  const maskedKey =
    apiKey.length > 8 ? apiKey.slice(0, 4) + "****" + apiKey.slice(-4) : "****";

  // 测试端点配置
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
      name: "OpenAI 聊天",
      method: "POST",
      path: "/v1/chat/completions",
      needsAuth: true,
      body: JSON.stringify({
        model: "claude-sonnet-4-5",
        messages: [{ role: "user", content: "Say hi in one word" }],
      }),
    },
    {
      id: "anthropic",
      name: "Anthropic 消息",
      method: "POST",
      path: "/v1/messages",
      needsAuth: true,
      body: JSON.stringify({
        model: "claude-sonnet-4-5",
        max_tokens: 100,
        messages: [
          {
            role: "user",
            content: "What is 1+1? Answer with just the number.",
          },
        ],
      }),
    },
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
        endpoint.needsAuth, // maps to 'auth' parameter
      );

      // 添加调试日志
      console.log(`Test ${endpoint.id}:`, result);

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
    } catch (e: any) {
      console.error(`Test ${endpoint.id} error:`, e);
      setTestResults((prev) => ({
        ...prev,
        [endpoint.id]: {
          endpoint: endpoint.path,
          status: "error",
          response: `请求失败: ${e.toString()}`,
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
      return <span className="text-xs text-green-600">✓ {result.time}ms</span>;
    }
    return (
      <span className="text-xs text-red-500">
        ✗ 失败 {result.httpStatus ? `(${result.httpStatus})` : ""}
      </span>
    );
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-bold">仪表盘</h2>
        <p className="text-muted-foreground">服务状态概览</p>
      </div>

      {/* Status Cards */}
      <div className="grid grid-cols-4 gap-4">
        <div className="rounded-lg border bg-card p-4">
          <div className="flex items-center gap-2">
            <Activity className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">状态</span>
          </div>
          <div className="mt-2 flex items-center gap-2">
            <div
              className={`h-2 w-2 rounded-full ${status?.running ? "bg-green-500" : "bg-red-500"}`}
            />
            <span className="font-medium">
              {status?.running ? "运行中" : "已停止"}
            </span>
          </div>
        </div>

        <div className="rounded-lg border bg-card p-4">
          <div className="flex items-center gap-2">
            <Zap className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">请求数</span>
          </div>
          <div className="mt-2 text-2xl font-bold">{status?.requests || 0}</div>
        </div>

        <div className="rounded-lg border bg-card p-4">
          <div className="flex items-center gap-2">
            <Clock className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">运行时间</span>
          </div>
          <div className="mt-2 font-medium">
            {formatUptime(status?.uptime_secs || 0)}
          </div>
        </div>

        <div className="rounded-lg border bg-card p-4">
          <div className="flex items-center gap-2">
            <Server className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">当前 Provider</span>
          </div>
          <div className="mt-2 font-medium capitalize">Kiro</div>
        </div>
      </div>

      {/* Token 同步状态 */}
      <div className="rounded-lg border bg-card p-4">
        <div className="flex items-center justify-between mb-3">
          <h3 className="font-semibold flex items-center gap-2">
            <RefreshCw className="h-4 w-4" />
            Token 同步状态
          </h3>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <span className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
            自动监测中 (5s)
          </div>
        </div>
        <div className="grid grid-cols-4 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">Token 状态:</span>
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
              {kiroStatus?.has_refresh_token ? "✓ 可用" : "✗ 不可用"}
            </span>
          </div>
          <div>
            <span className="text-muted-foreground">最后同步:</span>
            <span className="ml-2">
              {lastSyncTime ? lastSyncTime.toLocaleTimeString() : "从未同步"}
            </span>
          </div>
          <div>
            <span className="text-muted-foreground">最后检测:</span>
            <span className="ml-2">
              {lastCheckTime ? lastCheckTime.toLocaleTimeString() : "-"}
            </span>
          </div>
        </div>
      </div>

      {error && (
        <div className="rounded-lg border border-red-500 bg-red-50 p-4 text-red-700">
          {error}
        </div>
      )}

      {/* Server Control */}
      <div className="rounded-lg border bg-card p-6">
        <h3 className="mb-4 font-semibold">服务控制</h3>
        <div className="flex items-center gap-4">
          <button
            className="rounded-lg bg-green-600 px-4 py-2 text-sm font-medium text-white hover:bg-green-700 disabled:opacity-50"
            onClick={handleStart}
            disabled={loading || status?.running}
          >
            {loading ? "处理中..." : "启动服务"}
          </button>
          <button
            className="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-50"
            onClick={handleStop}
            disabled={loading || !status?.running}
          >
            停止服务
          </button>
        </div>
        <div className="mt-4 flex items-center gap-4 text-sm text-muted-foreground">
          <span>
            API 地址:{" "}
            <code className="rounded bg-muted px-2 py-1">{serverUrl}</code>
          </span>
          <span>
            API Key:{" "}
            <code className="rounded bg-muted px-2 py-1">{maskedKey}</code>
          </span>
        </div>
      </div>

      {/* API 测试 */}
      <div className="rounded-lg border bg-card p-6">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h3 className="font-semibold">API 测试</h3>
            <p className="text-xs text-muted-foreground">
              如遇问题，请打开浏览器开发者工具查看控制台日志
            </p>
          </div>
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
                          ? "bg-green-100 text-green-700"
                          : "bg-blue-100 text-blue-700"
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
                          {result.httpStatus && `(HTTP ${result.httpStatus})`}
                        </p>
                        <pre
                          className={`rounded p-2 text-xs overflow-x-auto max-h-40 ${
                            result.status === "success"
                              ? "bg-green-50"
                              : "bg-red-50"
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
  );
}
