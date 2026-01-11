/**
 * Kiro 凭证添加表单
 *
 * 支持三种模式：
 * 1. 在线登录（OAuth 授权）- Google、GitHub、AWS Builder ID
 * 2. 粘贴 JSON（直接粘贴凭证内容）
 * 3. 导入文件
 *
 * 支持两种浏览器模式：
 * - 系统浏览器：使用系统默认浏览器
 * - 指纹浏览器：使用 Playwright 指纹浏览器（绕过机器人检测）
 *
 * @module components/provider-pool/credential-forms/KiroForm
 * @description 参考 liuyun-kiro 项目实现的 Kiro 凭证添加表单
 * @description 实现 Requirements 5.1, 5.2, 5.3, 5.4 错误处理
 */

import { useState, useEffect, useRef, useCallback } from "react";
import { providerPoolApi, PlaywrightStatus } from "@/lib/api/providerPool";
import {
  checkPlaywrightAvailable,
  startKiroPlaywrightLogin,
  cancelKiroPlaywrightLogin,
} from "@/lib/api/providerPool";
import { FileImportForm } from "./FileImportForm";
import { BrowserModeSelector, BrowserMode } from "./BrowserModeSelector";
import { PlaywrightInstallGuide } from "./PlaywrightInstallGuide";
import { PlaywrightErrorDisplay } from "./PlaywrightErrorDisplay";
import {
  logPlaywrightError,
  parsePlaywrightError,
  PlaywrightErrorType,
} from "@/lib/errors/playwrightErrors";
import {
  FileText,
  FolderOpen,
  LogIn,
  Loader2,
  Copy,
  Check,
  ExternalLink,
} from "lucide-react";
import { safeListen } from "@/lib/dev-bridge";
import { open } from "@tauri-apps/plugin-shell";

interface KiroFormProps {
  name: string;
  credsFilePath: string;
  setCredsFilePath: (path: string) => void;
  onSelectFile: () => void;
  loading: boolean;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  onSuccess: () => void;
}

type KiroMode = "login" | "json" | "file";
type LoginType = "builderid" | "google" | "github";

interface BuilderIdLoginData {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

export function KiroForm({
  name,
  credsFilePath,
  setCredsFilePath,
  onSelectFile,
  loading: _loading,
  setLoading,
  setError,
  onSuccess,
}: KiroFormProps) {
  const [mode, setMode] = useState<KiroMode>("json");
  const [jsonContent, setJsonContent] = useState("");

  // 浏览器模式状态
  const [browserMode, setBrowserMode] = useState<BrowserMode>("system");
  const [playwrightStatus, setPlaywrightStatus] = useState<PlaywrightStatus>({
    available: false,
  });
  const [playwrightChecking, setPlaywrightChecking] = useState(false);

  // 登录相关状态
  const [_loginType, setLoginType] = useState<LoginType>("builderid");
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [builderIdLoginData, setBuilderIdLoginData] =
    useState<BuilderIdLoginData | null>(null);
  const [copied, setCopied] = useState(false);
  const pollIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  // Playwright 错误状态（用于显示详细错误信息）
  // Requirements: 5.1, 5.2, 5.4
  const [playwrightError, setPlaywrightError] = useState<unknown>(null);
  const [lastLoginProvider, setLastLoginProvider] = useState<
    "Google" | "Github" | "BuilderId" | null
  >(null);

  // 检查 Playwright 可用性
  const checkPlaywright = useCallback(async () => {
    setPlaywrightChecking(true);
    setPlaywrightError(null);
    try {
      const status = await checkPlaywrightAvailable();
      setPlaywrightStatus(status);
      if (!status.available && status.error) {
        logPlaywrightError("checkPlaywright", status.error, {
          context: "availability_check",
        });
      }
    } catch (err) {
      logPlaywrightError("checkPlaywright", err, {
        context: "availability_check",
      });
      setPlaywrightStatus({
        available: false,
        error: err instanceof Error ? err.message : "检测失败",
      });
    } finally {
      setPlaywrightChecking(false);
    }
  }, []);

  // 初始化时检查 Playwright 可用性
  useEffect(() => {
    checkPlaywright();
  }, [checkPlaywright]);

  // 清理轮询和事件监听
  useEffect(() => {
    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, []);

  // 清除 Playwright 错误
  const clearPlaywrightError = useCallback(() => {
    setPlaywrightError(null);
    setError(null);
  }, [setError]);

  // 切换到系统浏览器模式
  const switchToSystemBrowser = useCallback(() => {
    setBrowserMode("system");
    clearPlaywrightError();
  }, [clearPlaywrightError]);

  // 复制 user_code
  const handleCopyUserCode = async () => {
    if (builderIdLoginData?.userCode) {
      await navigator.clipboard.writeText(builderIdLoginData.userCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  // 使用 Playwright 指纹浏览器登录
  // Requirements: 5.1, 5.2, 5.3, 5.4
  const handlePlaywrightLogin = async (
    provider: "Google" | "Github" | "BuilderId",
  ) => {
    setIsLoggingIn(true);
    setError(null);
    setPlaywrightError(null);
    setLastLoginProvider(provider);

    try {
      const trimmedName = name.trim() || undefined;
      await startKiroPlaywrightLogin(provider, trimmedName);
      onSuccess();
    } catch (e) {
      // 记录详细错误日志 (Requirements: 5.4)
      logPlaywrightError("handlePlaywrightLogin", e, {
        provider,
        browserMode,
        name: name.trim() || undefined,
      });

      // 解析错误类型
      const errorInfo = parsePlaywrightError(e);

      // 设置 Playwright 错误状态（用于显示详细错误组件）
      setPlaywrightError(e);

      // 根据错误类型设置用户友好的错误消息
      // Requirements: 5.1, 5.2
      if (
        errorInfo.type === PlaywrightErrorType.USER_CANCELLED ||
        errorInfo.type === PlaywrightErrorType.BROWSER_CLOSED
      ) {
        // 用户主动取消，不显示为错误
        setError(null);
      } else {
        setError(errorInfo.message);
      }
    } finally {
      setIsLoggingIn(false);
    }
  };

  // 重试 Playwright 登录
  const handleRetryPlaywrightLogin = useCallback(() => {
    if (lastLoginProvider) {
      handlePlaywrightLogin(lastLoginProvider);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lastLoginProvider]);

  // 启动 Social Auth 登录 (Google/GitHub)
  const handleStartSocialAuthLogin = async (provider: "Google" | "Github") => {
    // 如果选择了指纹浏览器模式，使用 Playwright 登录
    if (browserMode === "playwright") {
      await handlePlaywrightLogin(provider);
      return;
    }

    // 系统浏览器模式
    setIsLoggingIn(true);
    setError(null);
    setBuilderIdLoginData(null);

    try {
      // 启动回调服务器
      await providerPoolApi.startKiroSocialAuthCallbackServer();

      // 监听回调事件
      const unlisten = await safeListen<{ code: string; state: string }>(
        "kiro-social-auth-callback",
        async (event) => {
          try {
            // 交换 Token
            const tokenResult =
              await providerPoolApi.exchangeKiroSocialAuthToken(
                event.payload.code,
                event.payload.state,
              );

            if (tokenResult.success) {
              // 添加凭证到凭证池
              const trimmedName = name.trim() || undefined;
              await providerPoolApi.addKiroFromBuilderIdAuth(trimmedName);
              onSuccess();
            } else {
              setError(tokenResult.error || "Token 交换失败");
            }
          } catch (e) {
            setError(e instanceof Error ? e.message : "登录失败");
          } finally {
            setIsLoggingIn(false);
          }
        },
      );
      unlistenRef.current = unlisten;

      // 启动登录
      const result = await providerPoolApi.startKiroSocialAuthLogin(provider);

      if (result.success && result.loginUrl) {
        // 打开系统默认浏览器
        await open(result.loginUrl);
      } else {
        setError(result.error || "启动登录失败");
        setIsLoggingIn(false);
        if (unlistenRef.current) {
          unlistenRef.current();
          unlistenRef.current = null;
        }
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "启动登录失败");
      setIsLoggingIn(false);
    }
  };

  // 启动 Builder ID 登录
  const handleStartBuilderIdLogin = async () => {
    // 如果选择了指纹浏览器模式，使用 Playwright 登录
    if (browserMode === "playwright") {
      await handlePlaywrightLogin("BuilderId");
      return;
    }

    // 系统浏览器模式
    setIsLoggingIn(true);
    setError(null);
    setBuilderIdLoginData(null);

    try {
      const result = await providerPoolApi.startKiroBuilderIdLogin();

      if (result.userCode && result.verificationUri) {
        setBuilderIdLoginData({
          userCode: result.userCode,
          verificationUri: result.verificationUri,
          expiresIn: result.expiresIn || 600,
          interval: result.interval || 5,
        });

        // 打开浏览器
        await open(result.verificationUri);

        // 监听授权完成事件
        const unlisten = await safeListen<{ uuid: string }>(
          "kiro-builderid-auth-complete",
          () => {
            // 授权完成
            setIsLoggingIn(false);
            setBuilderIdLoginData(null);
            onSuccess();
          },
        );
        unlistenRef.current = unlisten;

        // 开始轮询
        startPolling(result.interval || 5);
      } else {
        setError(result.error || "启动登录失败");
        setIsLoggingIn(false);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "启动登录失败");
      setIsLoggingIn(false);
    }
  };

  // 开始轮询 Builder ID 授权
  const startPolling = (interval: number) => {
    if (pollIntervalRef.current) {
      clearInterval(pollIntervalRef.current);
    }

    pollIntervalRef.current = setInterval(async () => {
      try {
        const result = await providerPoolApi.pollKiroBuilderIdAuth();

        if (!result.success) {
          if (
            result.error?.includes("过期") ||
            result.error?.includes("超时")
          ) {
            setError("授权已过期，请重新登录");
            setIsLoggingIn(false);
            setBuilderIdLoginData(null);
            if (pollIntervalRef.current) {
              clearInterval(pollIntervalRef.current);
              pollIntervalRef.current = null;
            }
          }
          return;
        }

        if (result.completed) {
          if (pollIntervalRef.current) {
            clearInterval(pollIntervalRef.current);
            pollIntervalRef.current = null;
          }

          // 添加凭证到凭证池
          const trimmedName = name.trim() || undefined;
          await providerPoolApi.addKiroFromBuilderIdAuth(trimmedName);

          setIsLoggingIn(false);
          setBuilderIdLoginData(null);
          onSuccess();
        }
        // 如果是 pending，继续轮询
      } catch (e) {
        console.error("[KiroForm] Poll error:", e);
      }
    }, interval * 1000);
  };

  // 取消登录
  // Requirements: 5.3
  const handleCancelLogin = async () => {
    if (pollIntervalRef.current) {
      clearInterval(pollIntervalRef.current);
      pollIntervalRef.current = null;
    }

    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }

    // 取消 Builder ID 和 Social Auth 登录
    await providerPoolApi.cancelKiroBuilderIdLogin();
    await providerPoolApi.cancelKiroSocialAuthLogin();

    // 如果是 Playwright 模式，也取消 Playwright 登录
    // Requirements: 5.3
    if (browserMode === "playwright") {
      try {
        await cancelKiroPlaywrightLogin();
        logPlaywrightError("handleCancelLogin", "用户取消登录", {
          provider: lastLoginProvider,
          browserMode,
        });
      } catch (e) {
        // 取消操作的错误不需要显示给用户
        console.warn("[KiroForm] Cancel Playwright login error:", e);
      }
    }

    setIsLoggingIn(false);
    setBuilderIdLoginData(null);
    setError(null);
    setPlaywrightError(null);
  };

  // JSON 粘贴提交
  const handleJsonSubmit = async () => {
    if (!jsonContent.trim()) {
      setError("请粘贴凭证 JSON 内容");
      return;
    }

    // 验证 JSON 格式
    try {
      JSON.parse(jsonContent);
    } catch {
      setError("JSON 格式无效，请检查内容");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const trimmedName = name.trim() || undefined;
      await providerPoolApi.addKiroFromJson(jsonContent, trimmedName);
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  // 文件导入提交
  const handleFileSubmit = async () => {
    if (!credsFilePath) {
      setError("请选择凭证文件");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const trimmedName = name.trim() || undefined;
      await providerPoolApi.addKiroOAuth(credsFilePath, trimmedName);
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  // 模式选择器
  const renderModeSelector = () => (
    <div className="grid grid-cols-3 gap-1 p-1 bg-muted/50 rounded-xl border mb-4">
      <button
        type="button"
        onClick={() => {
          setMode("login");
          setError(null);
        }}
        disabled={isLoggingIn}
        className={`py-2 px-3 text-sm rounded-lg transition-all duration-200 font-medium ${
          mode === "login"
            ? "bg-background text-foreground shadow-sm ring-1 ring-black/5"
            : "text-muted-foreground hover:text-foreground hover:bg-background/50"
        }`}
      >
        <LogIn className="inline h-4 w-4 mr-1" />
        在线登录
      </button>
      <button
        type="button"
        onClick={() => {
          setMode("json");
          setError(null);
        }}
        disabled={isLoggingIn}
        className={`py-2 px-3 text-sm rounded-lg transition-all duration-200 font-medium ${
          mode === "json"
            ? "bg-background text-foreground shadow-sm ring-1 ring-black/5"
            : "text-muted-foreground hover:text-foreground hover:bg-background/50"
        }`}
      >
        <FileText className="inline h-4 w-4 mr-1" />
        粘贴 JSON
      </button>
      <button
        type="button"
        onClick={() => {
          setMode("file");
          setError(null);
        }}
        disabled={isLoggingIn}
        className={`py-2 px-3 text-sm rounded-lg transition-all duration-200 font-medium ${
          mode === "file"
            ? "bg-background text-foreground shadow-sm ring-1 ring-black/5"
            : "text-muted-foreground hover:text-foreground hover:bg-background/50"
        }`}
      >
        <FolderOpen className="inline h-4 w-4 mr-1" />
        导入文件
      </button>
    </div>
  );

  // 在线登录表单
  const renderLoginForm = () => (
    <div className="space-y-4">
      {/* 浏览器模式选择器 */}
      {!isLoggingIn && (
        <BrowserModeSelector
          mode={browserMode}
          onModeChange={setBrowserMode}
          playwrightAvailable={playwrightStatus.available}
          playwrightChecking={playwrightChecking}
          onCheckPlaywright={checkPlaywright}
          disabled={isLoggingIn}
        />
      )}

      {/* Playwright 安装引导（当选择指纹浏览器但未安装时显示） */}
      {!isLoggingIn &&
        browserMode === "playwright" &&
        !playwrightStatus.available &&
        !playwrightChecking && (
          <PlaywrightInstallGuide
            onRetryCheck={checkPlaywright}
            checking={playwrightChecking}
          />
        )}

      {/* Playwright 错误显示（当有错误且不在登录中时显示） */}
      {/* Requirements: 5.1, 5.2, 5.4 */}
      {!isLoggingIn &&
        playwrightError !== null &&
        browserMode === "playwright" && (
          <PlaywrightErrorDisplay
            error={playwrightError}
            onRetry={handleRetryPlaywrightLogin}
            onSwitchToSystemBrowser={switchToSystemBrowser}
            onDismiss={clearPlaywrightError}
            retrying={isLoggingIn}
          />
        )}

      {/* 登录中状态 - Builder ID */}
      {isLoggingIn && builderIdLoginData && (
        <div className="space-y-4">
          <div className="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg text-center">
            <p className="text-sm text-blue-700 dark:text-blue-300 mb-2">
              请在浏览器中完成登录，并输入以下代码：
            </p>
            <div className="flex items-center justify-center gap-2">
              <code className="text-2xl font-bold tracking-widest bg-white dark:bg-gray-800 px-4 py-2 rounded border">
                {builderIdLoginData.userCode}
              </code>
              <button
                type="button"
                onClick={handleCopyUserCode}
                className="p-2 rounded-lg border hover:bg-muted"
                title="复制代码"
              >
                {copied ? (
                  <Check className="h-4 w-4 text-green-500" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </button>
            </div>
            <div className="mt-3 flex items-center justify-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" />
              等待授权中...
            </div>
          </div>

          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => open(builderIdLoginData.verificationUri)}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg border hover:bg-muted"
            >
              <ExternalLink className="h-4 w-4" />
              重新打开浏览器
            </button>
            <button
              type="button"
              onClick={handleCancelLogin}
              className="flex-1 px-4 py-2 rounded-lg bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              取消登录
            </button>
          </div>
        </div>
      )}

      {/* 登录中状态 - Social Auth / Playwright */}
      {isLoggingIn && !builderIdLoginData && (
        <div className="space-y-4">
          <div className="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg text-center">
            <Loader2 className="h-8 w-8 animate-spin mx-auto mb-2 text-blue-500" />
            <p className="text-sm text-blue-700 dark:text-blue-300">
              {browserMode === "playwright"
                ? "正在使用指纹浏览器登录..."
                : "请在浏览器中完成登录..."}
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              {browserMode === "playwright"
                ? "请在弹出的浏览器窗口中完成登录"
                : "登录完成后会自动返回"}
            </p>
          </div>

          <button
            type="button"
            onClick={handleCancelLogin}
            className="w-full px-4 py-2 rounded-lg bg-destructive text-destructive-foreground hover:bg-destructive/90"
          >
            取消登录
          </button>
        </div>
      )}

      {/* 未登录状态 - 显示登录选项 */}
      {!isLoggingIn && (
        <div className="space-y-2">
          {/* 第一行：Google 和 GitHub */}
          <div className="grid grid-cols-2 gap-2">
            {/* Google */}
            <button
              type="button"
              onClick={() => {
                setLoginType("google");
                handleStartSocialAuthLogin("Google");
              }}
              disabled={
                browserMode === "playwright" && !playwrightStatus.available
              }
              className={`group flex items-center px-3 py-2 gap-2 bg-white dark:bg-slate-900 hover:bg-slate-50 dark:hover:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg transition-all duration-200 hover:shadow-sm hover:border-primary/30 ${
                browserMode === "playwright" && !playwrightStatus.available
                  ? "opacity-50 cursor-not-allowed"
                  : ""
              }`}
            >
              <div className="w-5 h-5 flex items-center justify-center bg-white rounded-full shadow-sm border p-0.5 group-hover:scale-110 transition-transform flex-shrink-0">
                <svg viewBox="0 0 24 24" className="w-full h-full">
                  <path
                    fill="#4285F4"
                    d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                  />
                  <path
                    fill="#34A853"
                    d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                  />
                  <path
                    fill="#FBBC05"
                    d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                  />
                  <path
                    fill="#EA4335"
                    d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                  />
                </svg>
              </div>
              <span className="text-xs font-medium text-foreground">
                Google
              </span>
            </button>

            {/* GitHub */}
            <button
              type="button"
              onClick={() => {
                setLoginType("github");
                handleStartSocialAuthLogin("Github");
              }}
              disabled={
                browserMode === "playwright" && !playwrightStatus.available
              }
              className={`group flex items-center px-3 py-2 gap-2 bg-white dark:bg-slate-900 hover:bg-slate-50 dark:hover:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg transition-all duration-200 hover:shadow-sm hover:border-primary/30 ${
                browserMode === "playwright" && !playwrightStatus.available
                  ? "opacity-50 cursor-not-allowed"
                  : ""
              }`}
            >
              <div className="w-5 h-5 flex items-center justify-center bg-white rounded-full shadow-sm border p-0.5 group-hover:scale-110 transition-transform flex-shrink-0">
                <svg
                  viewBox="0 0 24 24"
                  fill="#24292f"
                  className="w-full h-full"
                >
                  <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
                </svg>
              </div>
              <span className="text-xs font-medium text-foreground">
                GitHub
              </span>
            </button>
          </div>

          {/* 第二行：AWS Builder ID */}
          <button
            type="button"
            onClick={() => {
              setLoginType("builderid");
              handleStartBuilderIdLogin();
            }}
            disabled={
              browserMode === "playwright" && !playwrightStatus.available
            }
            className={`group w-full flex items-center justify-center px-3 py-2 gap-2 bg-white dark:bg-slate-900 hover:bg-slate-50 dark:hover:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg transition-all duration-200 hover:shadow-sm hover:border-primary/30 ${
              browserMode === "playwright" && !playwrightStatus.available
                ? "opacity-50 cursor-not-allowed"
                : ""
            }`}
          >
            <div className="w-5 h-5 flex items-center justify-center bg-[#232f3e] rounded-full shadow-sm border p-0.5 group-hover:scale-110 transition-transform flex-shrink-0">
              <svg viewBox="0 0 24 24" fill="#ff9900" className="w-full h-full">
                <text
                  x="2"
                  y="16"
                  fontSize="10"
                  fontWeight="bold"
                  fontFamily="Arial"
                >
                  aws
                </text>
              </svg>
            </div>
            <span className="text-xs font-medium text-foreground">
              AWS Builder ID
            </span>
          </button>
        </div>
      )}
    </div>
  );

  // JSON 粘贴表单
  const renderJsonForm = () => (
    <div className="space-y-4">
      <div className="rounded-lg border border-blue-200 bg-blue-50 p-4 dark:border-blue-800 dark:bg-blue-950/30">
        <p className="text-sm text-blue-700 dark:text-blue-300">
          直接粘贴 Kiro 凭证 JSON 内容，无需选择文件。
        </p>
        <p className="mt-2 text-xs text-blue-600 dark:text-blue-400">
          凭证 JSON 通常包含 accessToken、refreshToken 等字段。
        </p>
      </div>

      <div>
        <label className="mb-1 block text-sm font-medium">
          凭证 JSON <span className="text-red-500">*</span>
        </label>
        <textarea
          value={jsonContent}
          onChange={(e) => setJsonContent(e.target.value)}
          placeholder={`粘贴凭证 JSON 内容，例如：
{
  "accessToken": "...",
  "refreshToken": "...",
  "region": "us-east-1",
  ...
}`}
          className="w-full h-48 rounded-lg border bg-background px-3 py-2 text-sm font-mono resize-none"
        />
      </div>
    </div>
  );

  return {
    mode,
    handleJsonSubmit,
    handleFileSubmit,
    handleLoginSubmit: () => {}, // 登录模式不需要手动提交
    render: () => (
      <>
        {renderModeSelector()}

        {mode === "login" && renderLoginForm()}
        {mode === "json" && renderJsonForm()}
        {mode === "file" && (
          <FileImportForm
            credsFilePath={credsFilePath}
            setCredsFilePath={setCredsFilePath}
            onSelectFile={onSelectFile}
            placeholder="选择 kiro-auth-token.json..."
            hint="默认路径: ~/.aws/sso/cache/kiro-auth-token.json"
          />
        )}
      </>
    ),
  };
}
