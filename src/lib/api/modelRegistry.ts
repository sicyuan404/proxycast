/**
 * 模型注册表 API
 *
 * 提供与后端 ModelRegistryService 交互的 API
 */

import { safeInvoke } from "@/lib/dev-bridge";
import type {
  EnhancedModelMetadata,
  ModelSyncState,
  ModelTier,
  ProviderAliasConfig,
  UserModelPreference,
} from "@/lib/types/modelRegistry";

/**
 * 获取所有模型
 */
export async function getModelRegistry(): Promise<EnhancedModelMetadata[]> {
  return safeInvoke("get_model_registry");
}

/**
 * 刷新模型注册表（强制从内嵌资源重新加载）
 * @returns 加载的模型数量
 */
export async function refreshModelRegistry(): Promise<number> {
  return safeInvoke("refresh_model_registry");
}

/**
 * 搜索模型
 * @param query 搜索关键词
 * @param limit 返回数量限制
 */
export async function searchModels(
  query: string,
  limit?: number,
): Promise<EnhancedModelMetadata[]> {
  return safeInvoke("search_models", { query, limit });
}

/**
 * 获取用户模型偏好
 */
export async function getModelPreferences(): Promise<UserModelPreference[]> {
  return safeInvoke("get_model_preferences");
}

/**
 * 切换模型收藏状态
 * @param modelId 模型 ID
 * @returns 新的收藏状态
 */
export async function toggleModelFavorite(modelId: string): Promise<boolean> {
  return safeInvoke("toggle_model_favorite", { modelId });
}

/**
 * 隐藏模型
 * @param modelId 模型 ID
 */
export async function hideModel(modelId: string): Promise<void> {
  return safeInvoke("hide_model", { modelId });
}

/**
 * 记录模型使用
 * @param modelId 模型 ID
 */
export async function recordModelUsage(modelId: string): Promise<void> {
  return safeInvoke("record_model_usage", { modelId });
}

/**
 * 获取模型同步状态
 */
export async function getModelSyncState(): Promise<ModelSyncState> {
  return safeInvoke("get_model_sync_state");
}

/**
 * 按 Provider 获取模型
 * @param providerId Provider ID
 */
export async function getModelsForProvider(
  providerId: string,
): Promise<EnhancedModelMetadata[]> {
  return safeInvoke("get_models_for_provider", { providerId });
}

/**
 * 按服务等级获取模型
 * @param tier 服务等级
 */
export async function getModelsByTier(
  tier: ModelTier,
): Promise<EnhancedModelMetadata[]> {
  return safeInvoke("get_models_by_tier", { tier });
}

/**
 * 获取指定 Provider 的别名配置
 * 用于获取 Antigravity、Kiro 等中转服务的模型别名映射
 * @param provider Provider ID（如 "antigravity"、"kiro"）
 */
export async function getProviderAliasConfig(
  provider: string,
): Promise<ProviderAliasConfig | null> {
  return safeInvoke("get_provider_alias_config", { provider });
}

/**
 * 获取所有 Provider 的别名配置
 */
export async function getAllAliasConfigs(): Promise<
  Record<string, ProviderAliasConfig>
> {
  return safeInvoke("get_all_alias_configs");
}

/**
 * 模型注册表 API 对象
 */
export const modelRegistryApi = {
  getModelRegistry,
  refreshModelRegistry,
  searchModels,
  getModelPreferences,
  toggleModelFavorite,
  hideModel,
  recordModelUsage,
  getModelSyncState,
  getModelsForProvider,
  getModelsByTier,
  getProviderAliasConfig,
  getAllAliasConfigs,
};
