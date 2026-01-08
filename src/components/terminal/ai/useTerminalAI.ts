/**
 * @file useTerminalAI.ts
 * @description Terminal AI Hook - 管理 AI 聊天状态和操作
 * @module components/terminal/ai/useTerminalAI
 *
 * 复用 Agent 模块的 API，提供 Terminal 专用的 AI 聊天功能
 */

import { useState, useEffect, useCallback } from "react";
import { toast } from "sonner";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  startAgentProcess,
  getAgentProcessStatus,
  createAgentSession,
  sendAgentMessageStream,
  parseStreamEvent,
  type StreamEvent,
} from "@/lib/api/agent";
import type {
  AIMessage,
  AIMessageImage,
  AIContentPart,
  TerminalAIConfig,
  UseTerminalAIReturn,
} from "./types";

// 存储键
const STORAGE_KEYS = {
  PROVIDER: "terminal_ai_provider",
  MODEL: "terminal_ai_model",
  CONFIG: "terminal_ai_config",
  MESSAGES: "terminal_ai_messages",
};

// 默认配置
const DEFAULT_CONFIG: TerminalAIConfig = {
  widgetContext: true,
  contextLines: 50,
};

/**
 * 加载持久化数据
 */
const loadPersisted = <T>(key: string, defaultValue: T): T => {
  try {
    const stored = localStorage.getItem(key);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error("[useTerminalAI] 加载持久化数据失败:", e);
  }
  return defaultValue;
};

/**
 * 保存持久化数据
 */
const savePersisted = (key: string, value: unknown) => {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (e) {
    console.error("[useTerminalAI] 保存持久化数据失败:", e);
  }
};

/**
 * Terminal AI Hook
 *
 * @param getTerminalOutput - 获取终端输出的回调函数
 */
export function useTerminalAI(
  getTerminalOutput?: () => string | null,
): UseTerminalAIReturn {
  // 模型选择状态
  const [providerId, setProviderId] = useState(() =>
    loadPersisted(STORAGE_KEYS.PROVIDER, "claude"),
  );
  const [modelId, setModelId] = useState(() =>
    loadPersisted(STORAGE_KEYS.MODEL, "claude-sonnet-4-20250514"),
  );

  // 配置状态
  const [config, setConfig] = useState<TerminalAIConfig>(() =>
    loadPersisted(STORAGE_KEYS.CONFIG, DEFAULT_CONFIG),
  );

  // 消息状态
  const [messages, setMessages] = useState<AIMessage[]>([]);
  const [isSending, setIsSending] = useState(false);

  // 会话 ID
  const [sessionId, setSessionId] = useState<string | null>(null);

  // 持久化
  useEffect(() => {
    savePersisted(STORAGE_KEYS.PROVIDER, providerId);
  }, [providerId]);

  useEffect(() => {
    savePersisted(STORAGE_KEYS.MODEL, modelId);
  }, [modelId]);

  useEffect(() => {
    savePersisted(STORAGE_KEYS.CONFIG, config);
  }, [config]);

  // 初始化 Agent 进程
  useEffect(() => {
    const initAgent = async () => {
      try {
        const status = await getAgentProcessStatus();
        if (!status.running) {
          await startAgentProcess();
        }
      } catch (e) {
        console.error("[useTerminalAI] 初始化 Agent 失败:", e);
      }
    };
    initAgent();
  }, []);

  /**
   * 确保会话存在
   */
  const ensureSession = useCallback(async (): Promise<string | null> => {
    if (sessionId) return sessionId;

    try {
      // 构建系统提示词
      const systemPrompt = `你是一个终端助手，帮助用户解决命令行相关的问题。
你可以：
- 解释命令的用法和参数
- 帮助调试错误信息
- 建议更好的命令或脚本
- 解答 shell 脚本相关问题

请用简洁清晰的语言回答，必要时提供代码示例。`;

      const response = await createAgentSession(
        providerId,
        modelId,
        systemPrompt,
      );

      setSessionId(response.session_id);
      return response.session_id;
    } catch (error) {
      console.error("[useTerminalAI] 创建会话失败:", error);
      toast.error("创建 AI 会话失败");
      return null;
    }
  }, [sessionId, providerId, modelId]);

  /**
   * 获取终端上下文
   */
  const getTerminalContext = useCallback((): string | null => {
    if (!config.widgetContext || !getTerminalOutput) {
      return null;
    }

    const output = getTerminalOutput();
    if (!output) return null;

    // 限制行数
    const lines = output.split("\n");
    const limitedLines = lines.slice(-config.contextLines);
    return limitedLines.join("\n");
  }, [config.widgetContext, config.contextLines, getTerminalOutput]);

  /**
   * 发送消息
   */
  const sendMessage = useCallback(
    async (content: string, images?: AIMessageImage[]) => {
      if (!content.trim() && (!images || images.length === 0)) return;

      // 创建用户消息
      const userMsg: AIMessage = {
        id: crypto.randomUUID(),
        role: "user",
        content,
        images,
        timestamp: new Date(),
      };

      // 创建助手消息占位符
      const assistantMsgId = crypto.randomUUID();
      const assistantMsg: AIMessage = {
        id: assistantMsgId,
        role: "assistant",
        content: "",
        timestamp: new Date(),
        isThinking: true,
        thinkingContent: "思考中...",
        contentParts: [],
      };

      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setIsSending(true);

      let accumulatedContent = "";
      let unlisten: UnlistenFn | null = null;

      /**
       * 追加文本到 contentParts
       */
      const appendTextToParts = (
        parts: AIContentPart[],
        text: string,
      ): AIContentPart[] => {
        const newParts = [...parts];
        const lastPart = newParts[newParts.length - 1];

        if (lastPart && lastPart.type === "text") {
          newParts[newParts.length - 1] = {
            type: "text",
            text: lastPart.text + text,
          };
        } else {
          newParts.push({ type: "text", text });
        }
        return newParts;
      };

      try {
        const activeSessionId = await ensureSession();
        if (!activeSessionId) {
          throw new Error("无法创建会话");
        }

        const eventName = `terminal_ai_stream_${assistantMsgId}`;

        // 设置事件监听
        unlisten = await listen<StreamEvent>(eventName, (event) => {
          const data = parseStreamEvent(event.payload);
          if (!data) return;

          switch (data.type) {
            case "text_delta":
              accumulatedContent += data.text;
              setMessages((prev) =>
                prev.map((msg) =>
                  msg.id === assistantMsgId
                    ? {
                        ...msg,
                        content: accumulatedContent,
                        thinkingContent: undefined,
                        contentParts: appendTextToParts(
                          msg.contentParts || [],
                          data.text,
                        ),
                      }
                    : msg,
                ),
              );
              break;

            case "final_done":
              setMessages((prev) =>
                prev.map((msg) =>
                  msg.id === assistantMsgId
                    ? {
                        ...msg,
                        isThinking: false,
                        content: accumulatedContent || "(无响应)",
                      }
                    : msg,
                ),
              );
              setIsSending(false);
              if (unlisten) {
                unlisten();
                unlisten = null;
              }
              break;

            case "error":
              toast.error(`AI 响应错误: ${data.message}`);
              setMessages((prev) =>
                prev.map((msg) =>
                  msg.id === assistantMsgId
                    ? {
                        ...msg,
                        isThinking: false,
                        content: accumulatedContent || `错误: ${data.message}`,
                      }
                    : msg,
                ),
              );
              setIsSending(false);
              if (unlisten) {
                unlisten();
                unlisten = null;
              }
              break;

            case "tool_start": {
              const newToolCall = {
                id: data.tool_id,
                name: data.tool_name,
                arguments: data.arguments,
                status: "running" as const,
                startTime: new Date(),
              };
              setMessages((prev) =>
                prev.map((msg) =>
                  msg.id === assistantMsgId
                    ? {
                        ...msg,
                        toolCalls: [...(msg.toolCalls || []), newToolCall],
                        contentParts: [
                          ...(msg.contentParts || []),
                          { type: "tool_use" as const, toolCall: newToolCall },
                        ],
                      }
                    : msg,
                ),
              );
              break;
            }

            case "tool_end": {
              setMessages((prev) =>
                prev.map((msg) => {
                  if (msg.id !== assistantMsgId) return msg;

                  const updatedToolCalls = (msg.toolCalls || []).map((tc) =>
                    tc.id === data.tool_id
                      ? {
                          ...tc,
                          status: data.result.success
                            ? ("completed" as const)
                            : ("failed" as const),
                          result: data.result,
                          endTime: new Date(),
                        }
                      : tc,
                  );

                  const updatedContentParts = (msg.contentParts || []).map(
                    (part) => {
                      if (
                        part.type === "tool_use" &&
                        part.toolCall.id === data.tool_id
                      ) {
                        return {
                          ...part,
                          toolCall: {
                            ...part.toolCall,
                            status: data.result.success
                              ? ("completed" as const)
                              : ("failed" as const),
                            result: data.result,
                            endTime: new Date(),
                          },
                        };
                      }
                      return part;
                    },
                  );

                  return {
                    ...msg,
                    toolCalls: updatedToolCalls,
                    contentParts: updatedContentParts,
                  };
                }),
              );
              break;
            }
          }
        });

        // 构建消息内容（包含终端上下文）
        let messageContent = content;
        const terminalContext = getTerminalContext();
        if (terminalContext) {
          messageContent = `[终端上下文]\n\`\`\`\n${terminalContext}\n\`\`\`\n\n[用户问题]\n${content}`;
        }

        // 发送请求
        const imagesToSend = images?.map((img) => ({
          data: img.data,
          media_type: img.mediaType,
        }));

        await sendAgentMessageStream(
          messageContent,
          eventName,
          activeSessionId,
          modelId,
          imagesToSend,
          providerId,
        );
      } catch (error) {
        console.error("[useTerminalAI] 发送消息失败:", error);
        toast.error(`发送失败: ${error}`);
        setMessages((prev) => prev.filter((msg) => msg.id !== assistantMsgId));
        setIsSending(false);
        if (unlisten) {
          unlisten();
        }
      }
    },
    [ensureSession, getTerminalContext, modelId, providerId],
  );

  /**
   * 清空消息
   */
  const clearMessages = useCallback(() => {
    setMessages([]);
    setSessionId(null);
    toast.success("对话已清空");
  }, []);

  /**
   * 切换终端上下文
   */
  const toggleWidgetContext = useCallback(() => {
    setConfig((prev) => ({
      ...prev,
      widgetContext: !prev.widgetContext,
    }));
  }, []);

  /**
   * 设置上下文行数
   */
  const setContextLines = useCallback((lines: number) => {
    setConfig((prev) => ({
      ...prev,
      contextLines: lines,
    }));
  }, []);

  return {
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
    setContextLines,
    getTerminalContext,
  };
}
