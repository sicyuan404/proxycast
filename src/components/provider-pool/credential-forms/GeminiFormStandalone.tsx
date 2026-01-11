/**
 * Gemini 凭证添加表单（自包含版本）
 *
 * 支持两种认证方式：
 * 1. Google OAuth - 使用 Google 账户授权
 * 2. API Key - 使用 Google AI Studio API Key
 *
 * @module components/provider-pool/credential-forms/GeminiFormStandalone
 */

import { useState, useCallback, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { safeListen } from "@/lib/dev-bridge";
import { providerPoolApi } from "@/lib/api/providerPool";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Loader2,
  Key,
  KeyRound,
  Copy,
  Check,
  ExternalLink,
  Upload,
} from "lucide-react";

type AuthMethod = "oauth" | "api_key";

interface GeminiFormStandaloneProps {
  /** 添加成功回调 */
  onSuccess: () => void;
  /** 取消回调 */
  onCancel?: () => void;
  /** 初始名称 */
  initialName?: string;
  /** 初始认证方式 */
  initialAuthMethod?: AuthMethod;
}

/**
 * 自包含的 Gemini 凭证添加表单
 *
 * 内部管理所有状态，只需要提供 onSuccess 和 onCancel 回调
 */
export function GeminiFormStandalone({
  onSuccess,
  onCancel,
  initialName = "",
  initialAuthMethod = "oauth",
}: GeminiFormStandaloneProps) {
  const [authMethod, setAuthMethod] = useState<AuthMethod>(initialAuthMethod);
  const [name, setName] = useState(initialName);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // OAuth 状态
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [authCode, setAuthCode] = useState("");
  const [copied, setCopied] = useState(false);
  const [exchanging, setExchanging] = useState(false);

  // 文件导入状态
  const [credsFilePath, setCredsFilePath] = useState("");
  const [projectId, setProjectId] = useState("");

  // API Key 状态
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");

  // 监听后端发送的授权 URL 事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await safeListen<{ auth_url: string; session_id: string }>(
        "gemini-auth-url",
        (event) => {
          console.log("[Gemini OAuth] 收到授权 URL 事件:", event.payload);
          setAuthUrl(event.payload.auth_url);
          setSessionId(event.payload.session_id);
        },
      );
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 获取授权 URL
  const handleGetAuthUrl = useCallback(async () => {
    setLoading(true);
    setError(null);
    setAuthUrl(null);
    setSessionId(null);
    setAuthCode("");

    try {
      await providerPoolApi.getGeminiAuthUrlAndWait(name.trim() || undefined);
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      if (errorMsg.includes("AUTH_URL:")) {
        const urlMatch = errorMsg.match(/AUTH_URL:(.+?)(?:\s|$)/);
        if (urlMatch) {
          setAuthUrl(urlMatch[1]);
        }
      } else {
        setError(errorMsg);
      }
    } finally {
      setLoading(false);
    }
  }, [name]);

  // 用 code 交换 token
  const handleExchangeCode = useCallback(async () => {
    if (!authCode.trim()) {
      setError("请输入授权码");
      return;
    }

    setExchanging(true);
    setError(null);

    try {
      await providerPoolApi.exchangeGeminiCode(
        authCode.trim(),
        sessionId || undefined,
        name.trim() || undefined,
      );
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setExchanging(false);
    }
  }, [authCode, sessionId, name, onSuccess]);

  // 复制 URL
  const handleCopyUrl = useCallback(async () => {
    if (authUrl) {
      await navigator.clipboard.writeText(authUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [authUrl]);

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
      await providerPoolApi.addGeminiOAuth(
        credsFilePath,
        projectId.trim() || undefined,
        name.trim() || undefined,
      );
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [credsFilePath, projectId, name, onSuccess]);

  // API Key 提交
  const handleApiKeySubmit = useCallback(async () => {
    if (!apiKey.trim()) {
      setError("请输入 API Key");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await providerPoolApi.addGeminiApiKey(
        apiKey.trim(),
        baseUrl.trim() || undefined,
        undefined,
        name.trim() || undefined,
      );
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [apiKey, baseUrl, name, onSuccess]);

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
          disabled={loading || exchanging}
        />
      </div>

      {/* 认证方式选择 */}
      <Tabs
        value={authMethod}
        onValueChange={(v) => setAuthMethod(v as AuthMethod)}
      >
        <TabsList className="grid grid-cols-2">
          <TabsTrigger value="oauth" className="flex items-center gap-2">
            <Key className="h-4 w-4" />
            Google OAuth
          </TabsTrigger>
          <TabsTrigger value="api_key" className="flex items-center gap-2">
            <KeyRound className="h-4 w-4" />
            API Key
          </TabsTrigger>
        </TabsList>

        {/* OAuth 认证 */}
        <TabsContent value="oauth" className="space-y-4 mt-4">
          <div className="rounded-lg border border-blue-200 bg-blue-50 p-4 dark:border-blue-800 dark:bg-blue-950/30">
            <p className="text-sm text-blue-700 dark:text-blue-300">
              点击下方按钮获取授权 URL，然后复制到浏览器完成 Google 登录。
            </p>
            <p className="mt-2 text-xs text-blue-600 dark:text-blue-400">
              授权成功后，复制页面显示的授权码粘贴到下方输入框。
            </p>
          </div>

          {!authUrl ? (
            <Button
              onClick={handleGetAuthUrl}
              disabled={loading}
              className="w-full"
            >
              {loading ? (
                <>
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  获取授权 URL...
                </>
              ) : (
                <>
                  <ExternalLink className="h-4 w-4 mr-2" />
                  获取授权 URL
                </>
              )}
            </Button>
          ) : (
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">授权 URL</span>
                <button
                  onClick={handleCopyUrl}
                  className="flex items-center gap-1 rounded px-2 py-1 text-xs text-blue-600 hover:bg-blue-100 dark:text-blue-400 dark:hover:bg-blue-900/30"
                >
                  {copied ? (
                    <>
                      <Check className="h-3 w-3" />
                      已复制
                    </>
                  ) : (
                    <>
                      <Copy className="h-3 w-3" />
                      复制
                    </>
                  )}
                </button>
              </div>
              <div className="rounded-lg border bg-muted/50 p-3">
                <p className="break-all text-xs text-muted-foreground">
                  {authUrl.length > 100
                    ? `${authUrl.slice(0, 100)}...`
                    : authUrl}
                </p>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">
                  授权码 <span className="text-red-500">*</span>
                </label>
                <Input
                  type="text"
                  value={authCode}
                  onChange={(e) => setAuthCode(e.target.value)}
                  placeholder="粘贴浏览器页面显示的授权码..."
                />
                <p className="text-xs text-muted-foreground">
                  在浏览器中完成授权后，复制页面显示的授权码
                </p>
              </div>

              <Button
                onClick={handleExchangeCode}
                disabled={exchanging || !authCode.trim()}
                className="w-full"
              >
                {exchanging ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    验证中...
                  </>
                ) : (
                  "验证授权码"
                )}
              </Button>
            </div>
          )}

          {/* 文件导入选项 */}
          <div className="border-t pt-4">
            <p className="text-sm text-muted-foreground mb-3">
              或者导入已有的凭证文件：
            </p>
            <div className="space-y-3">
              <div className="flex gap-2">
                <Input
                  type="text"
                  value={credsFilePath}
                  onChange={(e) => setCredsFilePath(e.target.value)}
                  placeholder="选择 oauth_creds.json..."
                  className="flex-1"
                />
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleSelectFile}
                >
                  <Upload className="h-4 w-4" />
                </Button>
              </div>
              <Input
                type="text"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
                placeholder="Project ID (可选)"
              />
              <Button
                onClick={handleFileSubmit}
                disabled={loading || !credsFilePath}
                variant="outline"
                className="w-full"
              >
                {loading ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    导入中...
                  </>
                ) : (
                  "导入凭证文件"
                )}
              </Button>
            </div>
          </div>
        </TabsContent>

        {/* API Key 认证 */}
        <TabsContent value="api_key" className="space-y-4 mt-4">
          <div className="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-950/30">
            <p className="text-sm text-green-700 dark:text-green-300">
              使用 Google AI Studio 的 API Key 进行认证。
            </p>
            <p className="mt-2 text-xs text-green-600 dark:text-green-400">
              从{" "}
              <a
                href="https://aistudio.google.com/app/apikey"
                target="_blank"
                rel="noopener noreferrer"
                className="underline hover:no-underline"
              >
                Google AI Studio
              </a>{" "}
              获取 API Key。
            </p>
          </div>

          <div className="space-y-3">
            <div>
              <label className="mb-1 block text-sm font-medium">
                API Key <span className="text-red-500">*</span>
              </label>
              <Input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="AIzaSy..."
              />
            </div>

            <div>
              <label className="mb-1 block text-sm font-medium">
                Base URL (可选)
              </label>
              <Input
                type="text"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                placeholder="https://generativelanguage.googleapis.com"
              />
              <p className="mt-1 text-xs text-muted-foreground">
                留空使用官方 API
              </p>
            </div>
          </div>
        </TabsContent>
      </Tabs>

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
            disabled={loading || exchanging}
          >
            取消
          </Button>
        )}
        {authMethod === "api_key" && (
          <Button
            type="button"
            onClick={handleApiKeySubmit}
            disabled={loading || !apiKey.trim()}
          >
            {loading ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                添加中...
              </>
            ) : (
              "添加凭证"
            )}
          </Button>
        )}
      </div>
    </div>
  );
}

export default GeminiFormStandalone;
