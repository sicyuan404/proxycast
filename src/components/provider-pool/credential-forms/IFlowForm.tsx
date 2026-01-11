/**
 * iFlow 凭证添加表单
 * 支持 OAuth 登录和文件导入两种模式
 */

import { useState, useEffect } from "react";
import { safeListen } from "@/lib/dev-bridge";
import { providerPoolApi } from "@/lib/api/providerPool";
import { ModeSelector } from "./ModeSelector";
import { FileImportForm } from "./FileImportForm";
import { OAuthUrlDisplay } from "./OAuthUrlDisplay";

interface IFlowFormProps {
  name: string;
  credsFilePath: string;
  setCredsFilePath: (path: string) => void;
  onSelectFile: () => void;
  loading: boolean;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  onSuccess: () => void;
}

export function IFlowForm({
  name,
  credsFilePath,
  setCredsFilePath,
  onSelectFile,
  loading: _loading,
  setLoading,
  setError,
  onSuccess,
}: IFlowFormProps) {
  const [mode, setMode] = useState<"login" | "file">("login");
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [waitingForCallback, setWaitingForCallback] = useState(false);

  // 监听后端发送的授权 URL 事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await safeListen<{ auth_url: string }>(
        "iflow-auth-url",
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
      await providerPoolApi.getIFlowAuthUrlAndWait(trimmedName);
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
      await providerPoolApi.addIFlowOAuth(credsFilePath, trimmedName);
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
          loginLabel="iFlow 登录"
          fileLabel="导入文件"
        />

        {mode === "login" ? (
          <div className="space-y-4">
            <div className="rounded-lg border border-purple-200 bg-purple-50 p-4 dark:border-purple-800 dark:bg-purple-950/30">
              <p className="text-sm text-purple-700 dark:text-purple-300">
                点击下方按钮获取授权 URL，然后复制到浏览器（支持指纹浏览器）完成
                iFlow 登录。
              </p>
              <p className="mt-2 text-xs text-purple-600 dark:text-purple-400">
                授权成功后，凭证将自动保存并添加到凭证池。
              </p>
            </div>

            <OAuthUrlDisplay
              authUrl={authUrl}
              waitingForCallback={waitingForCallback}
              colorScheme="purple"
            />
          </div>
        ) : (
          <FileImportForm
            credsFilePath={credsFilePath}
            setCredsFilePath={setCredsFilePath}
            onSelectFile={onSelectFile}
            placeholder="选择 auth.json 或 oauth_creds.json..."
            hint="默认路径: ~/.iflow/auth.json"
          />
        )}
      </>
    ),
  };
}
