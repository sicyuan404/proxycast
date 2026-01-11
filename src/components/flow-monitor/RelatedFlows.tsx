/**
 * RelatedFlows 组件
 *
 * 显示与当前 Flow 相关的其他 Flow，包括：
 * - 同一会话中的 Flow
 * - 相似请求的 Flow（相同模型、相同提供商）
 *
 * **Validates: Requirements 7.5**
 */

import { useState, useEffect } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  Loader2,
  Link2,
  Clock,
  CheckCircle2,
  XCircle,
  ChevronRight,
  Folder,
  Sparkles,
} from "lucide-react";
import {
  flowMonitorApi,
  type LLMFlow,
  type FlowState,
  formatLatency,
  formatTokenCount,
  truncateText,
} from "@/lib/api/flowMonitor";
import { cn } from "@/lib/utils";

interface RelatedFlowsProps {
  /** 当前 Flow */
  flow: LLMFlow;
  /** 点击 Flow 时的回调 */
  onFlowClick?: (flowId: string) => void;
  /** 最大显示数量 */
  maxItems?: number;
}

interface FlowSession {
  id: string;
  name: string;
  description?: string;
  flow_ids: string[];
  created_at: string;
  updated_at: string;
  archived: boolean;
}

export function RelatedFlows({
  flow,
  onFlowClick,
  maxItems = 10,
}: RelatedFlowsProps) {
  const [loading, setLoading] = useState(true);
  const [sessionFlows, setSessionFlows] = useState<LLMFlow[]>([]);
  const [similarFlows, setSimilarFlows] = useState<LLMFlow[]>([]);
  const [sessions, setSessions] = useState<FlowSession[]>([]);
  const [activeTab, setActiveTab] = useState<"session" | "similar">("session");

  useEffect(() => {
    const loadRelatedFlows = async () => {
      setLoading(true);
      try {
        // 获取当前 Flow 所属的会话
        const flowSessions = await safeInvoke<string[]>(
          "get_sessions_for_flow",
          {
            flowId: flow.id,
          },
        );

        if (flowSessions.length > 0) {
          // 获取会话详情
          const sessionDetails: FlowSession[] = [];
          for (const sessionId of flowSessions) {
            const session = await safeInvoke<FlowSession | null>(
              "get_session",
              {
                sessionId,
              },
            );
            if (session) {
              sessionDetails.push(session);
            }
          }
          setSessions(sessionDetails);

          // 获取同一会话中的其他 Flow
          const relatedFlowIds = new Set<string>();
          for (const session of sessionDetails) {
            for (const fid of session.flow_ids) {
              if (fid !== flow.id) {
                relatedFlowIds.add(fid);
              }
            }
          }

          // 获取 Flow 详情
          const relatedFlows: LLMFlow[] = [];
          for (const fid of Array.from(relatedFlowIds).slice(0, maxItems)) {
            const detail = await flowMonitorApi.getFlowDetail(fid);
            if (detail) {
              relatedFlows.push(detail);
            }
          }
          setSessionFlows(relatedFlows);
        }

        // 获取相似的 Flow（相同模型和提供商）
        const queryResult = await flowMonitorApi.queryFlows(
          {
            models: [flow.request.model],
            providers: [flow.metadata.provider],
          },
          "created_at",
          true,
          1,
          maxItems + 1, // 多获取一个，因为可能包含当前 Flow
        );

        // 过滤掉当前 Flow
        const similar = queryResult.flows
          .filter((f) => f.id !== flow.id)
          .slice(0, maxItems);
        setSimilarFlows(similar);
      } catch (e) {
        console.error("Failed to load related flows:", e);
      } finally {
        setLoading(false);
      }
    };

    loadRelatedFlows();
  }, [flow.id, flow.request.model, flow.metadata.provider, maxItems]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const hasSessionFlows = sessionFlows.length > 0;
  const hasSimilarFlows = similarFlows.length > 0;

  if (!hasSessionFlows && !hasSimilarFlows) {
    return (
      <div className="rounded-lg border bg-muted/50 p-6 text-center text-muted-foreground">
        <Link2 className="h-8 w-8 mx-auto mb-2 opacity-50" />
        <p className="text-sm">暂无相关 Flow</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 标签页切换 */}
      <div className="flex border-b">
        {hasSessionFlows && (
          <button
            onClick={() => setActiveTab("session")}
            className={cn(
              "flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors",
              activeTab === "session"
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
          >
            <Folder className="h-4 w-4" />
            同一会话 ({sessionFlows.length})
          </button>
        )}
        {hasSimilarFlows && (
          <button
            onClick={() => setActiveTab("similar")}
            className={cn(
              "flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors",
              activeTab === "similar"
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
          >
            <Sparkles className="h-4 w-4" />
            相似请求 ({similarFlows.length})
          </button>
        )}
      </div>

      {/* 会话信息 */}
      {activeTab === "session" && sessions.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {sessions.map((session) => (
            <div
              key={session.id}
              className="flex items-center gap-1 px-2 py-1 rounded-full bg-muted text-xs"
            >
              <Folder className="h-3 w-3" />
              <span>{session.name}</span>
            </div>
          ))}
        </div>
      )}

      {/* Flow 列表 */}
      <div className="space-y-2">
        {(activeTab === "session" ? sessionFlows : similarFlows).map(
          (relatedFlow) => (
            <RelatedFlowItem
              key={relatedFlow.id}
              flow={relatedFlow}
              onClick={() => onFlowClick?.(relatedFlow.id)}
            />
          ),
        )}
      </div>
    </div>
  );
}

// ============================================================================
// 子组件
// ============================================================================

interface RelatedFlowItemProps {
  flow: LLMFlow;
  onClick?: () => void;
}

function RelatedFlowItem({ flow, onClick }: RelatedFlowItemProps) {
  const getStateIcon = (state: FlowState) => {
    switch (state) {
      case "Completed":
        return <CheckCircle2 className="h-4 w-4 text-green-500" />;
      case "Failed":
        return <XCircle className="h-4 w-4 text-red-500" />;
      case "Streaming":
        return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
      case "Pending":
        return <Clock className="h-4 w-4 text-yellow-500" />;
      case "Cancelled":
        return <XCircle className="h-4 w-4 text-gray-500" />;
      default:
        return <Clock className="h-4 w-4 text-muted-foreground" />;
    }
  };

  const formatTime = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return "刚刚";
    if (diffMins < 60) return `${diffMins} 分钟前`;
    if (diffHours < 24) return `${diffHours} 小时前`;
    if (diffDays < 7) return `${diffDays} 天前`;
    return date.toLocaleDateString("zh-CN");
  };

  // 获取内容预览
  const getContentPreview = () => {
    if (flow.response?.content) {
      return truncateText(flow.response.content, 100);
    }
    if (flow.request.messages.length > 0) {
      const lastMessage =
        flow.request.messages[flow.request.messages.length - 1];
      const content =
        typeof lastMessage.content === "string"
          ? lastMessage.content
          : lastMessage.content
              .filter(
                (p): p is { type: "text"; text: string } => p.type === "text",
              )
              .map((p) => p.text)
              .join(" ");
      return truncateText(content, 100);
    }
    return "";
  };

  return (
    <div
      className={cn(
        "rounded-lg border bg-card p-3 transition-colors",
        onClick && "cursor-pointer hover:bg-muted/50",
      )}
      onClick={onClick}
    >
      <div className="flex items-start gap-3">
        <div className="mt-0.5">{getStateIcon(flow.state)}</div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-sm font-medium truncate">
              {flow.request.model}
            </span>
            <span className="text-xs text-muted-foreground">
              {flow.metadata.provider}
            </span>
          </div>
          <p className="text-xs text-muted-foreground line-clamp-2">
            {getContentPreview()}
          </p>
          <div className="flex items-center gap-3 mt-2 text-xs text-muted-foreground">
            <span>{formatTime(flow.timestamps.created)}</span>
            <span>{formatLatency(flow.timestamps.duration_ms)}</span>
            {flow.response?.usage && (
              <span>
                {formatTokenCount(flow.response.usage.total_tokens)} tokens
              </span>
            )}
          </div>
        </div>
        {onClick && (
          <ChevronRight className="h-4 w-4 text-muted-foreground shrink-0" />
        )}
      </div>
    </div>
  );
}

export default RelatedFlows;
