/**
 * @file screenshot-chat.tsx
 * @description 截图对话悬浮窗口 - 参考 Google Gemini 浮动栏设计
 *              半透明药丸形状，简洁的输入界面
 * @module pages/screenshot-chat
 */

import React, { useEffect, useState, useRef, useCallback } from "react";
import { Image as ImageIcon, ArrowUp, X, GripVertical } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./screenshot-chat.css";

// ProxyCast Logo组件
function Logo() {
  return (
    <svg
      viewBox="0 0 128 128"
      width="20"
      height="20"
      className="screenshot-logo"
    >
      <defs>
        <linearGradient id="leftP" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" style={{ stopColor: "#4fc3f7" }} />
          <stop offset="100%" style={{ stopColor: "#1a237e" }} />
        </linearGradient>
        <linearGradient id="rightP" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" style={{ stopColor: "#7c4dff" }} />
          <stop offset="100%" style={{ stopColor: "#e91e63" }} />
        </linearGradient>
      </defs>
      <g>
        <rect x="36" y="32" width="10" height="64" rx="3" fill="url(#leftP)" />
        <rect x="46" y="32" width="28" height="9" rx="3" fill="url(#rightP)" />
        <rect x="46" y="60" width="24" height="8" rx="2" fill="url(#rightP)" />
        <rect x="70" y="41" width="8" height="27" rx="3" fill="url(#rightP)" />
      </g>
    </svg>
  );
}

function getImagePathFromUrl(): string | null {
  const params = new URLSearchParams(window.location.search);
  const imagePath = params.get("image");
  return imagePath ? decodeURIComponent(imagePath) : null;
}

export function ScreenshotChatPage() {
  const [imagePath, setImagePath] = useState<string | null>(null);
  const [inputValue, setInputValue] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // 从 URL 获取图片路径
  useEffect(() => {
    const path = getImagePathFromUrl();
    if (path) {
      setImagePath(path);
    }
  }, []);

  // 自动聚焦
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // 关闭窗口
  const handleClose = useCallback(async () => {
    try {
      await getCurrentWindow().close();
    } catch (err) {
      console.error("关闭窗口失败:", err);
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
  const handleStartDrag = useCallback(async (e: React.MouseEvent) => {
    // 只响应左键
    if (e.button !== 0) return;
    try {
      await getCurrentWindow().startDragging();
    } catch (err) {
      console.error("拖动窗口失败:", err);
    }
  }, []);

  // 移除图片附件
  const handleRemoveImage = () => {
    setImagePath(null);
  };

  // 发送到主应用
  const handleSend = async () => {
    if (!inputValue.trim() || isLoading) return;
    setIsLoading(true);

    try {
      const { safeInvoke } = await import("@/lib/dev-bridge");
      await safeInvoke("send_screenshot_chat", {
        message: inputValue,
        imagePath: imagePath,
      });

      await getCurrentWindow().close();
    } catch (err) {
      console.error("发送失败:", err);
      setIsLoading(false);
    }
  };

  const handleInputKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="screenshot-container">
      <div className="screenshot-input-bar">
        {/* 拖动手柄 */}
        <div
          className="screenshot-drag-handle"
          onMouseDown={handleStartDrag}
          title="拖动移动窗口"
        >
          <GripVertical size={14} />
        </div>

        {/* Logo */}
        <Logo />

        {/* 图片附件标签 */}
        {imagePath && (
          <div className="screenshot-attachment">
            <ImageIcon size={12} />
            <span>Image</span>
            <button
              className="screenshot-attachment-remove"
              onClick={handleRemoveImage}
              title="移除图片"
            >
              <X size={10} />
            </button>
          </div>
        )}

        {/* 输入框 */}
        <input
          ref={inputRef}
          type="text"
          className="screenshot-input"
          placeholder="Ask anything..."
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleInputKeyDown}
          disabled={isLoading}
        />

        {/* 右侧按钮组 */}
        <div className="screenshot-actions">
          {/* 关闭按钮 */}
          <button
            className="screenshot-close-btn"
            onClick={handleClose}
            title="关闭 (ESC)"
          >
            <X size={14} />
          </button>

          {/* 发送按钮 */}
          <button
            className={`screenshot-send-btn ${inputValue.trim() ? "active" : ""}`}
            onClick={handleSend}
            disabled={!inputValue.trim() || isLoading}
            title="发送 (Enter)"
          >
            <ArrowUp size={16} />
          </button>
        </div>
      </div>
    </div>
  );
}

export default ScreenshotChatPage;
