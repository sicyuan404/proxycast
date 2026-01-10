import React, { useState, useEffect } from "react";
import {
  ArrowLeft,
  Copy,
  Download,
  Star,
  StarOff,
  Clock,
  CheckCircle2,
  XCircle,
  Loader2,
  ChevronDown,
  ChevronRight,
  Wrench,
  Brain,
  MessageSquare,
  Tag,
  AlertCircle,
  FileJson,
  Code,
  User,
  Bot,
  Settings,
  Zap,
} from "lucide-react";
import {
  flowMonitorApi,
  type LLMFlow,
  type Message,
  type ToolCall,
  type FlowState,
  type ExportFormat,
  formatFlowState,
  formatFlowType,
  formatErrorType,
  formatLatency,
  formatTokenCount,
  formatBytes,
  getMessageText,
} from "@/lib/api/flowMonitor";
import { useFlowActions } from "@/hooks/useFlowActions";
import { FlowTimeline } from "./FlowTimeline";
import { cn } from "@/lib/utils";

interface FlowDetailProps {
  flowId: string;
  onBack?: () => void;
  onExport?: (flowId: string, format: ExportFormat) => void;
}

export function FlowDetail({ flowId, onBack, onExport }: FlowDetailProps) {
  const [flow, setFlow] = useState<LLMFlow | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<
    "request" | "response" | "metadata" | "timeline"
  >("request");
  const [expandedSections, setExpandedSections] = useState<Set<string>>(
    new Set(["messages", "content", "toolCalls"]),
  );
  // 代码模式：显示原始 JSON
  const [codeMode, setCodeMode] = useState(false);

  // 使用 Flow 操作 Hook
  const { copyText, copyFlowContent, exportFlow, exporting } = useFlowActions();

  useEffect(() => {
    const loadFlowDetail = async () => {
      try {
        setLoading(true);
        setError(null);
        const detail = await flowMonitorApi.getFlowDetail(flowId);
        if (detail) {
          setFlow(detail);
        } else {
          setError("Flow 不存在");
        }
      } catch (e) {
        console.error("Failed to load flow detail:", e);
        setError(e instanceof Error ? e.message : "加载失败");
      } finally {
        setLoading(false);
      }
    };
    loadFlowDetail();
  }, [flowId]);

  const handleToggleStar = async () => {
    if (!flow) return;
    try {
      await flowMonitorApi.toggleFlowStar(flow.id);
      setFlow({
        ...flow,
        annotations: {
          ...flow.annotations,
          starred: !flow.annotations.starred,
        },
      });
    } catch (e) {
      console.error("Failed to toggle star:", e);
    }
  };

  const handleCopyContent = async (content: string, _label?: string) => {
    await copyText(content);
  };

  const handleExport = async (format: ExportFormat) => {
    if (onExport) {
      onExport(flowId, format);
    } else {
      await exportFlow(flowId, format);
    }
  };

  const toggleSection = (section: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev);
      if (next.has(section)) {
        next.delete(section);
      } else {
        next.add(section);
      }
      return next;
    });
  };

  const getStateIcon = (state: FlowState) => {
    switch (state) {
      case "Completed":
        return <CheckCircle2 className="h-5 w-5 text-green-500" />;
      case "Failed":
        return <XCircle className="h-5 w-5 text-red-500" />;
      case "Streaming":
        return <Loader2 className="h-5 w-5 text-blue-500 animate-spin" />;
      case "Pending":
        return <Clock className="h-5 w-5 text-yellow-500" />;
      case "Cancelled":
        return <XCircle className="h-5 w-5 text-gray-500" />;
      default:
        return <Clock className="h-5 w-5 text-muted-foreground" />;
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error || !flow) {
    return (
      <div className="rounded-lg border border-red-200 bg-red-50 dark:bg-red-950/20 p-4">
        <div className="flex items-center gap-2 text-red-600 dark:text-red-400">
          <AlertCircle className="h-5 w-5" />
          <span>{error || "Flow 不存在"}</span>
        </div>
        {onBack && (
          <button
            onClick={onBack}
            className="mt-3 flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
          >
            <ArrowLeft className="h-4 w-4" />
            返回列表
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 头部 */}
      <FlowDetailHeader
        flow={flow}
        onBack={onBack}
        onToggleStar={handleToggleStar}
        onExport={handleExport}
        onCopyAll={() => copyFlowContent(flow)}
        getStateIcon={getStateIcon}
        codeMode={codeMode}
        onToggleCodeMode={() => setCodeMode(!codeMode)}
      />

      {/* 代码模式：显示原始 JSON */}
      {codeMode ? (
        <div className="rounded-lg border bg-card">
          <div className="flex items-center justify-between px-4 py-2 border-b bg-muted/30">
            <span className="text-sm font-medium">原始 JSON</span>
            <button
              onClick={() => copyFlowContent(flow)}
              className="p-1.5 rounded hover:bg-muted"
              title="复制 JSON"
            >
              <Copy className="h-4 w-4 text-muted-foreground" />
            </button>
          </div>
          <pre className="p-4 text-xs font-mono whitespace-pre-wrap break-words max-h-[70vh] overflow-y-auto">
            {JSON.stringify(flow, null, 2)}
          </pre>
        </div>
      ) : (
        <>
          {/* 标签页 */}
          <div className="flex border-b">
            <TabButton
              active={activeTab === "request"}
              onClick={() => setActiveTab("request")}
            >
              请求
            </TabButton>
            <TabButton
              active={activeTab === "response"}
              onClick={() => setActiveTab("response")}
            >
              响应
            </TabButton>
            <TabButton
              active={activeTab === "metadata"}
              onClick={() => setActiveTab("metadata")}
            >
              元数据
            </TabButton>
            <TabButton
              active={activeTab === "timeline"}
              onClick={() => setActiveTab("timeline")}
            >
              时间线
            </TabButton>
          </div>

          {/* 内容区域 */}
          <div className="space-y-4">
            {activeTab === "request" && (
              <RequestTab
                flow={flow}
                expandedSections={expandedSections}
                toggleSection={toggleSection}
                onCopy={handleCopyContent}
              />
            )}
            {activeTab === "response" && (
              <ResponseTab
                flow={flow}
                expandedSections={expandedSections}
                toggleSection={toggleSection}
                onCopy={handleCopyContent}
              />
            )}
            {activeTab === "metadata" && (
              <MetadataTab flow={flow} onCopy={handleCopyContent} />
            )}
            {activeTab === "timeline" && <FlowTimeline flow={flow} />}
          </div>
        </>
      )}

      {/* 导出状态提示 */}
      {exporting && (
        <div className="fixed bottom-4 right-4 flex items-center gap-2 rounded-lg border bg-card px-4 py-2 shadow-lg">
          <Loader2 className="h-4 w-4 animate-spin" />
          <span className="text-sm">正在导出...</span>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 子组件
// ============================================================================

interface FlowDetailHeaderProps {
  flow: LLMFlow;
  onBack?: () => void;
  onToggleStar: () => void;
  onExport: (format: ExportFormat) => void;
  onCopyAll: () => void;
  getStateIcon: (state: FlowState) => React.ReactNode;
  codeMode: boolean;
  onToggleCodeMode: () => void;
}

function FlowDetailHeader({
  flow,
  onBack,
  onToggleStar,
  onExport,
  onCopyAll,
  getStateIcon,
  codeMode,
  onToggleCodeMode,
}: FlowDetailHeaderProps) {
  const [showExportMenu, setShowExportMenu] = useState(false);

  const formatTime = (timestamp: string) => {
    return new Date(timestamp).toLocaleString("zh-CN");
  };

  return (
    <div className="space-y-3">
      {/* 顶部操作栏 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          {onBack && (
            <button
              onClick={onBack}
              className="p-1.5 rounded hover:bg-muted"
              title="返回列表"
            >
              <ArrowLeft className="h-5 w-5" />
            </button>
          )}
          <div className="flex items-center gap-2">
            {getStateIcon(flow.state)}
            <span className="font-medium">{formatFlowState(flow.state)}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* 代码模式切换 */}
          <button
            onClick={onToggleCodeMode}
            className={cn(
              "p-1.5 rounded hover:bg-muted",
              codeMode ? "bg-muted text-primary" : "text-muted-foreground",
            )}
            title={codeMode ? "切换到格式化视图" : "切换到代码视图"}
          >
            <Code className="h-5 w-5" />
          </button>
          <button
            onClick={onToggleStar}
            className="p-1.5 rounded hover:bg-muted"
            title={flow.annotations.starred ? "取消收藏" : "收藏"}
          >
            {flow.annotations.starred ? (
              <Star className="h-5 w-5 text-yellow-500 fill-yellow-500" />
            ) : (
              <StarOff className="h-5 w-5 text-muted-foreground" />
            )}
          </button>
          <button
            onClick={onCopyAll}
            className="p-1.5 rounded hover:bg-muted"
            title="复制完整 JSON"
          >
            <Copy className="h-5 w-5 text-muted-foreground" />
          </button>
          <div className="relative">
            <button
              onClick={() => setShowExportMenu(!showExportMenu)}
              className="flex items-center gap-1 px-3 py-1.5 rounded border hover:bg-muted text-sm"
            >
              <Download className="h-4 w-4" />
              导出
            </button>
            {showExportMenu && (
              <div className="absolute right-0 top-full mt-1 w-40 rounded-lg border bg-card shadow-lg z-10">
                {(["json", "markdown", "har"] as ExportFormat[]).map(
                  (format) => (
                    <button
                      key={format}
                      onClick={() => {
                        onExport(format);
                        setShowExportMenu(false);
                      }}
                      className="w-full px-3 py-2 text-left text-sm hover:bg-muted first:rounded-t-lg last:rounded-b-lg"
                    >
                      {format.toUpperCase()}
                    </button>
                  ),
                )}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 基本信息卡片 */}
      <div className="rounded-lg border bg-card p-4">
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <InfoItem label="模型" value={flow.request.model} />
          <InfoItem
            label="提供商"
            value={flow.metadata.provider_id || flow.metadata.provider}
          />
          <InfoItem label="类型" value={formatFlowType(flow.flow_type)} />
          <InfoItem
            label="创建时间"
            value={formatTime(flow.timestamps.created)}
          />
          <InfoItem
            label="耗时"
            value={formatLatency(flow.timestamps.duration_ms)}
          />
          <InfoItem
            label="TTFB"
            value={
              flow.timestamps.ttfb_ms
                ? formatLatency(flow.timestamps.ttfb_ms)
                : "-"
            }
          />
          <InfoItem
            label="输入 Token"
            value={
              flow.response?.usage
                ? formatTokenCount(flow.response.usage.input_tokens)
                : "-"
            }
          />
          <InfoItem
            label="输出 Token"
            value={
              flow.response?.usage
                ? formatTokenCount(flow.response.usage.output_tokens)
                : "-"
            }
          />
        </div>

        {/* 标签和标记 */}
        {(flow.annotations.tags.length > 0 ||
          flow.annotations.marker ||
          flow.annotations.comment) && (
          <div className="mt-4 pt-4 border-t space-y-2">
            {flow.annotations.marker && (
              <div className="flex items-center gap-2">
                <span className="text-lg">{flow.annotations.marker}</span>
              </div>
            )}
            {flow.annotations.tags.length > 0 && (
              <div className="flex items-center gap-2 flex-wrap">
                <Tag className="h-4 w-4 text-muted-foreground" />
                {flow.annotations.tags.map((tag) => (
                  <span
                    key={tag}
                    className="text-xs px-2 py-0.5 rounded-full bg-muted"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            )}
            {flow.annotations.comment && (
              <div className="flex items-start gap-2">
                <MessageSquare className="h-4 w-4 text-muted-foreground mt-0.5" />
                <span className="text-sm text-muted-foreground">
                  {flow.annotations.comment}
                </span>
              </div>
            )}
          </div>
        )}
      </div>

      {/* 错误信息 */}
      {flow.error && (
        <div className="rounded-lg border border-red-200 bg-red-50 dark:bg-red-950/20 p-4">
          <div className="flex items-center gap-2 text-red-600 dark:text-red-400 font-medium">
            <AlertCircle className="h-5 w-5" />
            错误: {formatErrorType(flow.error.error_type)}
          </div>
          <p className="mt-2 text-sm text-red-600 dark:text-red-400">
            {flow.error.message}
          </p>
          {flow.error.status_code && (
            <p className="mt-1 text-xs text-muted-foreground">
              状态码: {flow.error.status_code}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

interface InfoItemProps {
  label: string;
  value: string;
}

function InfoItem({ label, value }: InfoItemProps) {
  return (
    <div>
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="text-sm font-medium truncate" title={value}>
        {value}
      </div>
    </div>
  );
}

interface TabButtonProps {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function TabButton({ active, onClick, children }: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors",
        active
          ? "border-primary text-primary"
          : "border-transparent text-muted-foreground hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}

// ============================================================================
// 请求标签页
// ============================================================================

interface RequestTabProps {
  flow: LLMFlow;
  expandedSections: Set<string>;
  toggleSection: (section: string) => void;
  onCopy: (content: string, label: string) => void;
}

function RequestTab({
  flow,
  expandedSections,
  toggleSection,
  onCopy,
}: RequestTabProps) {
  const { request } = flow;

  return (
    <div className="space-y-4">
      {/* 请求基本信息 */}
      <CollapsibleSection
        title="请求信息"
        icon={<Settings className="h-4 w-4" />}
        expanded={expandedSections.has("requestInfo")}
        onToggle={() => toggleSection("requestInfo")}
      >
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <span className="text-xs px-2 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300 font-mono">
              {request.method}
            </span>
            <span className="text-sm font-mono text-muted-foreground">
              {request.path}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">请求大小:</span>{" "}
              {formatBytes(request.size_bytes)}
            </div>
            <div>
              <span className="text-muted-foreground">流式:</span>{" "}
              {request.parameters.stream ? "是" : "否"}
            </div>
            {request.parameters.temperature !== undefined && (
              <div>
                <span className="text-muted-foreground">Temperature:</span>{" "}
                {request.parameters.temperature}
              </div>
            )}
            {request.parameters.max_tokens !== undefined && (
              <div>
                <span className="text-muted-foreground">Max Tokens:</span>{" "}
                {request.parameters.max_tokens}
              </div>
            )}
          </div>
        </div>
      </CollapsibleSection>

      {/* 系统提示词 */}
      {request.system_prompt && (
        <CollapsibleSection
          title="系统提示词"
          icon={<Settings className="h-4 w-4" />}
          expanded={expandedSections.has("systemPrompt")}
          onToggle={() => toggleSection("systemPrompt")}
          onCopy={() => onCopy(request.system_prompt!, "系统提示词")}
        >
          <pre className="text-sm whitespace-pre-wrap break-words bg-muted/50 rounded p-3 max-h-60 overflow-y-auto">
            {request.system_prompt}
          </pre>
        </CollapsibleSection>
      )}

      {/* 消息列表 */}
      <CollapsibleSection
        title={`消息列表 (${request.messages.length})`}
        icon={<MessageSquare className="h-4 w-4" />}
        expanded={expandedSections.has("messages")}
        onToggle={() => toggleSection("messages")}
      >
        <div className="space-y-3">
          {request.messages.map((message, index) => (
            <MessageItem
              key={index}
              message={message}
              onCopy={(content) => onCopy(content, `消息 ${index + 1}`)}
            />
          ))}
        </div>
      </CollapsibleSection>

      {/* 工具定义 */}
      {request.tools && request.tools.length > 0 && (
        <CollapsibleSection
          title={`工具定义 (${request.tools.length})`}
          icon={<Wrench className="h-4 w-4" />}
          expanded={expandedSections.has("tools")}
          onToggle={() => toggleSection("tools")}
        >
          <div className="space-y-2">
            {request.tools.map((tool, index) => (
              <div key={index} className="rounded bg-muted/50 p-3">
                <div className="font-medium text-sm">{tool.function.name}</div>
                {tool.function.description && (
                  <div className="text-xs text-muted-foreground mt-1">
                    {tool.function.description}
                  </div>
                )}
              </div>
            ))}
          </div>
        </CollapsibleSection>
      )}

      {/* 请求头 */}
      <CollapsibleSection
        title="请求头"
        icon={<Code className="h-4 w-4" />}
        expanded={expandedSections.has("requestHeaders")}
        onToggle={() => toggleSection("requestHeaders")}
      >
        <div className="space-y-1 text-sm font-mono">
          {Object.entries(request.headers).map(([key, value]) => (
            <div key={key} className="flex">
              <span className="text-muted-foreground w-40 shrink-0">
                {key}:
              </span>
              <span className="break-all">
                {key.toLowerCase().includes("authorization") ||
                key.toLowerCase().includes("api-key")
                  ? "***"
                  : value}
              </span>
            </div>
          ))}
        </div>
      </CollapsibleSection>

      {/* 原始请求体 */}
      <CollapsibleSection
        title="原始请求体"
        icon={<FileJson className="h-4 w-4" />}
        expanded={expandedSections.has("requestBody")}
        onToggle={() => toggleSection("requestBody")}
        onCopy={() => onCopy(JSON.stringify(request.body, null, 2), "请求体")}
      >
        <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-muted/50 rounded p-3 max-h-96 overflow-y-auto">
          {JSON.stringify(request.body, null, 2)}
        </pre>
      </CollapsibleSection>
    </div>
  );
}

interface MessageItemProps {
  message: Message;
  onCopy: (content: string) => void;
}

function MessageItem({ message, onCopy }: MessageItemProps) {
  const [expanded, setExpanded] = useState(true);
  const content = getMessageText(message.content);

  const getRoleIcon = (role: string) => {
    switch (role) {
      case "user":
        return <User className="h-4 w-4 text-blue-500" />;
      case "assistant":
        return <Bot className="h-4 w-4 text-green-500" />;
      case "system":
        return <Settings className="h-4 w-4 text-purple-500" />;
      case "tool":
      case "function":
        return <Wrench className="h-4 w-4 text-orange-500" />;
      default:
        return <MessageSquare className="h-4 w-4 text-muted-foreground" />;
    }
  };

  const getRoleLabel = (role: string) => {
    const labels: Record<string, string> = {
      user: "用户",
      assistant: "助手",
      system: "系统",
      tool: "工具",
      function: "函数",
    };
    return labels[role] || role;
  };

  return (
    <div className="rounded border bg-card">
      <div
        className="flex items-center gap-2 px-3 py-2 cursor-pointer hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
        {getRoleIcon(message.role)}
        <span className="text-sm font-medium">
          {getRoleLabel(message.role)}
        </span>
        {message.name && (
          <span className="text-xs text-muted-foreground">
            ({message.name})
          </span>
        )}
        <button
          onClick={(e) => {
            e.stopPropagation();
            onCopy(content);
          }}
          className="ml-auto p-1 hover:bg-muted rounded"
        >
          <Copy className="h-3 w-3 text-muted-foreground" />
        </button>
      </div>
      {expanded && (
        <div className="px-3 pb-3 space-y-2">
          <pre className="text-sm whitespace-pre-wrap break-words bg-muted/50 rounded p-2 max-h-60 overflow-y-auto">
            {content}
          </pre>
          {/* 工具调用 */}
          {message.tool_calls && message.tool_calls.length > 0 && (
            <div className="space-y-2">
              <div className="text-xs text-muted-foreground">工具调用:</div>
              {message.tool_calls.map((tc, i) => (
                <ToolCallItem key={i} toolCall={tc} />
              ))}
            </div>
          )}
          {/* 工具结果 */}
          {message.tool_result && (
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">工具结果:</div>
              <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-muted/50 rounded p-2">
                {message.tool_result.content}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 响应标签页
// ============================================================================

interface ResponseTabProps {
  flow: LLMFlow;
  expandedSections: Set<string>;
  toggleSection: (section: string) => void;
  onCopy: (content: string, label: string) => void;
}

function ResponseTab({
  flow,
  expandedSections,
  toggleSection,
  onCopy,
}: ResponseTabProps) {
  const { response } = flow;

  // 调试日志
  console.log("[FlowDetail] ResponseTab - response:", response);
  console.log("[FlowDetail] ResponseTab - content:", response?.content);
  console.log(
    "[FlowDetail] ResponseTab - content length:",
    response?.content?.length,
  );
  console.log(
    "[FlowDetail] ResponseTab - expandedSections:",
    Array.from(expandedSections),
  );

  if (!response) {
    return (
      <div className="rounded-lg border bg-muted/50 p-8 text-center text-muted-foreground">
        暂无响应数据
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 响应基本信息 */}
      <CollapsibleSection
        title="响应信息"
        icon={<Zap className="h-4 w-4" />}
        expanded={expandedSections.has("responseInfo")}
        onToggle={() => toggleSection("responseInfo")}
      >
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">状态码:</span>{" "}
            <span
              className={cn(
                response.status_code >= 200 && response.status_code < 300
                  ? "text-green-600"
                  : "text-red-600",
              )}
            >
              {response.status_code} {response.status_text}
            </span>
          </div>
          <div>
            <span className="text-muted-foreground">响应大小:</span>{" "}
            {formatBytes(response.size_bytes)}
          </div>
          {response.stop_reason && (
            <div>
              <span className="text-muted-foreground">停止原因:</span>{" "}
              {typeof response.stop_reason === "string"
                ? response.stop_reason
                : response.stop_reason.other}
            </div>
          )}
          {response.stream_info && (
            <>
              <div>
                <span className="text-muted-foreground">Chunk 数:</span>{" "}
                {response.stream_info.chunk_count}
              </div>
              <div>
                <span className="text-muted-foreground">首 Chunk 延迟:</span>{" "}
                {formatLatency(response.stream_info.first_chunk_latency_ms)}
              </div>
              <div>
                <span className="text-muted-foreground">平均 Chunk 间隔:</span>{" "}
                {response.stream_info.avg_chunk_interval_ms.toFixed(1)}ms
              </div>
            </>
          )}
        </div>
      </CollapsibleSection>

      {/* Token 使用统计 */}
      <CollapsibleSection
        title="Token 使用"
        icon={<Zap className="h-4 w-4" />}
        expanded={expandedSections.has("tokenUsage")}
        onToggle={() => toggleSection("tokenUsage")}
      >
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">输入 Token:</span>{" "}
            {formatTokenCount(response.usage.input_tokens)}
          </div>
          <div>
            <span className="text-muted-foreground">输出 Token:</span>{" "}
            {formatTokenCount(response.usage.output_tokens)}
          </div>
          <div>
            <span className="text-muted-foreground">总 Token:</span>{" "}
            {formatTokenCount(response.usage.total_tokens)}
          </div>
          {response.usage.cache_read_tokens !== undefined && (
            <div>
              <span className="text-muted-foreground">缓存读取:</span>{" "}
              {formatTokenCount(response.usage.cache_read_tokens)}
            </div>
          )}
          {response.usage.cache_write_tokens !== undefined && (
            <div>
              <span className="text-muted-foreground">缓存写入:</span>{" "}
              {formatTokenCount(response.usage.cache_write_tokens)}
            </div>
          )}
          {response.usage.thinking_tokens !== undefined && (
            <div>
              <span className="text-muted-foreground">思维链 Token:</span>{" "}
              {formatTokenCount(response.usage.thinking_tokens)}
            </div>
          )}
        </div>
      </CollapsibleSection>

      {/* 响应内容 */}
      <CollapsibleSection
        title="响应内容"
        icon={<MessageSquare className="h-4 w-4" />}
        expanded={expandedSections.has("content")}
        onToggle={() => toggleSection("content")}
        onCopy={() => onCopy(response.content, "响应内容")}
      >
        <pre className="text-sm whitespace-pre-wrap break-words bg-muted/50 rounded p-3 max-h-96 overflow-y-auto">
          {response.content || "(空)"}
        </pre>
      </CollapsibleSection>

      {/* 思维链内容 */}
      {response.thinking && (
        <CollapsibleSection
          title="思维链"
          icon={<Brain className="h-4 w-4" />}
          expanded={expandedSections.has("thinking")}
          onToggle={() => toggleSection("thinking")}
          onCopy={() => onCopy(response.thinking!.text, "思维链")}
        >
          <div className="space-y-2">
            {response.thinking.tokens && (
              <div className="text-xs text-muted-foreground">
                Token 数: {formatTokenCount(response.thinking.tokens)}
              </div>
            )}
            <pre className="text-sm whitespace-pre-wrap break-words bg-purple-50 dark:bg-purple-950/20 rounded p-3 max-h-96 overflow-y-auto">
              {response.thinking.text}
            </pre>
          </div>
        </CollapsibleSection>
      )}

      {/* 工具调用 */}
      {response.tool_calls.length > 0 && (
        <CollapsibleSection
          title={`工具调用 (${response.tool_calls.length})`}
          icon={<Wrench className="h-4 w-4" />}
          expanded={expandedSections.has("toolCalls")}
          onToggle={() => toggleSection("toolCalls")}
        >
          <div className="space-y-3">
            {response.tool_calls.map((tc, index) => (
              <ToolCallItem key={index} toolCall={tc} />
            ))}
          </div>
        </CollapsibleSection>
      )}

      {/* 响应头 */}
      <CollapsibleSection
        title="响应头"
        icon={<Code className="h-4 w-4" />}
        expanded={expandedSections.has("responseHeaders")}
        onToggle={() => toggleSection("responseHeaders")}
      >
        <div className="space-y-1 text-sm font-mono">
          {Object.entries(response.headers).map(([key, value]) => (
            <div key={key} className="flex">
              <span className="text-muted-foreground w-40 shrink-0">
                {key}:
              </span>
              <span className="break-all">{value}</span>
            </div>
          ))}
        </div>
      </CollapsibleSection>

      {/* 原始响应体 */}
      <CollapsibleSection
        title="原始响应体"
        icon={<FileJson className="h-4 w-4" />}
        expanded={expandedSections.has("responseBody")}
        onToggle={() => toggleSection("responseBody")}
        onCopy={() => onCopy(JSON.stringify(response.body, null, 2), "响应体")}
      >
        <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-muted/50 rounded p-3 max-h-96 overflow-y-auto">
          {JSON.stringify(response.body, null, 2)}
        </pre>
      </CollapsibleSection>
    </div>
  );
}

interface ToolCallItemProps {
  toolCall: ToolCall;
}

function ToolCallItem({ toolCall }: ToolCallItemProps) {
  const [expanded, setExpanded] = useState(false);

  let parsedArgs: unknown = null;
  try {
    parsedArgs = JSON.parse(toolCall.function.arguments);
  } catch (_e) {
    // 保持原始字符串
  }

  return (
    <div className="rounded border bg-orange-50 dark:bg-orange-950/20">
      <div
        className="flex items-center gap-2 px-3 py-2 cursor-pointer hover:bg-orange-100 dark:hover:bg-orange-950/30"
        onClick={() => setExpanded(!expanded)}
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
        <Wrench className="h-4 w-4 text-orange-500" />
        <span className="text-sm font-medium">{toolCall.function.name}</span>
        <span className="text-xs text-muted-foreground ml-auto">
          ID: {toolCall.id.slice(0, 8)}...
        </span>
      </div>
      {expanded && (
        <div className="px-3 pb-3">
          <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-white dark:bg-background rounded p-2 max-h-60 overflow-y-auto">
            {parsedArgs
              ? JSON.stringify(parsedArgs, null, 2)
              : toolCall.function.arguments}
          </pre>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 元数据标签页
// ============================================================================

interface MetadataTabProps {
  flow: LLMFlow;
  onCopy: (content: string, label: string) => void;
}

function MetadataTab({ flow, onCopy }: MetadataTabProps) {
  const { metadata, timestamps } = flow;

  const formatTime = (timestamp: string | undefined) => {
    if (!timestamp) return "-";
    return new Date(timestamp).toLocaleString("zh-CN");
  };

  return (
    <div className="space-y-4">
      {/* 时间戳 */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
          <Clock className="h-4 w-4" />
          时间戳
        </h3>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">创建时间:</span>{" "}
            {formatTime(timestamps.created)}
          </div>
          <div>
            <span className="text-muted-foreground">请求开始:</span>{" "}
            {formatTime(timestamps.request_start)}
          </div>
          <div>
            <span className="text-muted-foreground">请求结束:</span>{" "}
            {formatTime(timestamps.request_end)}
          </div>
          <div>
            <span className="text-muted-foreground">响应开始:</span>{" "}
            {formatTime(timestamps.response_start)}
          </div>
          <div>
            <span className="text-muted-foreground">响应结束:</span>{" "}
            {formatTime(timestamps.response_end)}
          </div>
          <div>
            <span className="text-muted-foreground">总耗时:</span>{" "}
            {formatLatency(timestamps.duration_ms)}
          </div>
          {timestamps.ttfb_ms && (
            <div>
              <span className="text-muted-foreground">TTFB:</span>{" "}
              {formatLatency(timestamps.ttfb_ms)}
            </div>
          )}
        </div>
      </div>

      {/* 提供商信息 */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
          <Settings className="h-4 w-4" />
          提供商信息
        </h3>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">提供商:</span>{" "}
            {metadata.provider}
          </div>
          {metadata.credential_id && (
            <div>
              <span className="text-muted-foreground">凭证 ID:</span>{" "}
              {metadata.credential_id.slice(0, 8)}...
            </div>
          )}
          {metadata.credential_name && (
            <div>
              <span className="text-muted-foreground">凭证名称:</span>{" "}
              {metadata.credential_name}
            </div>
          )}
          <div>
            <span className="text-muted-foreground">重试次数:</span>{" "}
            {metadata.retry_count}
          </div>
          {metadata.context_usage_percentage !== undefined && (
            <div>
              <span className="text-muted-foreground">上下文使用率:</span>{" "}
              {(metadata.context_usage_percentage * 100).toFixed(1)}%
            </div>
          )}
        </div>
      </div>

      {/* 客户端信息 */}
      {(metadata.client_info.ip ||
        metadata.client_info.user_agent ||
        metadata.client_info.request_id) && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
            <User className="h-4 w-4" />
            客户端信息
          </h3>
          <div className="space-y-2 text-sm">
            {metadata.client_info.ip && (
              <div>
                <span className="text-muted-foreground">IP:</span>{" "}
                {metadata.client_info.ip}
              </div>
            )}
            {metadata.client_info.user_agent && (
              <div>
                <span className="text-muted-foreground">User-Agent:</span>{" "}
                <span className="break-all">
                  {metadata.client_info.user_agent}
                </span>
              </div>
            )}
            {metadata.client_info.request_id && (
              <div>
                <span className="text-muted-foreground">Request ID:</span>{" "}
                {metadata.client_info.request_id}
              </div>
            )}
          </div>
        </div>
      )}

      {/* 路由信息 */}
      {(metadata.routing_info.target_url ||
        metadata.routing_info.route_rule ||
        metadata.routing_info.load_balance_strategy) && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
            <Zap className="h-4 w-4" />
            路由信息
          </h3>
          <div className="space-y-2 text-sm">
            {metadata.routing_info.target_url && (
              <div>
                <span className="text-muted-foreground">目标 URL:</span>{" "}
                <span className="break-all">
                  {metadata.routing_info.target_url}
                </span>
              </div>
            )}
            {metadata.routing_info.route_rule && (
              <div>
                <span className="text-muted-foreground">路由规则:</span>{" "}
                {metadata.routing_info.route_rule}
              </div>
            )}
            {metadata.routing_info.load_balance_strategy && (
              <div>
                <span className="text-muted-foreground">负载均衡策略:</span>{" "}
                {metadata.routing_info.load_balance_strategy}
              </div>
            )}
          </div>
        </div>
      )}

      {/* 注入参数 */}
      {metadata.injected_params &&
        Object.keys(metadata.injected_params).length > 0 && (
          <div className="rounded-lg border bg-card p-4">
            <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
              <Code className="h-4 w-4" />
              注入参数
            </h3>
            <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-muted/50 rounded p-3">
              {JSON.stringify(metadata.injected_params, null, 2)}
            </pre>
          </div>
        )}

      {/* Flow ID */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="text-sm font-medium mb-3">Flow ID</h3>
        <div className="flex items-center gap-2">
          <code className="text-xs font-mono bg-muted px-2 py-1 rounded flex-1 break-all">
            {flow.id}
          </code>
          <button
            onClick={() => onCopy(flow.id, "Flow ID")}
            className="p-1.5 rounded hover:bg-muted shrink-0"
          >
            <Copy className="h-4 w-4 text-muted-foreground" />
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 通用组件
// ============================================================================

interface CollapsibleSectionProps {
  title: string;
  icon?: React.ReactNode;
  expanded: boolean;
  onToggle: () => void;
  onCopy?: () => void;
  children: React.ReactNode;
}

function CollapsibleSection({
  title,
  icon,
  expanded,
  onToggle,
  onCopy,
  children,
}: CollapsibleSectionProps) {
  return (
    <div className="rounded-lg border bg-card">
      <div
        className="flex items-center gap-2 px-4 py-3 cursor-pointer hover:bg-muted/50"
        onClick={onToggle}
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
        {icon}
        <span className="text-sm font-medium">{title}</span>
        {onCopy && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onCopy();
            }}
            className="ml-auto p-1 hover:bg-muted rounded"
            title="复制"
          >
            <Copy className="h-3.5 w-3.5 text-muted-foreground" />
          </button>
        )}
      </div>
      {expanded && <div className="px-4 pb-4">{children}</div>}
    </div>
  );
}

export default FlowDetail;
