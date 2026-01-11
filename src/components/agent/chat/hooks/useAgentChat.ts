import { useState, useEffect, useRef } from "react";
import { toast } from "sonner";
import { safeListen } from "@/lib/dev-bridge";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  startAgentProcess,
  stopAgentProcess,
  getAgentProcessStatus,
  createAgentSession,
  sendAgentMessageStream,
  listAgentSessions,
  deleteAgentSession,
  getAgentSessionMessages,
  parseStreamEvent,
  type AgentProcessStatus,
  type SessionInfo,
  type StreamEvent,
} from "@/lib/api/agent";
import {
  Message,
  MessageImage,
  ContentPart,
  PROVIDER_CONFIG,
  getProviderConfig,
  type ProviderConfigMap,
} from "../types";

/** 话题（会话）信息 */
export interface Topic {
  id: string;
  title: string;
  createdAt: Date;
  messagesCount: number;
}

// 音效播放器（模块级别单例）
let toolcallAudio: HTMLAudioElement | null = null;
let typewriterAudio: HTMLAudioElement | null = null;
let lastTypewriterTime = 0;
const TYPEWRITER_INTERVAL = 120;

const initAudio = () => {
  if (!toolcallAudio) {
    toolcallAudio = new Audio("/sounds/tool-call.mp3");
    toolcallAudio.volume = 1;
    toolcallAudio.load();
  }
  if (!typewriterAudio) {
    typewriterAudio = new Audio("/sounds/typing.mp3");
    typewriterAudio.volume = 0.6;
    typewriterAudio.load();
  }
};

const getSoundEnabled = (): boolean => {
  return localStorage.getItem("proxycast_sound_enabled") === "true";
};

const playToolcallSound = () => {
  if (!getSoundEnabled()) return;
  initAudio();
  if (toolcallAudio) {
    toolcallAudio.currentTime = 0;
    toolcallAudio.play().catch(console.error);
  }
};

const playTypewriterSound = () => {
  if (!getSoundEnabled()) return;
  const now = Date.now();
  if (now - lastTypewriterTime < TYPEWRITER_INTERVAL) return;
  initAudio();
  if (typewriterAudio) {
    typewriterAudio.currentTime = 0;
    typewriterAudio.play().catch(console.error);
    lastTypewriterTime = now;
  }
};

// Helper for localStorage (Persistent across reloads)
const loadPersisted = <T>(key: string, defaultValue: T): T => {
  try {
    const stored = localStorage.getItem(key);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error(e);
  }
  return defaultValue;
};

const savePersisted = (key: string, value: unknown) => {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (e) {
    console.error(e);
  }
};

// Helper for session storage (Transient data like messages)
const loadTransient = <T>(key: string, defaultValue: T): T => {
  try {
    const stored = sessionStorage.getItem(key);
    if (stored) {
      const parsed = JSON.parse(stored);
      if (key === "agent_messages" && Array.isArray(parsed)) {
        return parsed.map((msg: any) => ({
          ...msg,
          timestamp: new Date(msg.timestamp),
        })) as unknown as T;
      }
      return parsed;
    }
  } catch (e) {
    console.error(e);
  }
  return defaultValue;
};

const saveTransient = (key: string, value: unknown) => {
  try {
    sessionStorage.setItem(key, JSON.stringify(value));
  } catch (e) {
    console.error(e);
  }
};

/** useAgentChat 的配置选项 */
interface UseAgentChatOptions {
  /** 系统提示词（用于内容创作等场景） */
  systemPrompt?: string;
  /** 文件写入回调 */
  onWriteFile?: (content: string, fileName: string) => void;
}

export function useAgentChat(options: UseAgentChatOptions = {}) {
  const { systemPrompt, onWriteFile } = options;
  const [processStatus, setProcessStatus] = useState<AgentProcessStatus>({
    running: false,
  });

  // 动态模型配置（从后端加载）
  const [providerConfig, setProviderConfig] =
    useState<ProviderConfigMap>(PROVIDER_CONFIG);
  const [isConfigLoading, setIsConfigLoading] = useState(true);

  // Configuration State (Persistent)
  const defaultProvider = "claude";
  const defaultModel = PROVIDER_CONFIG["claude"]?.models[0] || "";

  const [providerType, setProviderType] = useState(() =>
    loadPersisted("agent_pref_provider", defaultProvider),
  );
  const [model, setModel] = useState(() =>
    loadPersisted("agent_pref_model", defaultModel),
  );

  // Session State
  const [sessionId, setSessionId] = useState<string | null>(() =>
    loadTransient("agent_curr_sessionId", null),
  );
  const [messages, setMessages] = useState<Message[]>(() =>
    loadTransient("agent_messages", []),
  );

  // 话题列表
  const [topics, setTopics] = useState<Topic[]>([]);

  const [isSending, setIsSending] = useState(false);

  // 用于保存当前流式请求的取消函数
  const unlistenRef = useRef<UnlistenFn | null>(null);
  // 用于保存当前正在处理的消息 ID
  const currentAssistantMsgIdRef = useRef<string | null>(null);

  // 加载动态模型配置
  useEffect(() => {
    const loadConfig = async () => {
      try {
        const config = await getProviderConfig();
        setProviderConfig(config);
      } catch (error) {
        console.warn("加载模型配置失败，使用默认配置:", error);
      } finally {
        setIsConfigLoading(false);
      }
    };
    loadConfig();
  }, []);

  // Persistence Effects
  useEffect(() => {
    savePersisted("agent_pref_provider", providerType);
  }, [providerType]);
  useEffect(() => {
    savePersisted("agent_pref_model", model);
  }, [model]);

  // 当 provider 改变时，检查当前模型是否兼容
  // 如果不兼容，自动切换到新 provider 的第一个模型
  // 注意：model 不能放在依赖中，否则会导致无限循环
  useEffect(() => {
    const currentProviderModels = providerConfig[providerType]?.models || [];
    // 只有当模型列表非空时才检查兼容性
    if (currentProviderModels.length > 0) {
      // 使用 setModel 的函数形式来访问当前 model 值，避免将 model 放入依赖
      setModel((currentModel) => {
        if (!currentProviderModels.includes(currentModel)) {
          console.log(
            `[useAgentChat] 模型 ${currentModel} 不在 ${providerType} 支持列表中，自动切换到 ${currentProviderModels[0]}`,
          );
          return currentProviderModels[0];
        }
        return currentModel;
      });
    }
  }, [providerType, providerConfig]);

  useEffect(() => {
    saveTransient("agent_curr_sessionId", sessionId);
  }, [sessionId]);
  useEffect(() => {
    saveTransient("agent_messages", messages);
  }, [messages]);

  // 当 systemPrompt 变化时，需要创建新会话以应用新的系统提示词
  // 这对于内容创作模式切换非常重要
  useEffect(() => {
    if (systemPrompt !== undefined && sessionId) {
      console.log(
        "[useAgentChat] systemPrompt 变化，重置 session 以应用新提示词",
      );
      setSessionId(null);
    }
    // 注意：只在 systemPrompt 变化时触发，不包含 sessionId
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [systemPrompt]);

  // 加载话题列表
  const loadTopics = async () => {
    try {
      const sessions = await listAgentSessions();
      const topicList: Topic[] = sessions.map((s: SessionInfo) => ({
        id: s.session_id,
        title: generateTopicTitle(s),
        createdAt: new Date(s.created_at),
        messagesCount: s.messages_count,
      }));
      setTopics(topicList);
    } catch (error) {
      console.error("加载话题列表失败:", error);
    }
  };

  // 根据会话信息生成话题标题
  const generateTopicTitle = (session: SessionInfo): string => {
    if (session.messages_count === 0) {
      return "新话题";
    }
    // 使用创建时间作为默认标题
    const date = new Date(session.created_at);
    return `话题 ${date.toLocaleDateString("zh-CN")} ${date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}`;
  };

  // Initial Load
  useEffect(() => {
    getAgentProcessStatus().then(setProcessStatus).catch(console.error);
    loadTopics();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 监听截图对话消息事件
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await safeListen<{
        message: string;
        image_path: string | null;
        image_base64: string | null;
      }>("screenshot-chat-message", async (event) => {
        console.log("[AgentChat] 收到截图对话消息:", event.payload);
        const { message, image_base64 } = event.payload;

        // 构建图片数组
        const images: MessageImage[] = [];
        if (image_base64) {
          images.push({
            data: image_base64,
            mediaType: "image/png",
          });
        }

        // 发送消息
        await sendMessage(message, images, false, false);
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerType, model, sessionId]);

  // 当 sessionId 变化时刷新话题列表
  useEffect(() => {
    if (sessionId) {
      loadTopics();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // Ensure an active session exists (internal helper)
  const _ensureSession = async (): Promise<string | null> => {
    // If we already have a session, we might want to continue using it.
    // However, check if we need to "re-initialize" if critical params changed?
    // User said: "选择模型后，不用和会话绑定". So we keep the session ID if it exists.
    if (sessionId) return sessionId;

    try {
      // TEMPORARY FIX: Disable skills integration due to API type mismatch (Backend expects []SystemMessage, Client sends String)
      // const [claudeSkills, proxyCastSkills] = await Promise.all([
      //     skillsApi.getAll("claude").catch(() => []),
      //     skillsApi.getInstalledProxyCastSkills().catch(() => []),
      // ]);

      // const details: SkillInfo[] = claudeSkills.filter(s => s.installed).map(s => ({
      //     name: s.name,
      //     description: s.description,
      //     path: s.directory ? `~/.claude/skills/${s.directory}/SKILL.md` : undefined,
      // }));

      // proxyCastSkills.forEach(name => {
      //     if (!details.find(d => d.name === name)) {
      //         details.push({ name, path: `~/.proxycast/skills/${name}/SKILL.md` });
      //     }
      // });

      // Create new session with CURRENT provider/model as baseline
      // 传递 systemPrompt 用于内容创作等场景
      const response = await createAgentSession(
        providerType,
        model || undefined,
        systemPrompt, // 传递系统提示词
        undefined, // details.length > 0 ? details : undefined
      );

      setSessionId(response.session_id);
      return response.session_id;
    } catch (error) {
      console.error("[AgentChat] Auto-creation failed:", error);
      toast.error("Failed to initialize session", {
        id: "session-init-error",
        duration: 8000,
      });
      return null;
    }
  };

  const sendMessage = async (
    content: string,
    images: MessageImage[],
    webSearch?: boolean,
    thinking?: boolean,
  ) => {
    // 1. Optimistic UI Update
    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: "user",
      content,
      images: images.length > 0 ? images : undefined,
      timestamp: new Date(),
    };

    // Placeholder for assistant
    const assistantMsgId = crypto.randomUUID();
    let thinkingText = "思考中...";
    if (thinking && webSearch) {
      thinkingText = "深度思考 + 联网搜索中...";
    } else if (thinking) {
      thinkingText = "深度思考中...";
    } else if (webSearch) {
      thinkingText = "正在搜索网络...";
    }

    const assistantMsg: Message = {
      id: assistantMsgId,
      role: "assistant",
      content: "",
      timestamp: new Date(),
      isThinking: true,
      thinkingContent: thinkingText,
      contentParts: [], // 初始化交错内容列表
    };

    setMessages((prev) => [...prev, userMsg, assistantMsg]);
    setIsSending(true);

    // 保存当前消息 ID 到 ref，用于停止时更新状态
    currentAssistantMsgIdRef.current = assistantMsgId;

    // 用于累积流式内容
    let accumulatedContent = "";
    let unlisten: UnlistenFn | null = null;

    /**
     * 辅助函数：更新 contentParts，支持交错显示
     * - text_delta: 追加到最后一个 text 类型，或创建新的 text 类型
     * - tool_start: 添加新的 tool_use 类型
     * - tool_end: 更新对应的 tool_use 状态
     */
    const appendTextToParts = (
      parts: ContentPart[],
      text: string,
    ): ContentPart[] => {
      const newParts = [...parts];
      const lastPart = newParts[newParts.length - 1];

      if (lastPart && lastPart.type === "text") {
        // 追加到最后一个 text 类型
        newParts[newParts.length - 1] = {
          type: "text",
          text: lastPart.text + text,
        };
      } else {
        // 创建新的 text 类型
        newParts.push({ type: "text", text });
      }
      return newParts;
    };

    try {
      // 2. 确保有一个活跃的 session（用于保持上下文）
      const activeSessionId = await _ensureSession();
      if (!activeSessionId) {
        throw new Error("无法创建或获取会话");
      }

      // 3. 创建唯一事件名称
      const eventName = `agent_stream_${assistantMsgId}`;

      // 4. 设置事件监听器（流式接收）
      console.log(
        `[AgentChat] 设置事件监听器: ${eventName}, sessionId: ${activeSessionId}`,
      );
      unlisten = await safeListen<StreamEvent>(eventName, (event) => {
        console.log("[AgentChat] 收到事件:", eventName, event.payload);
        const data = parseStreamEvent(event.payload);
        if (!data) {
          console.warn("[AgentChat] 解析事件失败:", event.payload);
          return;
        }
        console.log("[AgentChat] 解析后数据:", data);

        switch (data.type) {
          case "text_delta":
            // 累积文本并实时更新 UI（同时更新 content 和 contentParts）
            accumulatedContent += data.text;

            // 播放打字机音效
            playTypewriterSound();

            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === assistantMsgId
                  ? {
                      ...msg,
                      content: accumulatedContent,
                      thinkingContent: undefined,
                      // 更新 contentParts，支持交错显示
                      contentParts: appendTextToParts(
                        msg.contentParts || [],
                        data.text,
                      ),
                    }
                  : msg,
              ),
            );
            break;

          case "done":
            // 完成一次 API 响应，但工具循环可能还在继续
            // 不要取消监听，继续等待更多事件
            console.log("[AgentChat] 收到 done 事件，工具循环可能还在继续...");
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === assistantMsgId
                  ? {
                      ...msg,
                      // 保持 isThinking 为 true，直到收到 final_done 或 error
                      content: accumulatedContent || msg.content,
                    }
                  : msg,
              ),
            );
            // 注意：不要在这里 setIsSending(false) 或 unlisten()
            // 工具循环会继续发送事件
            break;

          case "final_done":
            // 整个对话完成（包括所有工具调用）
            console.log("[AgentChat] 收到 final_done 事件，对话完成");
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === assistantMsgId
                  ? {
                      ...msg,
                      isThinking: false,
                      content: accumulatedContent || "(No response)",
                    }
                  : msg,
              ),
            );
            setIsSending(false);
            // 清理 ref
            unlistenRef.current = null;
            currentAssistantMsgIdRef.current = null;
            if (unlisten) {
              unlisten();
              unlisten = null;
            }
            break;

          case "error":
            // 错误处理
            console.error("[AgentChat] Stream error:", data.message);
            toast.error(`响应错误: ${data.message}`, {
              id: `stream-error-${Date.now()}`,
              duration: 8000,
            });
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
            // 清理 ref
            unlistenRef.current = null;
            currentAssistantMsgIdRef.current = null;
            if (unlisten) {
              unlisten();
              unlisten = null;
            }
            break;

          case "tool_start": {
            // 工具开始执行 - 添加到工具调用列表和 contentParts
            console.log(`[Tool Start] ${data.tool_name} (${data.tool_id})`);

            // 播放工具调用音效
            playToolcallSound();

            const newToolCall = {
              id: data.tool_id,
              name: data.tool_name,
              arguments: data.arguments,
              status: "running" as const,
              startTime: new Date(),
            };

            // 如果是写入文件工具，立即调用 onWriteFile 展开右边栏
            const toolName = data.tool_name.toLowerCase();
            if (toolName.includes("write") || toolName.includes("create")) {
              try {
                const args = JSON.parse(data.arguments || "{}");
                const filePath = args.path || args.file_path || args.filePath;
                const content = args.content || args.text || "";
                if (filePath && content && onWriteFile) {
                  console.log(`[Tool Start] 触发文件写入: ${filePath}`);
                  onWriteFile(content, filePath);
                }
              } catch (e) {
                console.warn("[Tool Start] 解析工具参数失败:", e);
              }
            }

            setMessages((prev) =>
              prev.map((msg) => {
                if (msg.id !== assistantMsgId) return msg;

                // 检查是否已存在相同 ID 的工具调用（避免重复）
                const existingToolCall = msg.toolCalls?.find(
                  (tc) => tc.id === data.tool_id,
                );
                if (existingToolCall) {
                  console.log(
                    `[Tool Start] 工具调用已存在，跳过: ${data.tool_id}`,
                  );
                  return msg;
                }

                return {
                  ...msg,
                  toolCalls: [...(msg.toolCalls || []), newToolCall],
                  // 添加到 contentParts，支持交错显示
                  contentParts: [
                    ...(msg.contentParts || []),
                    { type: "tool_use" as const, toolCall: newToolCall },
                  ],
                };
              }),
            );
            break;
          }

          case "tool_end": {
            // 工具执行完成 - 更新工具调用状态和 contentParts
            console.log(`[Tool End] ${data.tool_id}`);
            setMessages((prev) =>
              prev.map((msg) => {
                if (msg.id !== assistantMsgId) return msg;

                // 更新 toolCalls
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

                // 更新 contentParts 中对应的 tool_use
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

      // 保存 unlisten 到 ref，用于停止功能
      unlistenRef.current = unlisten;

      // 5. 发送流式请求（传递 sessionId 以保持上下文）
      const imagesToSend =
        images.length > 0
          ? images.map((img) => ({ data: img.data, media_type: img.mediaType }))
          : undefined;

      console.log("[AgentChat] 发送消息:", {
        content: content.slice(0, 50),
        sessionId: activeSessionId,
        model,
        provider: providerType,
      });

      await sendAgentMessageStream(
        content,
        eventName,
        activeSessionId, // 传递 sessionId 以保持上下文
        model || undefined,
        imagesToSend,
        providerType, // 传递用户选择的 provider
      );
    } catch (error) {
      console.error("[AgentChat] Send failed:", error);
      toast.error(`发送失败: ${error}`, {
        id: `send-error-${Date.now()}`,
        duration: 8000,
      });
      // Remove the optimistic assistant message on failure
      setMessages((prev) => prev.filter((msg) => msg.id !== assistantMsgId));
      setIsSending(false);
      if (unlisten) {
        unlisten();
      }
    }
  };

  // 删除单条消息
  const deleteMessage = (id: string) => {
    setMessages((prev) => prev.filter((msg) => msg.id !== id));
  };

  // 编辑消息
  const editMessage = (id: string, newContent: string) => {
    setMessages((prev) =>
      prev.map((msg) =>
        msg.id === id ? { ...msg, content: newContent } : msg,
      ),
    );
  };

  const clearMessages = () => {
    setMessages([]);
    setSessionId(null);
    toast.success("新话题已创建");
  };

  // 切换话题
  const switchTopic = async (topicId: string) => {
    if (topicId === sessionId) return;

    try {
      // 从后端加载消息历史
      const agentMessages = await getAgentSessionMessages(topicId);

      // 转换为前端 Message 格式
      const loadedMessages: Message[] = agentMessages.map((msg, index) => {
        // 提取文本内容
        let content = "";
        if (typeof msg.content === "string") {
          content = msg.content;
        } else if (Array.isArray(msg.content)) {
          content = msg.content
            .filter(
              (part): part is { type: "text"; text: string } =>
                part.type === "text",
            )
            .map((part) => part.text)
            .join("\n");
        }

        return {
          id: `${topicId}-${index}`,
          role: msg.role as "user" | "assistant",
          content,
          timestamp: new Date(msg.timestamp),
          isThinking: false,
        };
      });

      setMessages(loadedMessages);
      setSessionId(topicId);
      toast.info("已切换话题");
    } catch (error) {
      console.error("[useAgentChat] 加载消息历史失败:", error);
      // 如果加载失败，仍然切换话题但清空消息
      setMessages([]);
      setSessionId(topicId);
      toast.error("加载对话历史失败");
    }
  };

  // 删除话题
  const deleteTopic = async (topicId: string) => {
    try {
      await deleteAgentSession(topicId);
      setTopics((prev) => prev.filter((t) => t.id !== topicId));

      // 如果删除的是当前话题，清空状态
      if (topicId === sessionId) {
        setSessionId(null);
        setMessages([]);
      }
      toast.success("话题已删除");
    } catch (_error) {
      toast.error("删除话题失败");
    }
  };

  // Status management wrappers
  const handleStartProcess = async () => {
    try {
      await startAgentProcess();
      setProcessStatus({ running: true });
    } catch (_e) {
      toast.error("Start failed");
    }
  };

  const handleStopProcess = async () => {
    try {
      await stopAgentProcess();
      setProcessStatus({ running: false });
      setSessionId(null); // Reset session on stop
    } catch (_e) {
      toast.error("Stop failed");
    }
  };

  // 停止当前发送中的消息
  const stopSending = () => {
    // 取消事件监听
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }

    // 更新当前消息状态为已停止
    if (currentAssistantMsgIdRef.current) {
      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === currentAssistantMsgIdRef.current
            ? {
                ...msg,
                isThinking: false,
                content: msg.content || "(已停止生成)",
              }
            : msg,
        ),
      );
      currentAssistantMsgIdRef.current = null;
    }

    setIsSending(false);
    toast.info("已停止生成");
  };

  return {
    processStatus,
    handleStartProcess,
    handleStopProcess,

    // Config
    providerType,
    setProviderType,
    model,
    setModel,
    providerConfig, // 动态模型配置
    isConfigLoading, // 配置加载状态

    // Chat
    messages,
    isSending,
    sendMessage,
    stopSending,
    clearMessages,
    deleteMessage,
    editMessage,

    // 话题管理
    topics,
    sessionId,
    switchTopic,
    deleteTopic,
    loadTopics,
  };
}
