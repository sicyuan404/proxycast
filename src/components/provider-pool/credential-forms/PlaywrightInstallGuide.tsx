/**
 * Playwright 安装引导组件
 *
 * 当 Playwright 未安装时显示安装指南
 * 提供一键安装、复制命令和重新检测功能
 *
 * @module components/provider-pool/credential-forms/PlaywrightInstallGuide
 * @description 实现 Requirements 1.4, 6.1, 6.2, 6.3, 6.4
 */

import { useState, useEffect } from "react";
import { Copy, Check, RefreshCw, Download, Loader2 } from "lucide-react";
import { safeListen } from "@/lib/dev-bridge";
import { installPlaywright } from "@/lib/api/providerPool";

interface PlaywrightInstallGuideProps {
  /** 重新检测回调 */
  onRetryCheck: () => void;
  /** 是否正在检测 */
  checking: boolean;
}

interface InstallProgress {
  message: string;
  done: boolean;
  success?: boolean;
}

const INSTALL_COMMAND = "npx playwright install chromium";

/**
 * Playwright 安装引导
 *
 * 紧凑的内联显示安装命令和操作按钮
 * 支持一键安装功能
 */
export function PlaywrightInstallGuide({
  onRetryCheck,
  checking,
}: PlaywrightInstallGuideProps) {
  const [copied, setCopied] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  // 监听安装进度事件
  useEffect(() => {
    const unlisten = safeListen<InstallProgress>(
      "playwright-install-progress",
      (event) => {
        setProgress(event.payload.message);
        if (event.payload.done) {
          setInstalling(false);
          if (event.payload.success) {
            // 安装成功，触发重新检测
            setError(null);
            onRetryCheck();
          } else {
            // 安装失败，显示错误
            setError(event.payload.message);
          }
        }
      },
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [onRetryCheck]);

  const handleCopyCommand = async () => {
    try {
      await navigator.clipboard.writeText(INSTALL_COMMAND);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("复制失败:", err);
    }
  };

  const handleInstall = async () => {
    setInstalling(true);
    setProgress("正在准备安装...");
    setError(null);
    try {
      await installPlaywright();
    } catch (err) {
      console.error("安装失败:", err);
      const errorMsg =
        typeof err === "string" ? err : (err as Error)?.message || String(err);
      setError(errorMsg);
      setProgress("");
      setInstalling(false);
    }
  };

  // 显示错误状态
  if (error) {
    return (
      <div className="rounded border border-red-300 dark:border-red-700 bg-red-50 dark:bg-red-950/30 px-2 py-1.5">
        <div className="flex flex-col gap-1">
          <div className="flex items-center gap-2 text-xs">
            <span
              className="text-red-700 dark:text-red-400 flex-1 truncate"
              title={error}
            >
              安装失败:{" "}
              {error.length > 50 ? error.substring(0, 50) + "..." : error}
            </span>
            <button
              type="button"
              onClick={handleInstall}
              className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-red-200 dark:bg-red-800 hover:bg-red-300 dark:hover:bg-red-700 transition-colors text-red-800 dark:text-red-200 whitespace-nowrap"
              title="重试安装"
            >
              <RefreshCw className="h-3 w-3" />
              <span>重试</span>
            </button>
          </div>
        </div>
      </div>
    );
  }

  // 安装中显示进度
  if (installing) {
    return (
      <div className="rounded border border-blue-300 dark:border-blue-700 bg-blue-50 dark:bg-blue-950/30 px-2 py-1.5">
        <div className="flex items-center gap-2 text-xs">
          <Loader2 className="h-3 w-3 text-blue-600 dark:text-blue-400 animate-spin" />
          <span className="flex-1 text-blue-700 dark:text-blue-300 truncate">
            {progress || "正在安装..."}
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="rounded border border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-950/30 px-2 py-1.5">
      <div className="flex items-center gap-2 text-xs">
        <button
          type="button"
          onClick={handleInstall}
          className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-amber-200 dark:bg-amber-800 hover:bg-amber-300 dark:hover:bg-amber-700 transition-colors text-amber-800 dark:text-amber-200"
          title="一键安装"
        >
          <Download className="h-3 w-3" />
          <span>安装</span>
        </button>
        <span className="text-amber-600 dark:text-amber-500">或</span>
        <code className="flex-1 font-mono text-amber-800 dark:text-amber-300 truncate select-all text-[10px]">
          {INSTALL_COMMAND}
        </code>
        <button
          type="button"
          onClick={handleCopyCommand}
          className="p-0.5 rounded hover:bg-amber-200 dark:hover:bg-amber-800 transition-colors"
          title="复制"
        >
          {copied ? (
            <Check className="h-3 w-3 text-green-600" />
          ) : (
            <Copy className="h-3 w-3 text-amber-600 dark:text-amber-400" />
          )}
        </button>
        <button
          type="button"
          onClick={onRetryCheck}
          disabled={checking}
          className="p-0.5 rounded hover:bg-amber-200 dark:hover:bg-amber-800 transition-colors disabled:opacity-50"
          title="重新检测"
        >
          <RefreshCw
            className={`h-3 w-3 text-amber-600 dark:text-amber-400 ${checking ? "animate-spin" : ""}`}
          />
        </button>
      </div>
    </div>
  );
}
