import React, { useState, useMemo, useEffect, useRef } from "react";
import { Bot, ChevronDown, Check, Box, Settings2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Navbar } from "../styles";
import { cn } from "@/lib/utils";
import { useProviderPool } from "@/hooks/useProviderPool";
import { useApiKeyProvider } from "@/hooks/useApiKeyProvider";
import { useModelRegistry } from "@/hooks/useModelRegistry";
import { getDefaultProvider } from "@/hooks/useTauri";

// Provider type 到 registry ID 的映射（用于获取模型列表）
const getRegistryIdFromType = (providerType: string): string => {
  const typeMap: Record<string, string> = {
    openai: "openai",
    anthropic: "anthropic",
    gemini: "google",
    "azure-openai": "openai",
    vertexai: "google",
    ollama: "ollama",
    kiro: "anthropic",
    claude: "anthropic",
    claude_oauth: "anthropic",
    qwen: "alibaba",
    codex: "openai",
    antigravity: "google",
    iflow: "openai",
    gemini_api_key: "google",
  };
  return typeMap[providerType.toLowerCase()] || providerType.toLowerCase();
};

// 生成 Provider 的显示标签
const getProviderLabel = (providerType: string): string => {
  const labelMap: Record<string, string> = {
    kiro: "Kiro",
    gemini: "Gemini",
    qwen: "通义千问",
    antigravity: "Antigravity",
    codex: "Codex",
    claude_oauth: "Claude OAuth",
    claude: "Claude",
    openai: "OpenAI",
    anthropic: "Anthropic",
    "azure-openai": "Azure OpenAI",
    vertexai: "VertexAI",
    ollama: "Ollama",
    gemini_api_key: "Gemini",
    iflow: "iFlow",
  };
  // 如果在映射表中，使用映射；否则首字母大写
  return (
    labelMap[providerType.toLowerCase()] ||
    providerType.charAt(0).toUpperCase() + providerType.slice(1)
  );
};

/** 已配置的 Provider 信息 */
interface ConfiguredProvider {
  key: string;
  label: string;
  registryId: string;
  fallbackRegistryId?: string; // 当 registryId 没有模型时的回退
  type: string; // 原始 provider type，用于确定 API 协议
}

interface ChatNavbarProps {
  providerType: string;
  setProviderType: (type: string) => void;
  model: string;
  setModel: (model: string) => void;
  isRunning: boolean;
  onToggleHistory: () => void;
  onToggleFullscreen: () => void;
  onToggleSettings?: () => void;
}

export const ChatNavbar: React.FC<ChatNavbarProps> = ({
  providerType,
  setProviderType,
  model,
  setModel,
  isRunning: _isRunning,
  onToggleHistory,
  onToggleFullscreen: _onToggleFullscreen,
  onToggleSettings,
}) => {
  const [open, setOpen] = useState(false);
  const [serverDefaultProvider, setServerDefaultProvider] = useState<
    string | null
  >(null);

  // 用于防止无限循环
  const hasInitialized = useRef(false);

  // 获取凭证池数据
  const { overview: oauthCredentials } = useProviderPool();
  const { providers: apiKeyProviders } = useApiKeyProvider();

  // 获取服务器默认 Provider
  useEffect(() => {
    const loadDefaultProvider = async () => {
      try {
        const dp = await getDefaultProvider();
        setServerDefaultProvider(dp);
      } catch (e) {
        console.error("Failed to get default provider:", e);
      }
    };
    loadDefaultProvider();
  }, []);

  // 获取模型注册表数据
  const { models: registryModels } = useModelRegistry({ autoLoad: true });

  // 计算已配置的 Provider 列表（完全动态，无白名单限制）
  const configuredProviders = useMemo(() => {
    const providerMap = new Map<string, ConfiguredProvider>();

    // 从 OAuth 凭证提取 Provider（动态，支持所有类型）
    oauthCredentials.forEach((overview) => {
      if (overview.credentials.length > 0) {
        const key = overview.provider_type;
        if (!providerMap.has(key)) {
          providerMap.set(key, {
            key,
            label: getProviderLabel(key),
            registryId: getRegistryIdFromType(key),
            type: key,
          });
        }
      }
    });

    // 从 API Key Provider 提取（动态，支持所有自定义 Provider）
    // 使用 provider.id 作为 key，确保每个 Provider 单独显示
    apiKeyProviders
      .filter((p) => p.api_key_count > 0 && p.enabled)
      .forEach((provider) => {
        const key = provider.id; // 使用 provider.id 而不是 type 映射
        if (!providerMap.has(key)) {
          // 优先使用 provider.id 作为 registryId（适用于系统预设的 Provider，如 deepseek, moonshot）
          // 如果模型注册表中没有该 id 的模型，则回退到使用 type 映射（适用于自定义 Provider）
          providerMap.set(key, {
            key,
            label: provider.name, // 使用 Provider 的 name 作为显示名称
            registryId: provider.id, // 先尝试用 id
            fallbackRegistryId: getRegistryIdFromType(provider.type), // 回退用 type
            type: provider.type,
          });
        }
      });

    return Array.from(providerMap.values());
  }, [oauthCredentials, apiKeyProviders]);

  // 获取当前选中 Provider 的配置
  const selectedProvider = useMemo(() => {
    return configuredProviders.find((p) => p.key === providerType);
  }, [configuredProviders, providerType]);

  // 获取当前 Provider 的模型列表（从 model_registry 获取）
  // 按照模型版本排序，最新的在前面
  const currentModels = useMemo(() => {
    if (!selectedProvider) return [];

    // 从 model_registry 获取模型
    // 优先使用 registryId，如果没有模型则回退到 fallbackRegistryId
    let models = registryModels
      .filter((m) => m.provider_id === selectedProvider.registryId)
      .map((m) => m.id);

    // 如果没有找到模型，尝试使用 fallbackRegistryId
    if (models.length === 0 && selectedProvider.fallbackRegistryId) {
      models = registryModels
        .filter((m) => m.provider_id === selectedProvider.fallbackRegistryId)
        .map((m) => m.id);
    }

    // 按照模型名称排序，优先显示最新版本
    // 排序规则：
    // 1. 带日期后缀的模型（如 claude-opus-4-5-20251101）按日期降序
    // 2. 带 "latest" 后缀的模型排在最前面
    // 3. 其他模型按字母顺序
    return models.sort((a, b) => {
      const aIsLatest = a.includes("-latest");
      const bIsLatest = b.includes("-latest");

      // latest 版本排在最前面
      if (aIsLatest && !bIsLatest) return -1;
      if (!aIsLatest && bIsLatest) return 1;

      // 提取日期后缀（如 20251101）
      const dateRegex = /-(\d{8})$/;
      const aMatch = a.match(dateRegex);
      const bMatch = b.match(dateRegex);

      if (aMatch && bMatch) {
        // 两个都有日期，按日期降序（最新的在前）
        return bMatch[1].localeCompare(aMatch[1]);
      }

      if (aMatch && !bMatch) return -1; // 有日期的排在前面
      if (!aMatch && bMatch) return 1;

      // 其他情况按字母降序（通常版本号大的在前）
      return b.localeCompare(a);
    });
  }, [selectedProvider, registryModels]);

  // 初始化：优先选择服务器默认 Provider，否则选择第一个已配置的
  useEffect(() => {
    if (hasInitialized.current) return;
    if (configuredProviders.length === 0) return;
    if (serverDefaultProvider === null) return; // 等待服务器默认 Provider 加载完成

    // 检查服务器默认 Provider 是否在已配置列表中
    const serverDefaultInList = configuredProviders.find(
      (p) => p.key === serverDefaultProvider,
    );

    if (serverDefaultInList) {
      // 服务器默认 Provider 在列表中，使用它
      hasInitialized.current = true;
      if (providerType !== serverDefaultProvider) {
        setProviderType(serverDefaultProvider);
      }
    } else if (!selectedProvider) {
      // 服务器默认 Provider 不在列表中，使用第一个已配置的
      hasInitialized.current = true;
      setProviderType(configuredProviders[0].key);
    } else {
      hasInitialized.current = true;
    }
  }, [
    configuredProviders,
    selectedProvider,
    setProviderType,
    serverDefaultProvider,
    providerType,
  ]);

  // 当 Provider 切换或模型列表变化时，自动选择第一个模型
  useEffect(() => {
    // 如果模型列表不为空，且当前模型为空或不在列表中，选择第一个模型
    if (
      currentModels.length > 0 &&
      (!model || !currentModels.includes(model))
    ) {
      setModel(currentModels[0]);
    }
  }, [currentModels, model, setModel]);

  const selectedProviderLabel = selectedProvider?.label || providerType;

  return (
    <Navbar>
      <div className="flex items-center gap-2">
        {/* History Toggle (Left) */}
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8 text-muted-foreground"
          onClick={onToggleHistory}
        >
          <Box size={18} />
        </Button>
      </div>

      {/* Center: Model Selector */}
      <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2">
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="ghost"
              role="combobox"
              aria-expanded={open}
              className="h-9 px-3 gap-2 font-normal hover:bg-muted text-foreground"
            >
              <Bot size={16} className="text-primary" />
              <span className="font-medium">{selectedProviderLabel}</span>
              <span className="text-muted-foreground">/</span>
              <span className="text-sm">{model || "Select Model"}</span>
              <ChevronDown className="ml-1 h-3 w-3 text-muted-foreground opacity-50" />
            </Button>
          </PopoverTrigger>
          <PopoverContent
            className="w-[420px] p-0 bg-background/95 backdrop-blur-sm border-border shadow-lg"
            align="center"
          >
            {/* Provider/Model Selection */}
            <div className="flex h-[300px]">
              {/* Left Column: Providers (只显示已配置的) */}
              <div className="w-[140px] border-r bg-muted/30 p-2 flex flex-col gap-1 overflow-y-auto">
                <div className="text-xs font-semibold text-muted-foreground px-2 py-1.5 mb-1">
                  Providers
                </div>
                {configuredProviders.length === 0 ? (
                  <div className="text-xs text-muted-foreground p-2">
                    暂无已配置的 Provider
                  </div>
                ) : (
                  configuredProviders.map((provider) => {
                    // 判断是否是服务器默认 Provider
                    const isServerDefault =
                      serverDefaultProvider === provider.key;
                    const isSelected = providerType === provider.key;

                    return (
                      <button
                        key={provider.key}
                        onClick={() => {
                          setProviderType(provider.key);
                        }}
                        className={cn(
                          "flex items-center justify-between w-full px-2 py-1.5 text-sm rounded-md transition-colors text-left",
                          isSelected
                            ? "bg-primary/10 text-primary font-medium"
                            : isServerDefault
                              ? "hover:bg-muted text-foreground hover:text-foreground"
                              : "hover:bg-muted text-muted-foreground/50 hover:text-muted-foreground",
                        )}
                      >
                        {provider.label}
                        {isSelected && (
                          <div className="w-1 h-1 rounded-full bg-primary" />
                        )}
                      </button>
                    );
                  })
                )}
              </div>

              {/* Right Column: Models */}
              <div className="flex-1 p-2 flex flex-col overflow-hidden">
                <div className="text-xs font-semibold text-muted-foreground px-2 py-1.5 mb-1">
                  Models
                </div>
                <ScrollArea className="flex-1">
                  <div className="space-y-1 p-1">
                    {currentModels.length === 0 ? (
                      <div className="text-xs text-muted-foreground p-2">
                        No models available
                      </div>
                    ) : (
                      currentModels.map((m) => (
                        <button
                          key={m}
                          onClick={() => {
                            setModel(m);
                            setOpen(false);
                          }}
                          className={cn(
                            "flex items-center justify-between w-full px-2 py-1.5 text-sm rounded-md transition-colors text-left group",
                            model === m
                              ? "bg-accent text-accent-foreground"
                              : "hover:bg-muted text-muted-foreground hover:text-foreground",
                          )}
                        >
                          {m}
                          {model === m && (
                            <Check size={14} className="text-primary" />
                          )}
                        </button>
                      ))
                    )}
                  </div>
                </ScrollArea>
              </div>
            </div>
          </PopoverContent>
        </Popover>
      </div>

      {/* Right: Status & Settings */}
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8 text-muted-foreground"
          onClick={onToggleSettings}
        >
          <Settings2 size={18} />
        </Button>
      </div>
    </Navbar>
  );
};
