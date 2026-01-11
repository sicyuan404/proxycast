/**
 * @file update-notification.tsx
 * @description 更新提醒独立窗口页面
 *
 * 独立于主应用的更新提醒悬浮窗口，显示版本信息和更新操作。
 * 参考 screenshot-chat.tsx 的实现模式。
 *
 * input: URL 参数（current, latest, download_url）
 * output: 更新提醒 UI
 * pos: pages 层，独立 Tauri 窗口
 */

import { useEffect, useState, useCallback, type MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { safeInvoke } from "@/lib/dev-bridge";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { Download, X, Clock, SkipForward, ExternalLink } from "lucide-react";
import "./update-notification.css";

interface UpdateParams {
  currentVersion: string;
  latestVersion: string;
  downloadUrl: string;
}

function getUpdateParamsFromUrl(): UpdateParams {
  const params = new URLSearchParams(window.location.search);
  return {
    currentVersion: params.get("current") || "",
    latestVersion: params.get("latest") || "",
    downloadUrl: params.get("download_url") || "",
  };
}

export function UpdateNotificationPage() {
  const [params, setParams] = useState<UpdateParams>({
    currentVersion: "",
    latestVersion: "",
    downloadUrl: "",
  });
  const [downloading, setDownloading] = useState(false);

  useEffect(() => {
    setParams(getUpdateParamsFromUrl());
  }, []);

  // 关闭窗口
  const handleClose = useCallback(async () => {
    try {
      await safeInvoke("close_update_window");
    } catch (err) {
      console.error("关闭窗口失败:", err);
      // 备用方案：直接关闭
      await getCurrentWindow().close();
    }
  }, []);

  // ESC 关闭窗口
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        await handleClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleClose]);

  // 开始拖动窗口
  const handleStartDrag = useCallback(async (e: MouseEvent) => {
    if (e.button !== 0) return;
    try {
      await getCurrentWindow().startDragging();
    } catch (err) {
      console.error("拖动窗口失败:", err);
    }
  }, []);

  // 立即更新
  const handleDownload = async () => {
    setDownloading(true);
    try {
      await safeInvoke("download_update");
      // download_update 成功后会自动关闭窗口并启动安装程序
    } catch (error) {
      console.error("下载更新失败:", error);
      // 如果下载失败，尝试打开浏览器
      if (params.downloadUrl) {
        try {
          await shellOpen(params.downloadUrl);
          await handleClose();
        } catch {
          window.open(params.downloadUrl, "_blank");
        }
      }
    } finally {
      setDownloading(false);
    }
  };

  // 稍后提醒
  const handleLater = async () => {
    await handleClose();
  };

  // 跳过此版本
  const handleSkipVersion = async () => {
    if (params.latestVersion) {
      try {
        await safeInvoke("skip_update_version", {
          version: params.latestVersion,
        });
        await handleClose();
      } catch (error) {
        console.error("跳过版本失败:", error);
      }
    }
  };

  // 在浏览器中打开
  const handleOpenInBrowser = async () => {
    if (params.downloadUrl) {
      try {
        await shellOpen(params.downloadUrl);
      } catch (error) {
        console.error("打开浏览器失败:", error);
        // 备用方案
        window.open(params.downloadUrl, "_blank");
      }
    }
  };

  return (
    <div className="update-container">
      <div className="update-card">
        {/* 头部 - 可拖动区域 */}
        <div className="update-header" onMouseDown={handleStartDrag}>
          <div className="update-title">
            <img src="/logo.png" alt="ProxyCast" className="update-logo" />
            <span>发现新版本</span>
          </div>
          <button
            onClick={handleClose}
            className="update-close-btn"
            title="关闭 (ESC)"
            onMouseDown={(e) => e.stopPropagation()}
          >
            <X size={16} />
          </button>
        </div>

        {/* 内容 */}
        <div className="update-content">
          <div className="update-version-row">
            <span className="update-label">当前版本</span>
            <span className="update-value">{params.currentVersion}</span>
          </div>
          <div className="update-version-row">
            <span className="update-label">最新版本</span>
            <span className="update-value update-new">
              {params.latestVersion}
            </span>
          </div>

          {/* 操作按钮 */}
          <div className="update-actions">
            <button
              onClick={handleDownload}
              disabled={downloading}
              className="update-btn update-btn-primary"
            >
              <Download
                size={16}
                className={downloading ? "animate-bounce" : ""}
              />
              {downloading ? "下载中..." : "立即更新"}
            </button>

            <div className="update-btn-row">
              <button
                onClick={handleLater}
                className="update-btn update-btn-secondary"
              >
                <Clock size={14} />
                稍后提醒
              </button>
              <button
                onClick={handleSkipVersion}
                className="update-btn update-btn-secondary"
              >
                <SkipForward size={14} />
                跳过此版本
              </button>
            </div>

            {params.downloadUrl && (
              <button onClick={handleOpenInBrowser} className="update-link">
                <ExternalLink size={12} />
                在浏览器中查看
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default UpdateNotificationPage;
