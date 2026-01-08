/**
 * @file TerminalAIPanel.tsx
 * @description Terminal AI 面板主组件
 * @module components/terminal/ai/TerminalAIPanel
 *
 * 参考 Waveterm 的 AIPanel 设计，实现终端 AI 助手面板
 */

import React, { useState, useCallback } from "react";
import { Sparkles, MoreVertical, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { Switch } from "@/components/ui/switch";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { TerminalAIModeSelector } from "./TerminalAIModeSelector";
import { TerminalAIMessages } from "./TerminalAIMessages";
import { TerminalAIInput } from "./TerminalAIInput";
import { TerminalAIWelcome } from "./TerminalAIWelcome";
import { useTerminalAI } from "./useTerminalAI";

// ============================================================================
// 类型
// ============================================================================

interface TerminalAIPanelProps {
  /** 获取终端输出的回调 */
  getTerminalOutput?: () => string | null;
  /** 自定义类名 */
  className?: string;
}

// ============================================================================
// 组件
// ============================================================================

export const TerminalAIPanel: React.FC<TerminalAIPanelProps> = ({
  getTerminalOutput,
  className,
}) => {
  const [input, setInput] = useState("");

  const {
    messages,
    isSending,
    config,
    providerId,
    setProviderId,
    modelId,
    setModelId,
    sendMessage,
    clearMessages,
    toggleWidgetContext,
  } = useTerminalAI(getTerminalOutput);

  /**
   * 处理发送
   */
  const handleSend = useCallback(async () => {
    if (!input.trim()) return;
    const text = input;
    setInput("");
    await sendMessage(text);
  }, [input, sendMessage]);

  /**
   * 处理快捷输入
   */
  const handleQuickInput = useCallback((text: string) => {
    setInput(text);
  }, []);

  const hasMessages = messages.length > 0;

  return (
    <div
      className={cn(
        "flex flex-col h-full bg-zinc-900 border-r border-zinc-700",
        className,
      )}
    >
      {/* 头部 */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-700">
        <div className="flex items-center gap-2">
          <Sparkles size={16} className="text-yellow-400" />
          <span className="font-medium text-zinc-200">Terminal AI</span>
        </div>

        {/* 更多菜单 */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="p-1 rounded hover:bg-zinc-700 text-zinc-400 hover:text-zinc-200 transition-colors">
              <MoreVertical size={16} />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            className="bg-zinc-800 border-zinc-700"
          >
            <DropdownMenuItem
              onClick={clearMessages}
              className="text-zinc-200 hover:bg-zinc-700 cursor-pointer"
            >
              <Trash2 size={14} className="mr-2" />
              清空对话
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {/* Widget Context 开关 */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-700/50">
        <span className="text-sm text-zinc-400">Widget Context</span>
        <Switch
          checked={config.widgetContext}
          onCheckedChange={toggleWidgetContext}
          className="data-[state=checked]:bg-green-500"
        />
      </div>

      {/* 模式选择器 */}
      <div className="px-3 py-2 border-b border-zinc-700/50">
        <TerminalAIModeSelector
          providerId={providerId}
          onProviderChange={setProviderId}
          modelId={modelId}
          onModelChange={setModelId}
        />
      </div>

      {/* 消息区域 */}
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {hasMessages ? (
          <TerminalAIMessages messages={messages} isSending={isSending} />
        ) : (
          <TerminalAIWelcome onQuickInput={handleQuickInput} />
        )}
      </div>

      {/* 输入区域 */}
      <TerminalAIInput
        value={input}
        onChange={setInput}
        onSubmit={handleSend}
        isSending={isSending}
        placeholder={
          hasMessages ? "Continue..." : "Ask Terminal AI anything..."
        }
      />
    </div>
  );
};
