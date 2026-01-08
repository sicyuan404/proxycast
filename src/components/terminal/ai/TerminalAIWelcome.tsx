/**
 * @file TerminalAIWelcome.tsx
 * @description Terminal AI 欢迎页面
 * @module components/terminal/ai/TerminalAIWelcome
 *
 * 参考 Waveterm 的 AIWelcomeMessage 设计
 */

import React, { memo } from "react";
import { Sparkles, Terminal, FileText, Bug, Lightbulb } from "lucide-react";
import { cn } from "@/lib/utils";

// ============================================================================
// 类型
// ============================================================================

interface QuickAction {
  icon: React.ReactNode;
  label: string;
  prompt: string;
}

interface TerminalAIWelcomeProps {
  /** 快捷输入回调 */
  onQuickInput?: (text: string) => void;
}

// ============================================================================
// 常量
// ============================================================================

const QUICK_ACTIONS: QuickAction[] = [
  {
    icon: <Terminal size={14} />,
    label: "解释命令",
    prompt: "请解释这个命令的作用：",
  },
  {
    icon: <Bug size={14} />,
    label: "调试错误",
    prompt: "帮我分析这个错误信息：",
  },
  {
    icon: <FileText size={14} />,
    label: "写脚本",
    prompt: "帮我写一个 shell 脚本：",
  },
  {
    icon: <Lightbulb size={14} />,
    label: "优化命令",
    prompt: "帮我优化这个命令：",
  },
];

// ============================================================================
// 组件
// ============================================================================

/**
 * 快捷操作按钮
 */
const QuickActionButton = memo(
  ({ action, onClick }: { action: QuickAction; onClick: () => void }) => (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-2 px-3 py-2 rounded-lg",
        "bg-zinc-800/50 hover:bg-zinc-700/50 border border-zinc-700/50",
        "text-sm text-zinc-300 hover:text-zinc-100 transition-colors",
        "text-left",
      )}
    >
      <span className="text-zinc-400">{action.icon}</span>
      <span>{action.label}</span>
    </button>
  ),
);

QuickActionButton.displayName = "QuickActionButton";

/**
 * Terminal AI 欢迎页面
 */
export const TerminalAIWelcome: React.FC<TerminalAIWelcomeProps> = ({
  onQuickInput,
}) => {
  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6 text-center">
      {/* 图标和标题 */}
      <div className="mb-6">
        <Sparkles size={40} className="text-yellow-400 mx-auto mb-3" />
        <h2 className="text-lg font-semibold text-zinc-100">
          欢迎使用 Terminal AI
        </h2>
      </div>

      {/* 描述 */}
      <p className="text-sm text-zinc-400 max-w-[280px] mb-6">
        我是你的终端助手，可以帮你解释命令、调试错误、编写脚本。 开启 Widget
        Context 后，我可以看到你的终端输出。
      </p>

      {/* 快捷操作 */}
      <div className="w-full max-w-[280px]">
        <p className="text-xs text-zinc-500 mb-3">快捷操作</p>
        <div className="grid grid-cols-2 gap-2">
          {QUICK_ACTIONS.map((action, index) => (
            <QuickActionButton
              key={index}
              action={action}
              onClick={() => onQuickInput?.(action.prompt)}
            />
          ))}
        </div>
      </div>

      {/* 提示 */}
      <div className="mt-6 text-xs text-zinc-500">
        <p>按 Enter 发送消息，Shift+Enter 换行</p>
      </div>
    </div>
  );
};
