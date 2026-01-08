/**
 * @file TerminalAIInput.tsx
 * @description Terminal AI 输入框组件
 * @module components/terminal/ai/TerminalAIInput
 *
 * 参考 Waveterm 的 AIPanelInput 设计
 */

import React, { useRef, useCallback, useEffect } from "react";
import { Send, Square, Paperclip } from "lucide-react";
import { cn } from "@/lib/utils";

interface TerminalAIInputProps {
  /** 输入值 */
  value: string;
  /** 输入变化回调 */
  onChange: (value: string) => void;
  /** 提交回调 */
  onSubmit: () => void;
  /** 停止回调 */
  onStop?: () => void;
  /** 是否正在发送 */
  isSending: boolean;
  /** 是否禁用 */
  disabled?: boolean;
  /** 占位符 */
  placeholder?: string;
}

export const TerminalAIInput: React.FC<TerminalAIInputProps> = ({
  value,
  onChange,
  onSubmit,
  onStop,
  isSending,
  disabled = false,
  placeholder = "Continue...",
}) => {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  /**
   * 自动调整高度
   */
  const resizeTextarea = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    textarea.style.height = "auto";
    const scrollHeight = textarea.scrollHeight;
    const maxHeight = 7 * 24; // 7 行
    textarea.style.height = `${Math.min(scrollHeight, maxHeight)}px`;
  }, []);

  useEffect(() => {
    resizeTextarea();
  }, [value, resizeTextarea]);

  /**
   * 处理键盘事件
   */
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const isComposing = e.nativeEvent?.isComposing || e.keyCode === 229;
    if (e.key === "Enter" && !e.shiftKey && !isComposing) {
      e.preventDefault();
      if (!isSending && value.trim()) {
        onSubmit();
      }
    }
  };

  /**
   * 处理提交
   */
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!isSending && value.trim()) {
      onSubmit();
    }
  };

  return (
    <div className="border-t border-zinc-700">
      <form onSubmit={handleSubmit}>
        <div className="relative">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            disabled={disabled}
            className={cn(
              "w-full text-white px-3 py-2 pr-16 focus:outline-none resize-none overflow-auto",
              "bg-zinc-800/50 text-sm",
              disabled && "opacity-50 cursor-not-allowed",
            )}
            rows={2}
          />

          {/* 附件按钮 */}
          <button
            type="button"
            className={cn(
              "absolute bottom-6 right-8 w-6 h-6 flex items-center justify-center",
              "text-zinc-400 hover:text-zinc-200 transition-colors",
            )}
            title="附加文件"
          >
            <Paperclip size={14} />
          </button>

          {/* 发送/停止按钮 */}
          {isSending ? (
            <button
              type="button"
              onClick={onStop}
              className={cn(
                "absolute bottom-1.5 right-2 w-6 h-6 flex items-center justify-center",
                "text-green-500 hover:text-green-400 transition-colors",
              )}
              title="停止响应"
            >
              <Square size={14} />
            </button>
          ) : (
            <button
              type="submit"
              disabled={disabled || !value.trim()}
              className={cn(
                "absolute bottom-1.5 right-2 w-6 h-6 flex items-center justify-center",
                "transition-colors",
                disabled || !value.trim()
                  ? "text-zinc-500 cursor-not-allowed"
                  : "text-blue-400 hover:text-blue-300",
              )}
              title="发送消息 (Enter)"
            >
              <Send size={14} />
            </button>
          )}
        </div>
      </form>
    </div>
  );
};
