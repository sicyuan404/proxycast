/**
 * Claude OAuth 凭证添加表单
 * 支持 Claude OAuth 登录和文件导入两种模式
 */

import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { providerPoolApi } from "@/lib/api/providerPool";
import { ModeSelector } from "./ModeSelector";
import { FileImportForm } from "./FileImportForm";
import { OAuthUrlDisplay } from "./OAuthUrlDisplay";

interface ClaudeOAuthFormProps {
  name: string;
  credsFilePath: string;
  setCredsFilePath: (path: string) => void;
  onSelectFile: () => void;
  loading: boolean;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  onSuccess: () => void;
}

export function ClaudeOAuthForm({
  name,
  credsFilePath,
  setCredsFilePath,
  onSelectFile,
  loading: _loading,
  setLoading,
  setError,
  onSuccess,
}: ClaudeOAuthFormProps) {
  const [mode, setMode] = useState<"login" | "file">("login");
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [waitingForCallback, setWaitingForCallback] = useState(false);

  // 监听后端发送的授权 URL 事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<{ auth_url: string }>(
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
  const handleGetAuthUrl = async () => {
    setLoading(true);
    setError(null);
    setAuthUrl(null);
    setWaitingForCallback(true);

    try {
      const trimmedName = name.trim() || undefined;
      await providerPoolApi.getClaudeOAuthAuthUrlAndWait(trimmedName);
      onSuccess();
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      setWaitingForCallback(false);
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
      await providerPoolApi.addClaudeOAuth(credsFilePath, trimmedName);
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return {
    mode,
    authUrl,
    waitingForCallback,
    handleGetAuthUrl,
    handleFileSubmit,
    render: () => (
      <>
        <ModeSelector
          mode={mode}
          setMode={setMode}
          loginLabel="Claude 登录"
          fileLabel="导入文件"
        />

        {mode === "login" ? (
          <div className="space-y-4">
            <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-950/30">
              <p className="text-sm text-amber-700 dark:text-amber-300">
                点击下方按钮获取授权 URL，然后复制到浏览器（支持指纹浏览器）完成
                Claude 登录。
              </p>
              <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
                授权成功后，凭证将自动保存并添加到凭证池。
              </p>
            </div>

            <OAuthUrlDisplay
              authUrl={authUrl}
              waitingForCallback={waitingForCallback}
              colorScheme="amber"
            />
          </div>
        ) : (
          <FileImportForm
            credsFilePath={credsFilePath}
            setCredsFilePath={setCredsFilePath}
            onSelectFile={onSelectFile}
            placeholder="选择 oauth.json 或 oauth_creds.json..."
            hint="默认路径: ~/.claude/oauth.json 或 Claude CLI 的凭证文件"
          />
        )}
      </>
    ),
  };
}
