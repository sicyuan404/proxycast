import { useState, useEffect } from "react";
import { Copy, Check, RefreshCw, Globe, Server, Tag } from "lucide-react";
import {
  routesApi,
  RouteInfo,
  RouteListResponse,
  CurlExample,
} from "@/lib/api/routes";

export function RoutesTab() {
  const [routes, setRoutes] = useState<RouteListResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedRoute, setExpandedRoute] = useState<string | null>(null);
  const [curlExamples, setCurlExamples] = useState<
    Record<string, CurlExample[]>
  >({});
  const [copiedUrl, setCopiedUrl] = useState<string | null>(null);
  const [copiedCmd, setCopiedCmd] = useState<string | null>(null);

  const fetchRoutes = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await routesApi.getAvailableRoutes();
      setRoutes(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    setLoading(false);
  };

  useEffect(() => {
    fetchRoutes();
  }, []);

  const fetchCurlExamples = async (selector: string) => {
    if (curlExamples[selector]) return;
    try {
      const examples = await routesApi.getCurlExamples(selector);
      setCurlExamples((prev) => ({ ...prev, [selector]: examples }));
    } catch (e) {
      console.error("Failed to fetch curl examples:", e);
    }
  };

  const handleExpand = (selector: string) => {
    if (expandedRoute === selector) {
      setExpandedRoute(null);
    } else {
      setExpandedRoute(selector);
      fetchCurlExamples(selector);
    }
  };

  const copyToClipboard = (text: string, type: "url" | "cmd", id: string) => {
    navigator.clipboard.writeText(text);
    if (type === "url") {
      setCopiedUrl(id);
      setTimeout(() => setCopiedUrl(null), 2000);
    } else {
      setCopiedCmd(id);
      setTimeout(() => setCopiedCmd(null), 2000);
    }
  };

  const getProviderColor = (provider: string) => {
    switch (provider) {
      case "kiro":
        return "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400";
      case "gemini":
        return "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400";
      case "qwen":
        return "bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400";
      case "openai":
        return "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400";
      case "claude":
        return "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400";
      case "antigravity":
        return "bg-cyan-100 text-cyan-700 dark:bg-cyan-900/30 dark:text-cyan-400";
      default:
        return "bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-400";
    }
  };

  if (loading && !routes) {
    return (
      <div className="flex items-center justify-center py-12">
        <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="font-semibold">可用路由端点</h3>
          <p className="text-sm text-muted-foreground">
            通过不同的 URL 路径访问不同的 Provider
          </p>
        </div>
        <button
          onClick={fetchRoutes}
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

      {routes && (
        <div className="space-y-4">
          {/* Base URL Info */}
          <div className="rounded-lg border bg-card p-4">
            <div className="flex items-center gap-2 text-sm">
              <Globe className="h-4 w-4 text-muted-foreground" />
              <span className="text-muted-foreground">服务器地址:</span>
              <code className="rounded bg-muted px-2 py-1 font-mono">
                {routes.base_url}
              </code>
            </div>
          </div>

          {/* Routes List */}
          <div className="space-y-3">
            {routes.routes.map((route) => (
              <RouteCard
                key={route.selector}
                route={route}
                expanded={expandedRoute === route.selector}
                onExpand={() => handleExpand(route.selector)}
                curlExamples={curlExamples[route.selector]}
                copiedUrl={copiedUrl}
                copiedCmd={copiedCmd}
                onCopyUrl={(url, id) => copyToClipboard(url, "url", id)}
                onCopyCmd={(cmd, id) => copyToClipboard(cmd, "cmd", id)}
                getProviderColor={getProviderColor}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

interface RouteCardProps {
  route: RouteInfo;
  expanded: boolean;
  onExpand: () => void;
  curlExamples?: CurlExample[];
  copiedUrl: string | null;
  copiedCmd: string | null;
  onCopyUrl: (url: string, id: string) => void;
  onCopyCmd: (cmd: string, id: string) => void;
  getProviderColor: (provider: string) => string;
}

function RouteCard({
  route,
  expanded,
  onExpand,
  curlExamples,
  copiedUrl,
  copiedCmd,
  onCopyUrl,
  onCopyCmd,
  getProviderColor,
}: RouteCardProps) {
  return (
    <div className="rounded-lg border bg-card overflow-hidden">
      {/* Header */}
      <div
        className="flex items-center justify-between p-4 cursor-pointer hover:bg-muted/50"
        onClick={onExpand}
      >
        <div className="flex items-center gap-3">
          <Server className="h-5 w-5 text-muted-foreground" />
          <div>
            <div className="flex items-center gap-2">
              <span className="font-medium">{route.selector}</span>
              <span
                className={`rounded px-2 py-0.5 text-xs font-medium ${getProviderColor(route.provider_type)}`}
              >
                {route.provider_type}
              </span>
              {route.tags.map((tag) => (
                <span
                  key={tag}
                  className="flex items-center gap-1 rounded bg-muted px-2 py-0.5 text-xs"
                >
                  <Tag className="h-3 w-3" />
                  {tag}
                </span>
              ))}
            </div>
            <div className="text-sm text-muted-foreground">
              {route.credential_count} 个凭证
              {!route.enabled && (
                <span className="ml-2 text-red-500">(已禁用)</span>
              )}
            </div>
          </div>
        </div>
        <div className="text-muted-foreground">
          {expanded ? "收起" : "展开"}
        </div>
      </div>

      {/* Expanded Content */}
      {expanded && (
        <div className="border-t p-4 space-y-4">
          {/* Endpoints */}
          <div>
            <p className="text-sm font-medium mb-2">端点地址</p>
            <div className="space-y-2">
              {route.endpoints.map((endpoint, idx) => (
                <div
                  key={idx}
                  className="flex items-center justify-between rounded bg-muted p-2"
                >
                  <div className="flex items-center gap-2">
                    <span
                      className={`rounded px-2 py-0.5 text-xs font-medium ${
                        endpoint.protocol === "claude"
                          ? "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
                          : "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
                      }`}
                    >
                      {endpoint.protocol.toUpperCase()}
                    </span>
                    <code className="text-sm font-mono">{endpoint.url}</code>
                  </div>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onCopyUrl(endpoint.url, `${route.selector}-${idx}`);
                    }}
                    className="rounded p-1 hover:bg-background"
                    title="复制 URL"
                  >
                    {copiedUrl === `${route.selector}-${idx}` ? (
                      <Check className="h-4 w-4 text-green-500" />
                    ) : (
                      <Copy className="h-4 w-4" />
                    )}
                  </button>
                </div>
              ))}
            </div>
          </div>

          {/* Curl Examples */}
          {curlExamples && curlExamples.length > 0 && (
            <div>
              <p className="text-sm font-medium mb-2">curl 示例</p>
              <div className="space-y-3">
                {curlExamples.map((example, idx) => (
                  <div key={idx} className="rounded border bg-background p-3">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm text-muted-foreground">
                        {example.description}
                      </span>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onCopyCmd(
                            example.command,
                            `${route.selector}-cmd-${idx}`,
                          );
                        }}
                        className="rounded p-1 hover:bg-muted"
                        title="复制命令"
                      >
                        {copiedCmd === `${route.selector}-cmd-${idx}` ? (
                          <Check className="h-4 w-4 text-green-500" />
                        ) : (
                          <Copy className="h-4 w-4" />
                        )}
                      </button>
                    </div>
                    <pre className="text-xs overflow-x-auto whitespace-pre-wrap bg-muted rounded p-2">
                      {example.command}
                    </pre>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
