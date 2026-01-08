/**
 * @file TerminalAIModeSelector.tsx
 * @description Terminal AI 模式选择器 - 复用 Agent 的模型选择逻辑
 * @module components/terminal/ai/TerminalAIModeSelector
 *
 * 参考 Waveterm 的 AIModeDropdown 设计，但复用 ProxyCast 的 Provider/Model 选择器
 */

import React, { useState, useMemo, useEffect } from "react";
import { ChevronDown, Check } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useProviderPool } from "@/hooks/useProviderPool";
import { useApiKeyProvider } from "@/hooks/useApiKeyProvider";
import { useModelRegistry } from "@/hooks/useModelRegistry";
import { getProviderAliasConfig } from "@/lib/api/modelRegistry";
import type { ProviderAliasConfig } from "@/lib/types/modelRegistry";

// ============================================================================
// 常量
// ============================================================================

/** Provider type 到 registry ID 的映射 */
const getRegistryIdFromType = (providerType: string): string => {
  const typeMap: Record<string, string> = {
    openai: "openai",
    anthropic: "anthropic",
    gemini: "google",
    kiro: "kiro",
    claude: "anthropic",
    claude_oauth: "anthropic",
    qwen: "alibaba",
    codex: "openai",
    antigravity: "antigravity",
    iflow: "openai",
    gemini_api_key: "google",
  };
  return typeMap[providerType.toLowerCase()] || providerType.toLowerCase();
};

/** 需要使用别名配置的 Provider */
const ALIAS_PROVIDERS = ["antigravity", "kiro"];

/** Provider 显示名称 */
const getProviderLabel = (providerType: string): string => {
  const labelMap: Record<string, string> = {
    kiro: "Kiro",
    gemini: "Gemini OAuth",
    qwen: "通义千问",
    antigravity: "Antigravity",
    codex: "Codex",
    claude_oauth: "Claude OAuth",
    claude: "Claude",
    openai: "OpenAI",
    anthropic: "Anthropic",
    gemini_api_key: "Gemini API Key",
    iflow: "iFlow",
  };
  return (
    labelMap[providerType.toLowerCase()] ||
    providerType.charAt(0).toUpperCase() + providerType.slice(1)
  );
};

// ============================================================================
// 类型
// ============================================================================

interface ConfiguredProvider {
  key: string;
  label: string;
  registryId: string;
  fallbackRegistryId?: string;
  type: string;
}

interface TerminalAIModeSelectorProps {
  /** 当前 Provider ID */
  providerId: string;
  /** Provider 变化回调 */
  onProviderChange: (id: string) => void;
  /** 当前模型 ID */
  modelId: string;
  /** 模型变化回调 */
  onModelChange: (id: string) => void;
  /** 自定义类名 */
  className?: string;
}

// ============================================================================
// 组件
// ============================================================================

export const TerminalAIModeSelector: React.FC<TerminalAIModeSelectorProps> = ({
  providerId,
  onProviderChange,
  modelId,
  onModelChange,
  className,
}) => {
  const [open, setOpen] = useState(false);
  const [aliasConfig, setAliasConfig] = useState<ProviderAliasConfig | null>(
    null,
  );

  // 获取凭证数据
  const { overview: oauthCredentials } = useProviderPool();
  const { providers: apiKeyProviders } = useApiKeyProvider();
  const { models: registryModels } = useModelRegistry({ autoLoad: true });

  // 计算已配置的 Provider 列表
  const configuredProviders = useMemo(() => {
    const providerMap = new Map<string, ConfiguredProvider>();

    // OAuth 凭证
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

    // API Key Provider
    apiKeyProviders
      .filter((p) => p.api_key_count > 0 && p.enabled)
      .forEach((provider) => {
        let key = provider.id;
        let label = provider.name;

        if (providerMap.has(key)) {
          key = `${provider.id}_api_key`;
          label = `${provider.name} API Key`;
        }

        if (!providerMap.has(key)) {
          providerMap.set(key, {
            key,
            label,
            registryId: provider.id,
            fallbackRegistryId: getRegistryIdFromType(provider.type),
            type: provider.type,
          });
        }
      });

    return Array.from(providerMap.values());
  }, [oauthCredentials, apiKeyProviders]);

  // 当前选中的 Provider
  const selectedProvider = useMemo(() => {
    return configuredProviders.find((p) => p.key === providerId);
  }, [configuredProviders, providerId]);

  // 加载别名配置
  useEffect(() => {
    if (selectedProvider && ALIAS_PROVIDERS.includes(selectedProvider.key)) {
      getProviderAliasConfig(selectedProvider.key)
        .then(setAliasConfig)
        .catch(() => setAliasConfig(null));
    } else {
      setAliasConfig(null);
    }
  }, [selectedProvider]);

  // 当前 Provider 的模型列表
  const currentModels = useMemo(() => {
    if (!selectedProvider) return [];

    // 别名 Provider 使用别名配置
    if (ALIAS_PROVIDERS.includes(selectedProvider.key) && aliasConfig) {
      return aliasConfig.models;
    }

    // 从 model_registry 获取
    let models = registryModels
      .filter((m) => m.provider_id === selectedProvider.registryId)
      .map((m) => m.id);

    if (models.length === 0 && selectedProvider.fallbackRegistryId) {
      models = registryModels
        .filter((m) => m.provider_id === selectedProvider.fallbackRegistryId)
        .map((m) => m.id);
    }

    // 排序
    return models.sort((a, b) => {
      const aIsLatest = a.includes("-latest");
      const bIsLatest = b.includes("-latest");

      if (aIsLatest && !bIsLatest) return -1;
      if (!aIsLatest && bIsLatest) return 1;

      const dateRegex = /-(\d{8})$/;
      const aMatch = a.match(dateRegex);
      const bMatch = b.match(dateRegex);

      if (aMatch && bMatch) {
        return bMatch[1].localeCompare(aMatch[1]);
      }

      if (aMatch && !bMatch) return -1;
      if (!aMatch && bMatch) return 1;

      return b.localeCompare(a);
    });
  }, [selectedProvider, registryModels, aliasConfig]);

  // 自动选择第一个模型
  useEffect(() => {
    if (
      selectedProvider &&
      ALIAS_PROVIDERS.includes(selectedProvider.key) &&
      !aliasConfig
    ) {
      return;
    }

    if (currentModels.length > 0 && !currentModels.includes(modelId)) {
      onModelChange(currentModels[0]);
    }
  }, [currentModels, modelId, onModelChange, selectedProvider, aliasConfig]);

  // 初始化 Provider
  useEffect(() => {
    if (configuredProviders.length > 0 && !selectedProvider) {
      onProviderChange(configuredProviders[0].key);
    }
  }, [configuredProviders, selectedProvider, onProviderChange]);

  const displayLabel = selectedProvider?.label || providerId;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          className={cn(
            "flex items-center gap-1.5 px-2 py-1 rounded-md text-sm",
            "bg-zinc-800 hover:bg-zinc-700 text-zinc-200 transition-colors",
            className,
          )}
        >
          <span className="font-medium">{displayLabel}</span>
          <ChevronDown size={14} className="text-zinc-400" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        className="w-[380px] p-0 bg-zinc-900/95 backdrop-blur-sm border-zinc-700"
        align="start"
      >
        <div className="flex h-[280px]">
          {/* 左侧：Provider 列表 */}
          <div className="w-[130px] border-r border-zinc-700 bg-zinc-800/30 p-2 overflow-y-auto">
            <div className="text-xs font-semibold text-zinc-400 px-2 py-1 mb-1">
              Providers
            </div>
            {configuredProviders.length === 0 ? (
              <div className="text-xs text-zinc-500 p-2">
                暂无已配置的 Provider
              </div>
            ) : (
              configuredProviders.map((provider) => (
                <button
                  key={provider.key}
                  onClick={() => onProviderChange(provider.key)}
                  className={cn(
                    "flex items-center justify-between w-full px-2 py-1.5 text-sm rounded-md transition-colors text-left",
                    providerId === provider.key
                      ? "bg-blue-500/20 text-blue-400 font-medium"
                      : "hover:bg-zinc-700 text-zinc-400 hover:text-zinc-200",
                  )}
                >
                  {provider.label}
                  {providerId === provider.key && (
                    <div className="w-1.5 h-1.5 rounded-full bg-blue-400" />
                  )}
                </button>
              ))
            )}
          </div>

          {/* 右侧：模型列表 */}
          <div className="flex-1 p-2 flex flex-col overflow-hidden">
            <div className="text-xs font-semibold text-zinc-400 px-2 py-1 mb-1">
              Models
            </div>
            <ScrollArea className="flex-1">
              <div className="space-y-0.5 p-1">
                {currentModels.length === 0 ? (
                  <div className="text-xs text-zinc-500 p-2">暂无可用模型</div>
                ) : (
                  currentModels.map((m) => (
                    <button
                      key={m}
                      onClick={() => {
                        onModelChange(m);
                        setOpen(false);
                      }}
                      className={cn(
                        "flex items-center justify-between w-full px-2 py-1.5 text-sm rounded-md transition-colors text-left",
                        modelId === m
                          ? "bg-zinc-700 text-zinc-100"
                          : "hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200",
                      )}
                    >
                      <span className="truncate">{m}</span>
                      {modelId === m && (
                        <Check
                          size={14}
                          className="text-blue-400 flex-shrink-0"
                        />
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
  );
};
