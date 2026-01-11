/**
 * 拦截编辑器组件
 *
 * 实现请求/响应编辑器和继续/取消按钮
 * **Validates: Requirements 2.2, 2.3, 2.4, 2.5**
 */

import { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  X,
  Play,
  Save,
  RotateCcw,
  AlertCircle,
  Loader2,
  ArrowRight,
  ArrowLeft,
  Clock,
  Copy,
  Check,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type {
  InterceptedFlow,
  InterceptType,
  InterceptState,
} from "./InterceptPanel";
import type { LLMRequest, LLMResponse } from "@/lib/api/flowMonitor";

// ============================================================================
// 组件属性
// ============================================================================

interface InterceptEditorProps {
  flowId: string;
  onClose?: () => void;
  onContinue?: () => void;
  onCancel?: () => void;
  className?: string;
}

// ============================================================================
// 主组件
// ============================================================================

export function InterceptEditor({
  flowId,
  onClose,
  onContinue,
  onCancel,
  className,
}: InterceptEditorProps) {
  // 状态
  const [interceptedFlow, setInterceptedFlow] =
    useState<InterceptedFlow | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [continuing, setContinuing] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  // 编辑状态
  const [editedContent, setEditedContent] = useState<string>("");
  const [isModified, setIsModified] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);

  // 视图模式
  const [viewMode, setViewMode] = useState<"formatted" | "raw">("formatted");
  const [copied, setCopied] = useState(false);

  // 加载被拦截的 Flow
  const loadInterceptedFlow = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const flow = await safeInvoke<InterceptedFlow | null>(
        "intercept_get_flow",
        {
          flowId,
        },
      );
      if (flow) {
        setInterceptedFlow(flow);
        // 初始化编辑内容
        const content = getOriginalContent(flow);
        setEditedContent(JSON.stringify(content, null, 2));
        setIsModified(false);
      } else {
        setError("拦截的 Flow 不存在或已处理");
      }
    } catch (e) {
      console.error("加载拦截 Flow 失败:", e);
      setError(e instanceof Error ? e.message : "加载失败");
    } finally {
      setLoading(false);
    }
  }, [flowId]);

  // 获取原始内容
  const getOriginalContent = (flow: InterceptedFlow): unknown => {
    if (flow.intercept_type === "request") {
      return flow.original_request;
    } else {
      return flow.original_response;
    }
  };

  // 处理内容变更
  const handleContentChange = (value: string) => {
    setEditedContent(value);
    setIsModified(true);

    // 验证 JSON
    try {
      JSON.parse(value);
      setParseError(null);
    } catch (e) {
      setParseError(e instanceof Error ? e.message : "JSON 解析错误");
    }
  };

  // 重置内容
  const handleReset = () => {
    if (interceptedFlow) {
      const content = getOriginalContent(interceptedFlow);
      setEditedContent(JSON.stringify(content, null, 2));
      setIsModified(false);
      setParseError(null);
    }
  };

  // 继续处理
  const handleContinue = async () => {
    if (!interceptedFlow) return;

    try {
      setContinuing(true);
      setError(null);

      let modifiedRequest: LLMRequest | null = null;
      let modifiedResponse: LLMResponse | null = null;

      // 如果有修改，解析修改后的内容
      if (isModified && !parseError) {
        try {
          const parsed = JSON.parse(editedContent);
          if (interceptedFlow.intercept_type === "request") {
            modifiedRequest = parsed as LLMRequest;
          } else {
            modifiedResponse = parsed as LLMResponse;
          }
        } catch (_e) {
          setError("JSON 解析失败，请检查格式");
          return;
        }
      }

      await safeInvoke("intercept_continue", {
        flowId: interceptedFlow.flow_id,
        modifiedRequest,
        modifiedResponse,
      });

      onContinue?.();
      onClose?.();
    } catch (e) {
      console.error("继续 Flow 失败:", e);
      setError(e instanceof Error ? e.message : "操作失败");
    } finally {
      setContinuing(false);
    }
  };

  // 取消处理
  const handleCancel = async () => {
    if (!interceptedFlow) return;

    try {
      setCancelling(true);
      setError(null);

      await safeInvoke("intercept_cancel", {
        flowId: interceptedFlow.flow_id,
      });

      onCancel?.();
      onClose?.();
    } catch (e) {
      console.error("取消 Flow 失败:", e);
      setError(e instanceof Error ? e.message : "操作失败");
    } finally {
      setCancelling(false);
    }
  };

  // 复制内容
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(editedContent);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error("复制失败:", e);
    }
  };

  // 初始化加载
  useEffect(() => {
    loadInterceptedFlow();
  }, [loadInterceptedFlow]);

  // 格式化时间
  const formatTime = (timestamp: string) => {
    return new Date(timestamp).toLocaleString("zh-CN");
  };

  // 获取状态标签
  const getStateLabel = (state: InterceptState) => {
    const labels: Record<InterceptState, string> = {
      pending: "等待处理",
      editing: "编辑中",
      continued: "已继续",
      cancelled: "已取消",
      timedout: "已超时",
    };
    return labels[state] || state;
  };

  if (loading) {
    return (
      <div
        className={cn(
          "rounded-lg border bg-card p-8 flex items-center justify-center",
          className,
        )}
      >
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error && !interceptedFlow) {
    return (
      <div className={cn("rounded-lg border bg-card p-4", className)}>
        <div className="flex items-center gap-2 text-red-600 dark:text-red-400">
          <AlertCircle className="h-5 w-5" />
          <span>{error}</span>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            className="mt-3 text-sm text-muted-foreground hover:text-foreground"
          >
            关闭
          </button>
        )}
      </div>
    );
  }

  if (!interceptedFlow) {
    return null;
  }

  return (
    <div className={cn("rounded-lg border bg-card flex flex-col", className)}>
      {/* 头部 */}
      <div className="flex items-center justify-between px-4 py-3 border-b">
        <div className="flex items-center gap-3">
          {interceptedFlow.intercept_type === "request" ? (
            <ArrowRight className="h-5 w-5 text-blue-500" />
          ) : (
            <ArrowLeft className="h-5 w-5 text-green-500" />
          )}
          <div>
            <div className="font-medium">
              {interceptedFlow.intercept_type === "request"
                ? "拦截请求"
                : "拦截响应"}
            </div>
            <div className="text-xs text-muted-foreground flex items-center gap-2">
              <span className="font-mono">
                {interceptedFlow.flow_id.slice(0, 12)}...
              </span>
              <span>•</span>
              <span>{getStateLabel(interceptedFlow.state)}</span>
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {/* 视图模式切换 */}
          <div className="flex rounded-lg border overflow-hidden">
            <button
              onClick={() => setViewMode("formatted")}
              className={cn(
                "px-2 py-1 text-xs",
                viewMode === "formatted"
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-muted",
              )}
            >
              格式化
            </button>
            <button
              onClick={() => setViewMode("raw")}
              className={cn(
                "px-2 py-1 text-xs",
                viewMode === "raw"
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-muted",
              )}
            >
              原始
            </button>
          </div>
          {onClose && (
            <button
              onClick={onClose}
              className="p-1.5 rounded hover:bg-muted"
              title="关闭"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>

      {/* 信息栏 */}
      <div className="px-4 py-2 border-b bg-muted/30 flex items-center justify-between text-sm">
        <div className="flex items-center gap-4 text-muted-foreground">
          <span className="flex items-center gap-1">
            <Clock className="h-4 w-4" />
            {formatTime(interceptedFlow.intercepted_at)}
          </span>
          {isModified && (
            <span className="text-orange-600 dark:text-orange-400">已修改</span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleCopy}
            className="p-1.5 rounded hover:bg-muted"
            title="复制"
          >
            {copied ? (
              <Check className="h-4 w-4 text-green-500" />
            ) : (
              <Copy className="h-4 w-4 text-muted-foreground" />
            )}
          </button>
          {isModified && (
            <button
              onClick={handleReset}
              className="p-1.5 rounded hover:bg-muted"
              title="重置"
            >
              <RotateCcw className="h-4 w-4 text-muted-foreground" />
            </button>
          )}
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="px-4 py-2 bg-red-50 dark:bg-red-950/20 text-red-600 dark:text-red-400 text-sm flex items-center gap-2">
          <AlertCircle className="h-4 w-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* 解析错误提示 */}
      {parseError && (
        <div className="px-4 py-2 bg-yellow-50 dark:bg-yellow-950/20 text-yellow-600 dark:text-yellow-400 text-sm flex items-center gap-2">
          <AlertCircle className="h-4 w-4 shrink-0" />
          <span>JSON 格式错误: {parseError}</span>
        </div>
      )}

      {/* 编辑区域 */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {viewMode === "formatted" ? (
          <FormattedView
            content={editedContent}
            interceptType={interceptedFlow.intercept_type}
          />
        ) : (
          <textarea
            value={editedContent}
            onChange={(e) => handleContentChange(e.target.value)}
            className={cn(
              "w-full h-full p-4 font-mono text-sm bg-transparent resize-none focus:outline-none",
              parseError && "text-red-600 dark:text-red-400",
            )}
            spellCheck={false}
          />
        )}
      </div>

      {/* 底部操作栏 */}
      <div className="flex items-center justify-between px-4 py-3 border-t bg-muted/30">
        <div className="text-xs text-muted-foreground">
          {isModified ? "修改后的内容将用于继续处理" : "可以编辑内容后继续处理"}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleCancel}
            disabled={continuing || cancelling}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border hover:bg-muted text-sm disabled:opacity-50"
          >
            {cancelling ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <X className="h-4 w-4" />
            )}
            取消请求
          </button>
          <button
            onClick={handleContinue}
            disabled={continuing || cancelling || !!parseError}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm disabled:opacity-50",
              isModified
                ? "bg-orange-500 hover:bg-orange-600 text-white"
                : "bg-green-500 hover:bg-green-600 text-white",
            )}
          >
            {continuing ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : isModified ? (
              <Save className="h-4 w-4" />
            ) : (
              <Play className="h-4 w-4" />
            )}
            {isModified ? "应用修改并继续" : "继续处理"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 格式化视图组件
// ============================================================================

interface FormattedViewProps {
  content: string;
  interceptType: InterceptType;
}

function FormattedView({ content, interceptType }: FormattedViewProps) {
  const [parsed, setParsed] = useState<unknown>(null);
  const [parseError, setParseError] = useState<string | null>(null);

  useEffect(() => {
    try {
      const data = JSON.parse(content);
      setParsed(data);
      setParseError(null);
    } catch (e) {
      setParsed(null);
      setParseError(e instanceof Error ? e.message : "解析错误");
    }
  }, [content]);

  if (parseError) {
    return (
      <div className="p-4 text-sm text-muted-foreground">
        <p>无法解析 JSON，请切换到原始视图编辑</p>
        <p className="text-red-500 mt-2">{parseError}</p>
      </div>
    );
  }

  if (!parsed) {
    return (
      <div className="p-4 flex items-center justify-center">
        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (interceptType === "request") {
    return <RequestFormattedView request={parsed as LLMRequest} />;
  } else {
    return <ResponseFormattedView response={parsed as LLMResponse} />;
  }
}

// ============================================================================
// 请求格式化视图
// ============================================================================

interface RequestFormattedViewProps {
  request: LLMRequest;
}

function RequestFormattedView({ request }: RequestFormattedViewProps) {
  return (
    <div className="p-4 space-y-4 overflow-y-auto h-full">
      {/* 基本信息 */}
      <div className="space-y-2">
        <h4 className="text-sm font-medium text-muted-foreground">基本信息</h4>
        <div className="grid grid-cols-2 gap-2 text-sm">
          <div>
            <span className="text-muted-foreground">方法:</span>{" "}
            <span className="font-mono">{request.method}</span>
          </div>
          <div>
            <span className="text-muted-foreground">路径:</span>{" "}
            <span className="font-mono">{request.path}</span>
          </div>
          <div>
            <span className="text-muted-foreground">模型:</span>{" "}
            <span className="font-medium">{request.model}</span>
          </div>
          <div>
            <span className="text-muted-foreground">流式:</span>{" "}
            {request.parameters?.stream ? "是" : "否"}
          </div>
        </div>
      </div>

      {/* 系统提示词 */}
      {request.system_prompt && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-muted-foreground">
            系统提示词
          </h4>
          <pre className="text-sm whitespace-pre-wrap break-words bg-muted/50 rounded p-3 max-h-40 overflow-y-auto">
            {request.system_prompt}
          </pre>
        </div>
      )}

      {/* 消息列表 */}
      {request.messages && request.messages.length > 0 && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-muted-foreground">
            消息 ({request.messages.length})
          </h4>
          <div className="space-y-2">
            {request.messages.map((msg, idx) => (
              <div key={idx} className="rounded border p-2">
                <div className="text-xs font-medium text-muted-foreground mb-1">
                  {msg.role}
                </div>
                <pre className="text-sm whitespace-pre-wrap break-words">
                  {typeof msg.content === "string"
                    ? msg.content
                    : JSON.stringify(msg.content, null, 2)}
                </pre>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 参数 */}
      {request.parameters && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-muted-foreground">参数</h4>
          <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-muted/50 rounded p-3">
            {JSON.stringify(request.parameters, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 响应格式化视图
// ============================================================================

interface ResponseFormattedViewProps {
  response: LLMResponse;
}

function ResponseFormattedView({ response }: ResponseFormattedViewProps) {
  return (
    <div className="p-4 space-y-4 overflow-y-auto h-full">
      {/* 基本信息 */}
      <div className="space-y-2">
        <h4 className="text-sm font-medium text-muted-foreground">基本信息</h4>
        <div className="grid grid-cols-2 gap-2 text-sm">
          <div>
            <span className="text-muted-foreground">状态码:</span>{" "}
            <span
              className={cn(
                "font-mono",
                response.status_code >= 200 && response.status_code < 300
                  ? "text-green-600"
                  : "text-red-600",
              )}
            >
              {response.status_code} {response.status_text}
            </span>
          </div>
          {response.stop_reason && (
            <div>
              <span className="text-muted-foreground">停止原因:</span>{" "}
              {typeof response.stop_reason === "string"
                ? response.stop_reason
                : response.stop_reason.other}
            </div>
          )}
        </div>
      </div>

      {/* Token 使用 */}
      {response.usage && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-muted-foreground">
            Token 使用
          </h4>
          <div className="grid grid-cols-3 gap-2 text-sm">
            <div>
              <span className="text-muted-foreground">输入:</span>{" "}
              {response.usage.input_tokens}
            </div>
            <div>
              <span className="text-muted-foreground">输出:</span>{" "}
              {response.usage.output_tokens}
            </div>
            <div>
              <span className="text-muted-foreground">总计:</span>{" "}
              {response.usage.total_tokens}
            </div>
          </div>
        </div>
      )}

      {/* 响应内容 */}
      {response.content && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-muted-foreground">
            响应内容
          </h4>
          <pre className="text-sm whitespace-pre-wrap break-words bg-muted/50 rounded p-3 max-h-60 overflow-y-auto">
            {response.content}
          </pre>
        </div>
      )}

      {/* 思维链 */}
      {response.thinking && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-muted-foreground">思维链</h4>
          <pre className="text-sm whitespace-pre-wrap break-words bg-purple-50 dark:bg-purple-950/20 rounded p-3 max-h-40 overflow-y-auto">
            {response.thinking.text}
          </pre>
        </div>
      )}

      {/* 工具调用 */}
      {response.tool_calls && response.tool_calls.length > 0 && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-muted-foreground">
            工具调用 ({response.tool_calls.length})
          </h4>
          <div className="space-y-2">
            {response.tool_calls.map((tc, idx) => (
              <div key={idx} className="rounded border p-2">
                <div className="text-xs font-medium">{tc.function.name}</div>
                <pre className="text-xs font-mono whitespace-pre-wrap break-words mt-1">
                  {tc.function.arguments}
                </pre>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default InterceptEditor;
