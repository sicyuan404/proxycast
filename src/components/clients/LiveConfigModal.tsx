import { useState, useEffect, useCallback } from "react";
import { X, RefreshCw, FileText } from "lucide-react";
import { switchApi, AppType } from "@/lib/api/switch";

interface LiveConfigModalProps {
  appType: AppType;
  onClose: () => void;
}

const configPaths: Record<AppType, string> = {
  claude: "~/.claude.json",
  codex: "~/.codex/auth.json & config.toml",
  gemini: "~/.gemini/.env & settings.json",
  proxycast: "",
};

export function LiveConfigModal({ appType, onClose }: LiveConfigModalProps) {
  const [config, setConfig] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await switchApi.readLiveSettings(appType);
      setConfig(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [appType]);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-background rounded-xl shadow-lg w-full max-w-2xl max-h-[80vh] overflow-hidden border border-border">
        <div className="flex items-center justify-between p-4 border-b">
          <div className="flex items-center gap-2">
            <FileText className="h-5 w-5 text-primary" />
            <h3 className="text-lg font-semibold">当前生效的配置</h3>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={loadConfig}
              disabled={loading}
              className="p-1.5 rounded hover:bg-muted"
              title="刷新"
            >
              <RefreshCw
                className={`h-4 w-4 ${loading ? "animate-spin" : ""}`}
              />
            </button>
            <button onClick={onClose} className="p-1.5 rounded hover:bg-muted">
              <X className="h-5 w-5" />
            </button>
          </div>
        </div>

        <div className="p-4 overflow-auto max-h-[60vh]">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          ) : error ? (
            <div className="rounded-lg border border-destructive bg-destructive/10 p-4">
              <p className="text-destructive">{error}</p>
            </div>
          ) : config ? (
            <pre className="p-4 rounded-lg bg-muted/50 font-mono text-sm overflow-auto whitespace-pre-wrap">
              {JSON.stringify(config, null, 2)}
            </pre>
          ) : (
            <p className="text-muted-foreground text-center py-8">无配置数据</p>
          )}
        </div>

        <div className="p-4 border-t bg-muted/30">
          <p className="text-xs text-muted-foreground">
            配置文件路径:{" "}
            <code className="px-1 py-0.5 rounded bg-muted">
              {configPaths[appType]}
            </code>
          </p>
        </div>
      </div>
    </div>
  );
}
