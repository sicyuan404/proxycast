/**
 * Qwen 凭证添加表单
 * 支持 Device Code Flow 登录和文件导入两种模式
 */

import { useState, useEffect } from "react";
import { safeListen } from "@/lib/dev-bridge";
import { open } from "@tauri-apps/plugin-shell";
import { providerPoolApi } from "@/lib/api/providerPool";
import { ModeSelector } from "./ModeSelector";
import { FileImportForm } from "./FileImportForm";
import { Copy, Check, ExternalLink } from "lucide-react";

interface QwenFormProps {
  name: string;
  credsFilePath: string;
  setCredsFilePath: (path: string) => void;
  onSelectFile: () => void;
  loading: boolean;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  onSuccess: () => void;
}

interface DeviceCodeInfo {
  user_code: string;
  verification_uri: string;
  verification_uri_complete?: string;
  expires_in: number;
}

export function QwenForm({
  name,
  credsFilePath,
  setCredsFilePath,
  onSelectFile,
  loading: _loading,
  setLoading,
  setError,
  onSuccess,
}: QwenFormProps) {
  const [mode, setMode] = useState<"login" | "file">("login");
  const [deviceCode, setDeviceCode] = useState<DeviceCodeInfo | null>(null);
  const [waitingForAuth, setWaitingForAuth] = useState(false);
  const [copied, setCopied] = useState(false);

  // 监听后端发送的设备码事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await safeListen<DeviceCodeInfo>(
        "qwen-device-code",
        (event) => {
          setDeviceCode(event.payload);
        },
      );
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 复制用户码
  const handleCopyCode = async () => {
    if (deviceCode?.user_code) {
      await navigator.clipboard.writeText(deviceCode.user_code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  // 打开验证链接
  const handleOpenVerificationUrl = async () => {
    if (deviceCode) {
      const url =
        deviceCode.verification_uri_complete || deviceCode.verification_uri;
      try {
        await open(url);
      } catch (e) {
        console.error("Failed to open URL:", e);
        // 如果 Tauri shell 失败，尝试使用 window.open 作为后备
        window.open(url, "_blank");
      }
    }
  };

  // 获取设备码并启动轮询
  const handleGetDeviceCode = async () => {
    setLoading(true);
    setError(null);
    setDeviceCode(null);
    setWaitingForAuth(true);

    try {
      const trimmedName = name.trim() || undefined;
      await providerPoolApi.getQwenDeviceCodeAndWait(trimmedName);
      onSuccess();
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      setWaitingForAuth(false);
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
      await providerPoolApi.addQwenOAuth(credsFilePath, trimmedName);
      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return {
    mode,
    deviceCode,
    waitingForAuth,
    handleGetDeviceCode,
    handleFileSubmit,
    render: () => (
      <>
        <ModeSelector
          mode={mode}
          setMode={setMode}
          loginLabel="Qwen 登录"
          fileLabel="导入文件"
        />

        {mode === "login" ? (
          <div className="space-y-4">
            <div className="rounded-lg border border-blue-200 bg-blue-50 p-4 dark:border-blue-800 dark:bg-blue-950/30">
              <p className="text-sm text-blue-700 dark:text-blue-300">
                点击下方按钮获取设备码，然后在浏览器中完成 Qwen 登录授权。
              </p>
              <p className="mt-2 text-xs text-blue-600 dark:text-blue-400">
                授权成功后，凭证将自动保存并添加到凭证池。
              </p>
            </div>

            {deviceCode && (
              <div className="space-y-3">
                {/* 用户码显示 */}
                <div className="rounded-lg border border-blue-300 bg-blue-100 p-4 dark:border-blue-700 dark:bg-blue-900/50">
                  <p className="mb-2 text-sm font-medium text-blue-800 dark:text-blue-200">
                    请在浏览器中输入以下验证码：
                  </p>
                  <div className="flex items-center justify-between">
                    <code className="text-2xl font-bold tracking-wider text-blue-900 dark:text-blue-100">
                      {deviceCode.user_code}
                    </code>
                    <button
                      onClick={handleCopyCode}
                      className="flex items-center gap-1 rounded-lg border border-blue-300 bg-white px-3 py-1.5 text-sm text-blue-700 hover:bg-blue-50 dark:border-blue-600 dark:bg-blue-800 dark:text-blue-200 dark:hover:bg-blue-700"
                    >
                      {copied ? (
                        <>
                          <Check className="h-4 w-4" />
                          已复制
                        </>
                      ) : (
                        <>
                          <Copy className="h-4 w-4" />
                          复制
                        </>
                      )}
                    </button>
                  </div>
                </div>

                {/* 验证链接 */}
                <button
                  onClick={handleOpenVerificationUrl}
                  className="flex w-full items-center justify-center gap-2 rounded-lg border border-blue-300 bg-white px-4 py-2 text-sm text-blue-700 hover:bg-blue-50 dark:border-blue-600 dark:bg-blue-800 dark:text-blue-200 dark:hover:bg-blue-700"
                >
                  <ExternalLink className="h-4 w-4" />
                  打开验证页面
                </button>

                {waitingForAuth && (
                  <div className="flex items-center justify-center gap-2 text-sm text-blue-600 dark:text-blue-400">
                    <div className="h-4 w-4 animate-spin rounded-full border-2 border-blue-600 border-t-transparent" />
                    等待授权中...
                  </div>
                )}
              </div>
            )}
          </div>
        ) : (
          <FileImportForm
            credsFilePath={credsFilePath}
            setCredsFilePath={setCredsFilePath}
            onSelectFile={onSelectFile}
            placeholder="选择 oauth_creds.json..."
            hint="默认路径: ~/.qwen/oauth_creds.json"
          />
        )}
      </>
    ),
  };
}
