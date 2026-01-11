/**
 * 插件安装对话框组件
 *
 * 支持从本地文件或 URL 安装插件，显示安装进度
 * _需求: 1.1, 2.1, 3.1, 3.2, 3.3, 3.4_
 */

import { useState, useEffect, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import { safeListen } from "@/lib/dev-bridge";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FolderOpen,
  Link,
  Loader2,
  CheckCircle,
  XCircle,
  Download,
  FileArchive,
} from "lucide-react";
import { Modal, ModalHeader, ModalBody, ModalFooter } from "@/components/Modal";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";

/** 安装阶段 */
type InstallStage =
  | "downloading"
  | "validating"
  | "extracting"
  | "installing"
  | "registering"
  | "complete"
  | "failed";

/** 安装进度事件 */
interface InstallProgress {
  stage: InstallStage;
  percent: number;
  message: string;
}

/** 安装来源 */
interface InstallSource {
  type: "local" | "url" | "github";
  path?: string;
  url?: string;
  owner?: string;
  repo?: string;
  tag?: string;
}

/** 已安装插件信息 */
interface InstalledPlugin {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string | null;
  install_path: string;
  installed_at: string;
  source: InstallSource;
  enabled: boolean;
}

/** 安装结果 */
interface InstallResult {
  success: boolean;
  plugin: InstalledPlugin | null;
  error: string | null;
}

interface PluginInstallDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  /** 初始 URL，用于一键安装 */
  initialUrl?: string;
}

/** 获取阶段显示文本 */
function getStageText(stage: InstallStage): string {
  switch (stage) {
    case "downloading":
      return "下载中";
    case "validating":
      return "验证中";
    case "extracting":
      return "解压中";
    case "installing":
      return "安装中";
    case "registering":
      return "注册中";
    case "complete":
      return "完成";
    case "failed":
      return "失败";
    default:
      return stage;
  }
}

export function PluginInstallDialog({
  isOpen,
  onClose,
  onSuccess,
  initialUrl,
}: PluginInstallDialogProps) {
  const [activeTab, setActiveTab] = useState<"file" | "url">("file");
  const [filePath, setFilePath] = useState("");
  const [url, setUrl] = useState("");
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<InstalledPlugin | null>(null);
  const [autoInstallTriggered, setAutoInstallTriggered] = useState(false);

  // 监听安装进度事件
  useEffect(() => {
    if (!isOpen) return;

    const unlisten = safeListen<InstallProgress>(
      "plugin-install-progress",
      (event) => {
        setProgress(event.payload);
      },
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [isOpen]);

  // 处理 initialUrl - 自动切换到 URL tab 并填充
  useEffect(() => {
    if (isOpen && initialUrl && !autoInstallTriggered) {
      setActiveTab("url");
      setUrl(initialUrl);
      setAutoInstallTriggered(true);
    }
  }, [isOpen, initialUrl, autoInstallTriggered]);

  // 从 URL 安装
  const handleInstallFromUrl = useCallback(async () => {
    const currentUrl = url;
    if (!currentUrl.trim()) {
      setError("请输入插件包 URL");
      return;
    }

    if (
      !currentUrl.startsWith("http://") &&
      !currentUrl.startsWith("https://")
    ) {
      setError("URL 必须以 http:// 或 https:// 开头");
      return;
    }

    setInstalling(true);
    setError(null);
    setProgress(null);
    setResult(null);

    try {
      const installResult = await safeInvoke<InstallResult>(
        "install_plugin_from_url",
        { url: currentUrl },
      );

      if (installResult.success && installResult.plugin) {
        // 安装成功，关闭对话框并通知父组件
        onSuccess();
        resetState();
        onClose();
      } else {
        setError(installResult.error || "安装失败");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setInstalling(false);
    }
  }, [url, onSuccess, onClose]);

  // 自动开始安装（当有 initialUrl 时）
  useEffect(() => {
    if (
      isOpen &&
      initialUrl &&
      autoInstallTriggered &&
      url === initialUrl &&
      !installing &&
      !result &&
      !error
    ) {
      // 延迟一点开始安装，让用户看到 UI
      const timer = setTimeout(() => {
        handleInstallFromUrl();
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [
    isOpen,
    initialUrl,
    autoInstallTriggered,
    url,
    installing,
    result,
    error,
    handleInstallFromUrl,
  ]);

  // 重置状态
  const resetState = () => {
    setFilePath("");
    setUrl("");
    setInstalling(false);
    setProgress(null);
    setError(null);
    setResult(null);
    setAutoInstallTriggered(false);
  };

  // 关闭对话框
  const handleClose = () => {
    if (installing) return; // 安装中不允许关闭
    resetState();
    onClose();
  };

  // 选择本地文件
  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "插件包", extensions: ["zip", "tar.gz", "tgz"] }],
      });
      if (selected) {
        setFilePath(selected as string);
        setError(null);
      }
    } catch (e) {
      console.error("选择文件失败:", e);
    }
  };

  // 从本地文件安装
  const handleInstallFromFile = async () => {
    if (!filePath.trim()) {
      setError("请选择插件包文件");
      return;
    }

    setInstalling(true);
    setError(null);
    setProgress(null);
    setResult(null);

    try {
      const installResult = await safeInvoke<InstallResult>(
        "install_plugin_from_file",
        { filePath },
      );

      if (installResult.success && installResult.plugin) {
        // 安装成功，关闭对话框并通知父组件
        onSuccess();
        resetState();
        onClose();
      } else {
        setError(installResult.error || "安装失败");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setInstalling(false);
    }
  };

  // 渲染进度显示
  const renderProgress = () => {
    if (!progress) return null;

    const isComplete = progress.stage === "complete";
    const isFailed = progress.stage === "failed";

    return (
      <div className="space-y-3">
        <div className="flex items-center justify-between text-sm">
          <span className="flex items-center gap-2">
            {isComplete ? (
              <CheckCircle className="h-4 w-4 text-green-500" />
            ) : isFailed ? (
              <XCircle className="h-4 w-4 text-red-500" />
            ) : (
              <Loader2 className="h-4 w-4 animate-spin" />
            )}
            {getStageText(progress.stage)}
          </span>
          <span>{progress.percent}%</span>
        </div>
        <Progress value={progress.percent} />
        <p className="text-xs text-muted-foreground">{progress.message}</p>
      </div>
    );
  };

  // 渲染安装结果
  const renderResult = () => {
    if (!result) return null;

    return (
      <div className="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-950/30">
        <div className="flex items-start gap-3">
          <CheckCircle className="h-5 w-5 text-green-500 mt-0.5" />
          <div className="flex-1">
            <h4 className="font-medium text-green-700 dark:text-green-300">
              安装成功
            </h4>
            <div className="mt-2 space-y-1 text-sm text-green-600 dark:text-green-400">
              <p>
                <span className="font-medium">插件名称：</span>
                {result.name}
              </p>
              <p>
                <span className="font-medium">版本：</span>
                {result.version}
              </p>
              {result.description && (
                <p>
                  <span className="font-medium">描述：</span>
                  {result.description}
                </p>
              )}
            </div>
          </div>
        </div>
      </div>
    );
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleClose}
      maxWidth="max-w-md"
      closeOnOverlayClick={!installing}
    >
      <ModalHeader>安装插件</ModalHeader>

      <ModalBody>
        {/* 安装结果显示 */}
        {result ? (
          renderResult()
        ) : (
          <>
            {/* 安装方式选择 */}
            <Tabs
              value={activeTab}
              onValueChange={(v) => setActiveTab(v as "file" | "url")}
            >
              <TabsList className="w-full">
                <TabsTrigger value="file" className="flex-1">
                  <FileArchive className="h-4 w-4 mr-2" />
                  本地文件
                </TabsTrigger>
                <TabsTrigger value="url" className="flex-1">
                  <Link className="h-4 w-4 mr-2" />
                  URL 下载
                </TabsTrigger>
              </TabsList>

              {/* 本地文件安装 */}
              <TabsContent value="file" className="space-y-4">
                <div>
                  <label className="mb-2 block text-sm font-medium">
                    选择插件包文件
                  </label>
                  <div className="flex gap-2">
                    <Input
                      value={filePath}
                      onChange={(e) => setFilePath(e.target.value)}
                      placeholder="选择 .zip 或 .tar.gz 文件..."
                      disabled={installing}
                      className="flex-1"
                    />
                    <Button
                      variant="outline"
                      onClick={handleSelectFile}
                      disabled={installing}
                    >
                      <FolderOpen className="h-4 w-4" />
                    </Button>
                  </div>
                  <p className="mt-1 text-xs text-muted-foreground">
                    支持 .zip 和 .tar.gz 格式的插件包
                  </p>
                </div>
              </TabsContent>

              {/* URL 安装 */}
              <TabsContent value="url" className="space-y-4">
                <div>
                  <label className="mb-2 block text-sm font-medium">
                    插件包 URL
                  </label>
                  <Input
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    placeholder="https://github.com/.../releases/download/..."
                    disabled={installing}
                  />
                  <p className="mt-1 text-xs text-muted-foreground">
                    支持 GitHub Releases 或其他直接下载链接
                  </p>
                </div>
              </TabsContent>
            </Tabs>

            {/* 进度显示 */}
            {installing && progress && (
              <div className="mt-4">{renderProgress()}</div>
            )}

            {/* 错误显示 */}
            {error && (
              <div className="mt-4 rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-800 dark:bg-red-950/30 dark:text-red-400">
                <div className="flex items-start gap-2">
                  <XCircle className="h-4 w-4 mt-0.5 flex-shrink-0" />
                  <span>{error}</span>
                </div>
              </div>
            )}
          </>
        )}
      </ModalBody>

      <ModalFooter>
        {result ? (
          <Button onClick={handleClose}>完成</Button>
        ) : (
          <>
            <Button
              variant="outline"
              onClick={handleClose}
              disabled={installing}
            >
              取消
            </Button>
            <Button
              onClick={
                activeTab === "file"
                  ? handleInstallFromFile
                  : handleInstallFromUrl
              }
              disabled={
                installing ||
                (activeTab === "file" ? !filePath.trim() : !url.trim())
              }
            >
              {installing ? (
                <>
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  安装中...
                </>
              ) : (
                <>
                  <Download className="h-4 w-4 mr-2" />
                  安装
                </>
              )}
            </Button>
          </>
        )}
      </ModalFooter>
    </Modal>
  );
}
