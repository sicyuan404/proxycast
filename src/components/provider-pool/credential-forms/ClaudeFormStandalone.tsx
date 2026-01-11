/**
 * Claude 凭证添加表单（自包含版本）
 *
 * 支持多种认证方式：
 * 1. Cookie 授权 - 使用 sessionKey 自动完成 OAuth 流程
 * 2. OAuth 登录 - 通过授权 URL 手动复制授权码
 * 3. 文件导入 - 导入已有的凭证文件
 *
 * @module components/provider-pool/credential-forms/ClaudeFormStandalone
 */

import { useState, useCallback, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { safeListen } from "@/lib/dev-bridge";
import { providerPoolApi } from "@/lib/api/providerPool";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Loader2, Cookie, Key, FileJson, Upload } from "lucide-react";
import { OAuthUrlDisplay } from "./OAuthUrlDisplay";

type AuthMode = "cookie" | "login" | "file";

interface ClaudeFormStandaloneProps {
  /** 添加成功回调 */
  onSuccess: () => void;
  /** 取消回调 */
  onCancel?: () => void;
  /** 初始名称 */
  initialName?: string;
  /** 认证类型（预留扩展） */
  authType?: string;
}

/**
 * 自包含的 Claude 凭证添加表单
 */
export function ClaudeFormStandalone({
  onSuccess,
  onCancel,
  initialName = "",
  authType: _authType,
}: ClaudeFormStandaloneProps) {
  const [mode, setMode] = useState<AuthMode>("cookie");
  const [name, setName] = useState(initialName);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // OAuth 状态
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [waitingForCallback, setWaitingForCallback] = useState(false);

  // Cookie 状态
  const [sessionKey, setSessionKey] = useState("");
  const [isSetupToken, setIsSetupToken] = useState(false);

  // 文件导入状态
  const [credsFilePath, setCredsFilePath] = useState("");

  // 监听后端发送的授权 URL 事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await safeListen<{ auth_url: string }>(
        "claude-oauth-auth-url",
        (event) => {
          setAuthUrl(event.payload.auth_url);
        },
      );
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 获取授权 URL 并启动服务器等待回调
  const handleGetAuthUrl = useCallback(async () => {
    setLoading(true);
    setError(null);
    setAuthUrl(null);
    setWaitingForCallback(true);

    try {
      await providerPoolApi.getClaudeOAuthAuthUrlAndWait(
        name.trim() || undefined,
      );
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setWaitingForCallback(false);
    } finally {
      setLoading(false);
    }
  }, [name, onSuccess]);

  // Cookie 自动授权
  const handleCookieSubmit = useCallback(async () => {
    if (!sessionKey.trim()) {
      setError("请输入 sessionKey");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await providerPoolApi.claudeOAuthWithCookie(
        sessionKey.trim(),
        isSetupToken,
        name.trim() || undefined,
      );
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [sessionKey, isSetupToken, name, onSuccess]);

  // 选择文件
  const handleSelectFile = useCallback(async () => {
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
  }, []);

  // 文件导入提交
  const handleFileSubmit = useCallback(async () => {
    if (!credsFilePath) {
      setError("请选择凭证文件");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await providerPoolApi.addClaudeOAuth(
        credsFilePath,
        name.trim() || undefined,
      );
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [credsFilePath, name, onSuccess]);

  return (
    <div className="space-y-4">
      {/* 名称输入 */}
      <div>
        <label className="mb-1 block text-sm font-medium">名称 (可选)</label>
        <Input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="给这个凭证起个名字..."
          disabled={loading}
        />
      </div>

      {/* 模式选择器 */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => setMode("cookie")}
          className={`flex flex-1 items-center justify-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors ${
            mode === "cookie"
              ? "border-amber-500 bg-amber-50 text-amber-700 dark:bg-amber-950/30 dark:text-amber-300"
              : "hover:bg-muted"
          }`}
        >
          <Cookie className="h-4 w-4" />
          Cookie 授权
        </button>
        <button
          type="button"
          onClick={() => setMode("login")}
          className={`flex flex-1 items-center justify-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors ${
            mode === "login"
              ? "border-amber-500 bg-amber-50 text-amber-700 dark:bg-amber-950/30 dark:text-amber-300"
              : "hover:bg-muted"
          }`}
        >
          <Key className="h-4 w-4" />
          OAuth 登录
        </button>
        <button
          type="button"
          onClick={() => setMode("file")}
          className={`flex flex-1 items-center justify-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors ${
            mode === "file"
              ? "border-amber-500 bg-amber-50 text-amber-700 dark:bg-amber-950/30 dark:text-amber-300"
              : "hover:bg-muted"
          }`}
        >
          <FileJson className="h-4 w-4" />
          导入文件
        </button>
      </div>

      {/* Cookie 授权表单 */}
      {mode === "cookie" && (
        <div className="space-y-4">
          <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-950/30">
            <p className="text-sm text-amber-700 dark:text-amber-300">
              使用浏览器 Cookie 中的 sessionKey 自动完成 OAuth
              授权，无需手动复制授权码。
            </p>
            <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
              获取方式：在 claude.ai 登录后，打开开发者工具 → Application →
              Cookies → 复制 sessionKey 的值
            </p>
          </div>

          <div>
            <label className="mb-1 block text-sm font-medium">
              sessionKey <span className="text-red-500">*</span>
            </label>
            <Textarea
              value={sessionKey}
              onChange={(e) => setSessionKey(e.target.value)}
              placeholder="粘贴从浏览器 Cookie 中获取的 sessionKey..."
              className="font-mono"
              rows={3}
            />
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="isSetupToken"
              checked={isSetupToken}
              onChange={(e) => setIsSetupToken(e.target.checked)}
              className="h-4 w-4 rounded border-gray-300"
            />
            <label
              htmlFor="isSetupToken"
              className="text-sm text-muted-foreground"
            >
              Setup Token 模式（只需推理权限，无 refresh_token）
            </label>
          </div>
        </div>
      )}

      {/* OAuth 登录表单 */}
      {mode === "login" && (
        <div className="space-y-4">
          <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-950/30">
            <p className="text-sm text-amber-700 dark:text-amber-300">
              点击下方按钮获取授权 URL，然后复制到浏览器（支持指纹浏览器）完成
              Claude 登录。
            </p>
            <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
              授权成功后会自动完成，无需手动复制授权码。
            </p>
          </div>

          <OAuthUrlDisplay
            authUrl={authUrl}
            waitingForCallback={waitingForCallback}
            colorScheme="amber"
          />
        </div>
      )}

      {/* 文件导入表单 */}
      {mode === "file" && (
        <div className="space-y-4">
          <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-950/30">
            <p className="text-sm text-amber-700 dark:text-amber-300">
              导入已有的 Claude OAuth 凭证文件。
            </p>
            <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
              默认路径: ~/.claude/oauth.json 或 Claude CLI 的凭证文件
            </p>
          </div>

          <div className="flex gap-2">
            <Input
              type="text"
              value={credsFilePath}
              onChange={(e) => setCredsFilePath(e.target.value)}
              placeholder="选择 oauth.json 或 oauth_creds.json..."
              className="flex-1"
            />
            <Button type="button" variant="outline" onClick={handleSelectFile}>
              <Upload className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}

      {/* 错误提示 */}
      {error && (
        <div className="rounded-lg border border-red-300 bg-red-50 dark:bg-red-900/20 p-3 text-sm text-red-700 dark:text-red-300">
          {error}
        </div>
      )}

      {/* 按钮区域 */}
      <div className="flex justify-end gap-2 pt-2">
        {onCancel && (
          <Button
            type="button"
            variant="outline"
            onClick={onCancel}
            disabled={loading}
          >
            取消
          </Button>
        )}
        {mode === "cookie" && (
          <Button
            type="button"
            onClick={handleCookieSubmit}
            disabled={loading || !sessionKey.trim()}
          >
            {loading ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                授权中...
              </>
            ) : (
              "添加凭证"
            )}
          </Button>
        )}
        {mode === "login" && !authUrl && (
          <Button type="button" onClick={handleGetAuthUrl} disabled={loading}>
            {loading ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                获取中...
              </>
            ) : (
              "获取授权 URL"
            )}
          </Button>
        )}
        {mode === "file" && (
          <Button
            type="button"
            onClick={handleFileSubmit}
            disabled={loading || !credsFilePath}
          >
            {loading ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                导入中...
              </>
            ) : (
              "导入凭证"
            )}
          </Button>
        )}
      </div>
    </div>
  );
}

export default ClaudeFormStandalone;
