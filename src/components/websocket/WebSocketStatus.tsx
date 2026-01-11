import React, { useEffect, useState } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import {
  Wifi,
  WifiOff,
  Users,
  MessageSquare,
  AlertCircle,
  RefreshCw,
  ChevronDown,
  ChevronUp,
} from "lucide-react";

interface WsServiceStatus {
  enabled: boolean;
  active_connections: number;
  total_connections: number;
  total_messages: number;
  total_errors: number;
}

interface WsConnectionInfo {
  id: string;
  connected_at: string;
  client_info: string | null;
  request_count: number;
}

export function WebSocketStatus() {
  const [status, setStatus] = useState<WsServiceStatus | null>(null);
  const [connections, setConnections] = useState<WsConnectionInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showConnections, setShowConnections] = useState(false);

  const fetchStatus = async () => {
    try {
      const wsStatus = await safeInvoke<WsServiceStatus>(
        "get_websocket_status",
      );
      setStatus(wsStatus);

      if (wsStatus.active_connections > 0) {
        const wsConnections = await safeInvoke<WsConnectionInfo[]>(
          "get_websocket_connections",
        );
        setConnections(wsConnections);
      } else {
        setConnections([]);
      }

      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleString();
  };

  const truncateId = (id: string) => {
    return id.length > 8 ? `${id.slice(0, 8)}...` : id;
  };

  if (loading) {
    return (
      <div className="rounded-lg border bg-card p-4">
        <div className="flex items-center gap-2 text-muted-foreground">
          <RefreshCw className="h-4 w-4 animate-spin" />
          <span>加载中...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border bg-card p-4">
        <div className="flex items-center gap-2 text-red-500">
          <AlertCircle className="h-4 w-4" />
          <span>加载失败: {error}</span>
        </div>
      </div>
    );
  }

  if (!status) {
    return null;
  }

  return (
    <div className="space-y-4">
      {/* 状态概览 */}
      <div className="rounded-lg border bg-card p-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold flex items-center gap-2">
            {status.enabled ? (
              <Wifi className="h-4 w-4 text-green-500" />
            ) : (
              <WifiOff className="h-4 w-4 text-muted-foreground" />
            )}
            WebSocket 服务
          </h3>
          <button
            onClick={fetchStatus}
            className="p-1 hover:bg-muted rounded"
            title="刷新"
          >
            <RefreshCw className="h-4 w-4" />
          </button>
        </div>

        <div className="grid grid-cols-4 gap-4">
          <StatCard
            icon={Users}
            label="活跃连接"
            value={status.active_connections}
            highlight={status.active_connections > 0}
          />
          <StatCard
            icon={Users}
            label="总连接数"
            value={status.total_connections}
          />
          <StatCard
            icon={MessageSquare}
            label="总消息数"
            value={status.total_messages}
          />
          <StatCard
            icon={AlertCircle}
            label="错误数"
            value={status.total_errors}
            highlight={status.total_errors > 0}
            highlightColor="text-red-500"
          />
        </div>
      </div>

      {/* 连接列表 */}
      {status.active_connections > 0 && (
        <div className="rounded-lg border bg-card">
          <button
            onClick={() => setShowConnections(!showConnections)}
            className="w-full p-4 flex items-center justify-between hover:bg-muted/50"
          >
            <span className="font-semibold">
              活跃连接 ({status.active_connections})
            </span>
            {showConnections ? (
              <ChevronUp className="h-4 w-4" />
            ) : (
              <ChevronDown className="h-4 w-4" />
            )}
          </button>

          {showConnections && (
            <div className="border-t">
              {connections.length === 0 ? (
                <div className="p-4 text-center text-muted-foreground">
                  暂无连接数据
                </div>
              ) : (
                <div className="divide-y">
                  {connections.map((conn) => (
                    <div
                      key={conn.id}
                      className="p-4 flex items-center justify-between"
                    >
                      <div>
                        <div className="font-mono text-sm">
                          {truncateId(conn.id)}
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {conn.client_info || "未知客户端"}
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="text-sm">{conn.request_count} 请求</div>
                        <div className="text-xs text-muted-foreground">
                          {formatDate(conn.connected_at)}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function StatCard({
  icon: Icon,
  label,
  value,
  highlight = false,
  highlightColor = "text-green-500",
}: {
  icon: React.ElementType;
  label: string;
  value: number;
  highlight?: boolean;
  highlightColor?: string;
}) {
  return (
    <div className="text-center">
      <div className="flex items-center justify-center gap-1 text-muted-foreground mb-1">
        <Icon className="h-3 w-3" />
        <span className="text-xs">{label}</span>
      </div>
      <div className={`text-xl font-bold ${highlight ? highlightColor : ""}`}>
        {value}
      </div>
    </div>
  );
}

export default WebSocketStatus;
