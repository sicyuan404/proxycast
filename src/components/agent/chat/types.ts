import type { ToolCallState, TokenUsage } from "@/lib/api/agent";
import { safeInvoke } from "@/lib/dev-bridge";

export interface MessageImage {
  data: string;
  mediaType: string;
}

/**
 * 内容片段类型（用于交错显示）
 *
 * 参考 goose 框架的 MessageContent 设计：
 * - text: 文本内容片段
 * - tool_use: 工具调用（包含状态和结果）
 */
export type ContentPart =
  | { type: "text"; text: string }
  | { type: "tool_use"; toolCall: ToolCallState };

export interface Message {
  id: string;
  role: "user" | "assistant";
  /** 完整文本内容（向后兼容） */
  content: string;
  images?: MessageImage[];
  timestamp: Date;
  isThinking?: boolean;
  thinkingContent?: string;
  search_results?: any[]; // For potential future use
  /** 工具调用列表（assistant 消息可能包含） - 向后兼容 */
  toolCalls?: ToolCallState[];
  /** Token 使用量（响应完成后） */
  usage?: TokenUsage;
  /**
   * 交错内容列表（按事件到达顺序排列）
   * 如果存在且非空，StreamingRenderer 会按顺序渲染
   * 否则回退到 content + toolCalls 渲染方式
   */
  contentParts?: ContentPart[];
}

export interface ChatSession {
  id: string;
  title: string;
  providerType: string;
  model: string;
  messages: Message[];
  createdAt: Date;
  updatedAt: Date;
}

export const PROVIDER_CONFIG: Record<
  string,
  { label: string; models: string[] }
> = {
  claude: {
    label: "Claude",
    models: [
      "claude-opus-4-5",
      "claude-opus-4-5-20251101",
      "claude-opus-4-1",
      "claude-opus-4-1-20250805",
      "claude-opus-4-20250514",
      "claude-sonnet-4-5",
      "claude-sonnet-4-5-20250929",
      "claude-sonnet-4-0",
      "claude-sonnet-4-20250514",
      "claude-haiku-4-5",
      "claude-haiku-4-5-20251001",
      "claude-3-7-sonnet-20250219",
      "claude-3-5-haiku-20241022",
    ],
  },
  anthropic: {
    label: "Anthropic",
    models: [
      "claude-opus-4-5",
      "claude-opus-4-5-20251101",
      "claude-opus-4-1",
      "claude-opus-4-1-20250805",
      "claude-opus-4-20250514",
      "claude-sonnet-4-5",
      "claude-sonnet-4-5-20250929",
      "claude-sonnet-4-0",
      "claude-sonnet-4-20250514",
      "claude-haiku-4-5",
      "claude-haiku-4-5-20251001",
      "claude-3-7-sonnet-20250219",
      "claude-3-5-haiku-20241022",
    ],
  },
  kiro: {
    label: "Kiro",
    models: [
      "claude-opus-4-5",
      "claude-opus-4-5-20251101",
      "claude-haiku-4-5",
      "claude-sonnet-4-5",
      "claude-sonnet-4-5-20250929",
      "claude-sonnet-4-20250514",
      "claude-3-7-sonnet-20250219",
    ],
  },
  openai: {
    label: "OpenAI",
    models: [
      "gpt-5.2",
      "gpt-5.2-codex",
      "gpt-5.1",
      "gpt-5.1-codex-max",
      "gpt-5.1-codex",
      "gpt-5.1-codex-mini",
      "gpt-5",
      "gpt-5-pro",
      "gpt-5-codex",
      "gpt-5-codex-mini",
      "gpt-5-mini",
      "gpt-5-nano",
      "o4-mini",
      "o3",
      "o3-mini",
      "o1",
      "o1-mini",
      "gpt-4.1",
      "gpt-4.1-mini",
      "gpt-4.1-nano",
      "gpt-4o",
      "gpt-4o-mini",
    ],
  },
  gemini: {
    label: "Gemini",
    models: [
      "gemini-3-pro-preview",
      "gemini-3-flash-preview",
      "gemini-2.5-pro",
      "gemini-2.5-flash",
      "gemini-2.5-flash-lite",
    ],
  },
  qwen: {
    label: "通义千问",
    models: ["qwen3-coder-plus", "qwen3-coder-flash"],
  },
  codex: {
    label: "Codex",
    models: ["codex-mini-latest"],
  },
  claude_oauth: {
    label: "Claude OAuth",
    models: [
      "claude-opus-4-5",
      "claude-opus-4-5-20251101",
      "claude-sonnet-4-5",
      "claude-sonnet-4-5-20250929",
      "claude-sonnet-4-20250514",
      "claude-haiku-4-5",
      "claude-3-7-sonnet-20250219",
    ],
  },
  iflow: {
    label: "iFlow",
    models: [
      "tstars2.0",
      "qwen3-coder-plus",
      "qwen3-max",
      "qwen3-vl-plus",
      "qwen3-max-preview",
      "kimi-k2-0905",
      "glm-4.7",
      "glm-4.6",
      "kimi-k2",
      "kimi-k2-thinking",
      "deepseek-v3.2-chat",
      "deepseek-v3.2-reasoner",
      "deepseek-v3.2",
      "deepseek-v3.1",
      "deepseek-r1",
      "deepseek-v3",
      "minimax-m2.1",
      "minimax-m2",
    ],
  },
  antigravity: {
    label: "Antigravity",
    models: [
      "gemini-3-pro-preview",
      "gemini-3-pro-image-preview",
      "gemini-3-flash-preview",
      "gemini-2.5-flash",
      "gemini-2.5-computer-use-preview-10-2025",
      "gemini-claude-sonnet-4-5",
      "gemini-claude-sonnet-4-5-thinking",
      "gemini-claude-opus-4-5-thinking",
    ],
  },
};

// ============ 动态模型配置 API ============

/** 简化的 Provider 配置（从后端返回） */
export interface SimpleProviderConfig {
  label: string;
  models: string[];
}

/** Provider 配置映射类型 */
export type ProviderConfigMap = Record<string, SimpleProviderConfig>;

/**
 * 从后端获取所有 Provider 的模型配置
 * 如果获取失败，返回默认的 PROVIDER_CONFIG
 */
export async function getProviderConfig(): Promise<ProviderConfigMap> {
  try {
    const config = await safeInvoke<ProviderConfigMap>(
      "get_all_provider_models",
    );
    return config;
  } catch (error) {
    console.warn("获取模型配置失败，使用默认配置:", error);
    return PROVIDER_CONFIG;
  }
}

/**
 * 获取指定 Provider 的模型列表
 */
export async function getProviderModels(provider: string): Promise<string[]> {
  try {
    const models = await safeInvoke<string[]>("get_provider_models", {
      provider,
    });
    return models;
  } catch (error) {
    console.warn(`获取 ${provider} 模型列表失败:`, error);
    return PROVIDER_CONFIG[provider]?.models ?? [];
  }
}
