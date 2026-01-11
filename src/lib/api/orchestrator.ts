/**
 * 模型编排器 API
 *
 * 提供 Mini/Pro/Max 服务等级的智能路由接口
 */

import { safeInvoke } from "@/lib/dev-bridge";

// ============================================================================
// 类型定义
// ============================================================================

/** 服务等级 */
export type ServiceTier = "mini" | "pro" | "max";

/** 任务类型 */
export type TaskHint =
  | "coding"
  | "writing"
  | "analysis"
  | "chat"
  | "translation"
  | "summarization"
  | "math"
  | "other";

/** 可用模型 */
export interface AvailableModel {
  /** 模型 ID */
  id: string;
  /** 显示名称 */
  display_name: string;
  /** Provider 类型 */
  provider_type: string;
  /** 模型家族 */
  family?: string;
  /** 凭证 ID */
  credential_id: string;
  /** 上下文长度 */
  context_length?: number;
  /** 是否支持视觉 */
  supports_vision: boolean;
  /** 是否支持工具 */
  supports_tools: boolean;
  /** 输入价格（每 1M tokens） */
  input_cost_per_million?: number;
  /** 输出价格（每 1M tokens） */
  output_cost_per_million?: number;
  /** 是否健康 */
  is_healthy: boolean;
  /** 当前负载 (0-100) */
  current_load?: number;
}

/** 模型池统计 */
export interface PoolStats {
  /** Mini 等级模型数 */
  mini_count: number;
  /** Pro 等级模型数 */
  pro_count: number;
  /** Max 等级模型数 */
  max_count: number;
  /** 总模型数 */
  total_count: number;
  /** 健康模型数 */
  healthy_count: number;
}

/** 选择结果 */
export interface SelectionResult {
  /** 选中的模型 */
  model: AvailableModel;
  /** 使用的策略 ID */
  strategy_id: string;
  /** 选择原因 */
  reason: string;
  /** 置信度 (0-100) */
  confidence: number;
  /** 服务等级 */
  tier: ServiceTier;
  /** 是否是降级选择 */
  is_fallback: boolean;
  /** 降级原因（如果是降级） */
  fallback_reason?: string;
}

/** 选择请求 */
export interface SelectionRequest {
  /** 服务等级 */
  tier: ServiceTier;
  /** 任务类型 */
  task_hint?: TaskHint;
  /** 是否需要视觉能力 */
  requires_vision?: boolean;
  /** 是否需要工具能力 */
  requires_tools?: boolean;
  /** 首选 Provider */
  preferred_provider?: string;
  /** 排除的模型 */
  excluded_models?: string[];
  /** 策略 ID */
  strategy_id?: string;
}

/** 凭证信息请求 */
export interface CredentialInfoRequest {
  /** 凭证 ID */
  id: string;
  /** Provider 类型 */
  provider_type: string;
  /** 支持的模型列表 */
  supported_models: string[];
  /** 是否健康 */
  is_healthy: boolean;
  /** 当前负载 */
  current_load?: number;
}

/** 策略信息 */
export interface StrategyInfo {
  /** 策略 ID */
  id: string;
  /** 显示名称 */
  display_name: string;
  /** 描述 */
  description: string;
}

/** 服务等级信息 */
export interface ServiceTierInfo {
  /** 等级 ID */
  id: string;
  /** 显示名称 */
  display_name: string;
  /** 描述 */
  description: string;
  /** 等级数值 */
  level: number;
}

/** 任务类型信息 */
export interface TaskHintInfo {
  /** 任务 ID */
  id: string;
  /** 显示名称 */
  display_name: string;
  /** 描述 */
  description: string;
}

/** 编排器配置 */
export interface OrchestratorConfig {
  /** 默认服务等级 */
  default_tier: ServiceTier;
  /** 是否启用自动降级 */
  auto_fallback: boolean;
  /** 降级策略 */
  fallback_policy: "next_tier" | "same_tier" | "none";
  /** 是否启用负载均衡 */
  load_balancing: boolean;
  /** 模型池刷新间隔（秒） */
  pool_refresh_interval: number;
}

// ============================================================================
// API 函数
// ============================================================================

export const orchestratorApi = {
  // ==================== 初始化 ====================

  /** 初始化编排器 */
  init: (): Promise<void> => safeInvoke("init_orchestrator"),

  /** 获取编排器配置 */
  getConfig: (): Promise<OrchestratorConfig> =>
    safeInvoke("get_orchestrator_config"),

  /** 更新编排器配置 */
  updateConfig: (config: OrchestratorConfig): Promise<void> =>
    safeInvoke("update_orchestrator_config", { config }),

  // ==================== 模型池 ====================

  /** 获取模型池统计 */
  getPoolStats: (): Promise<PoolStats> => safeInvoke("get_pool_stats"),

  /** 获取指定等级的模型列表 */
  getTierModels: (tier: ServiceTier): Promise<AvailableModel[]> =>
    safeInvoke("get_tier_models", { tier }),

  /** 获取所有可用模型 */
  getAllModels: (): Promise<AvailableModel[]> => safeInvoke("get_all_models"),

  // ==================== 凭证管理 ====================

  /** 更新凭证列表 */
  updateCredentials: (credentials: CredentialInfoRequest[]): Promise<void> =>
    safeInvoke("update_orchestrator_credentials", { credentials }),

  /** 添加凭证 */
  addCredential: (credential: CredentialInfoRequest): Promise<void> =>
    safeInvoke("add_orchestrator_credential", { credential }),

  /** 移除凭证 */
  removeCredential: (credentialId: string): Promise<void> =>
    safeInvoke("remove_orchestrator_credential", { credentialId }),

  /** 标记凭证为不健康 */
  markCredentialUnhealthy: (
    modelId: string,
    credentialId: string,
  ): Promise<void> =>
    safeInvoke("mark_credential_unhealthy", { modelId, credentialId }),

  /** 标记凭证为健康 */
  markCredentialHealthy: (credentialId: string): Promise<void> =>
    safeInvoke("mark_credential_healthy", { credentialId }),

  /** 更新凭证负载 */
  updateCredentialLoad: (credentialId: string, load: number): Promise<void> =>
    safeInvoke("update_credential_load", { credentialId, load }),

  // ==================== 模型选择 ====================

  /** 选择模型 */
  selectModel: (request: SelectionRequest): Promise<SelectionResult> =>
    safeInvoke("select_model", { request }),

  /** 快速选择模型（使用默认配置） */
  quickSelectModel: (): Promise<SelectionResult> =>
    safeInvoke("quick_select_model"),

  /** 为特定任务选择模型 */
  selectModelForTask: (
    tier: ServiceTier,
    task: TaskHint,
  ): Promise<SelectionResult> =>
    safeInvoke("select_model_for_task", { tier, task }),

  // ==================== 策略 ====================

  /** 列出所有可用策略 */
  listStrategies: (): Promise<StrategyInfo[]> => safeInvoke("list_strategies"),

  /** 获取服务等级列表 */
  listServiceTiers: (): Promise<ServiceTierInfo[]> =>
    safeInvoke("list_service_tiers"),

  /** 获取任务类型列表 */
  listTaskHints: (): Promise<TaskHintInfo[]> => safeInvoke("list_task_hints"),
};

// ============================================================================
// React Hooks
// ============================================================================

import { useState, useEffect, useCallback } from "react";

/** 使用编排器状态 */
export function useOrchestrator() {
  const [initialized, setInitialized] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [poolStats, setPoolStats] = useState<PoolStats | null>(null);
  const [config, setConfig] = useState<OrchestratorConfig | null>(null);

  // 初始化
  useEffect(() => {
    const init = async () => {
      try {
        await orchestratorApi.init();
        setInitialized(true);

        const [stats, cfg] = await Promise.all([
          orchestratorApi.getPoolStats(),
          orchestratorApi.getConfig(),
        ]);

        setPoolStats(stats);
        setConfig(cfg);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    };

    init();
  }, []);

  // 刷新统计
  const refreshStats = useCallback(async () => {
    try {
      const stats = await orchestratorApi.getPoolStats();
      setPoolStats(stats);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // 更新配置
  const updateConfig = useCallback(async (newConfig: OrchestratorConfig) => {
    try {
      await orchestratorApi.updateConfig(newConfig);
      setConfig(newConfig);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  return {
    initialized,
    loading,
    error,
    poolStats,
    config,
    refreshStats,
    updateConfig,
  };
}

/** 使用模型选择 */
export function useModelSelection(defaultTier: ServiceTier = "pro") {
  const [tier, setTier] = useState<ServiceTier>(defaultTier);
  const [models, setModels] = useState<AvailableModel[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 加载模型列表
  const loadModels = useCallback(async (selectedTier: ServiceTier) => {
    setLoading(true);
    setError(null);
    try {
      const result = await orchestratorApi.getTierModels(selectedTier);
      setModels(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  // 切换等级时加载模型
  useEffect(() => {
    loadModels(tier);
  }, [tier, loadModels]);

  // 选择模型
  const selectModel = useCallback(
    async (request?: Partial<SelectionRequest>) => {
      try {
        return await orchestratorApi.selectModel({
          tier,
          ...request,
        });
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        throw err;
      }
    },
    [tier],
  );

  return {
    tier,
    setTier,
    models,
    loading,
    error,
    selectModel,
    refreshModels: () => loadModels(tier),
  };
}
