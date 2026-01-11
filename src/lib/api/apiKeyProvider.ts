/**
 * @file API Key Provider API 模块
 * @description 封装 API Key Provider 相关的 Tauri 命令调用
 * @module lib/api/apiKeyProvider
 *
 * **Feature: provider-ui-refactor**
 * **Validates: Requirements 9.1**
 */

import { safeInvoke } from "@/lib/dev-bridge";

// ============================================================================
// 请求类型
// ============================================================================

/**
 * 添加自定义 Provider 请求
 */
export interface AddCustomProviderRequest {
  name: string;
  type: string;
  api_host: string;
  api_version?: string;
  project?: string;
  location?: string;
  region?: string;
}

/**
 * 更新 Provider 请求
 */
export interface UpdateProviderRequest {
  name?: string;
  api_host?: string;
  enabled?: boolean;
  sort_order?: number;
  api_version?: string;
  project?: string;
  location?: string;
  region?: string;
}

/**
 * 添加 API Key 请求
 */
export interface AddApiKeyRequest {
  provider_id: string;
  api_key: string;
  alias?: string;
}

// ============================================================================
// 响应类型
// ============================================================================

/**
 * Provider 显示数据（用于前端）
 */
export interface ProviderDisplay {
  id: string;
  name: string;
  type: string;
  api_host: string;
  is_system: boolean;
  group: string;
  enabled: boolean;
  sort_order: number;
  api_version?: string;
  project?: string;
  location?: string;
  region?: string;
  api_key_count: number;
  created_at: string;
  updated_at: string;
}

/**
 * API Key 显示数据（用于前端，掩码显示）
 */
export interface ApiKeyDisplay {
  id: string;
  provider_id: string;
  /** 掩码后的 API Key */
  api_key_masked: string;
  alias?: string;
  enabled: boolean;
  usage_count: number;
  error_count: number;
  last_used_at?: string;
  created_at: string;
}

/**
 * Provider 完整显示数据（包含 API Keys）
 */
export interface ProviderWithKeysDisplay extends ProviderDisplay {
  api_keys: ApiKeyDisplay[];
}

/**
 * 导入结果
 */
export interface ImportResult {
  success: boolean;
  imported_providers: number;
  imported_api_keys: number;
  skipped_providers: number;
  errors: string[];
}

// ============================================================================
// API 函数
// ============================================================================

/**
 * API Key Provider API 封装
 */
export const apiKeyProviderApi = {
  /**
   * 获取所有 API Key Provider（包含 API Keys）
   */
  async getProviders(): Promise<ProviderWithKeysDisplay[]> {
    return safeInvoke("get_api_key_providers");
  },

  /**
   * 获取单个 API Key Provider（包含 API Keys）
   */
  async getProvider(id: string): Promise<ProviderWithKeysDisplay | null> {
    return safeInvoke("get_api_key_provider", { id });
  },

  /**
   * 添加自定义 Provider
   */
  async addCustomProvider(
    request: AddCustomProviderRequest,
  ): Promise<ProviderDisplay> {
    return safeInvoke("add_custom_api_key_provider", { request });
  },

  /**
   * 更新 Provider 配置
   */
  async updateProvider(
    id: string,
    request: UpdateProviderRequest,
  ): Promise<ProviderDisplay> {
    return safeInvoke("update_api_key_provider", { id, request });
  },

  /**
   * 删除自定义 Provider
   */
  async deleteCustomProvider(id: string): Promise<boolean> {
    return safeInvoke("delete_custom_api_key_provider", { id });
  },

  /**
   * 添加 API Key
   */
  async addApiKey(request: AddApiKeyRequest): Promise<ApiKeyDisplay> {
    return safeInvoke("add_api_key", { request });
  },

  /**
   * 删除 API Key
   */
  async deleteApiKey(keyId: string): Promise<boolean> {
    return safeInvoke("delete_api_key", { keyId });
  },

  /**
   * 切换 API Key 启用状态
   */
  async toggleApiKey(keyId: string, enabled: boolean): Promise<ApiKeyDisplay> {
    return safeInvoke("toggle_api_key", { keyId, enabled });
  },

  /**
   * 更新 API Key 别名
   */
  async updateApiKeyAlias(
    keyId: string,
    alias?: string,
  ): Promise<ApiKeyDisplay> {
    return safeInvoke("update_api_key_alias", { keyId, alias });
  },

  /**
   * 获取下一个可用的 API Key（用于 API 调用）
   */
  async getNextApiKey(providerId: string): Promise<string | null> {
    return safeInvoke("get_next_api_key", { providerId });
  },

  /**
   * 记录 API Key 使用
   */
  async recordUsage(keyId: string): Promise<void> {
    return safeInvoke("record_api_key_usage", { keyId });
  },

  /**
   * 记录 API Key 错误
   */
  async recordError(keyId: string): Promise<void> {
    return safeInvoke("record_api_key_error", { keyId });
  },

  /**
   * 获取 UI 状态
   */
  async getUiState(key: string): Promise<string | null> {
    return safeInvoke("get_provider_ui_state", { key });
  },

  /**
   * 设置 UI 状态
   */
  async setUiState(key: string, value: string): Promise<void> {
    return safeInvoke("set_provider_ui_state", { key, value });
  },

  /**
   * 批量更新 Provider 排序顺序
   * **Validates: Requirements 8.4**
   */
  async updateSortOrders(sortOrders: [string, number][]): Promise<void> {
    return safeInvoke("update_provider_sort_orders", { sortOrders });
  },

  /**
   * 导出 Provider 配置
   */
  async exportConfig(includeKeys: boolean): Promise<string> {
    return safeInvoke("export_api_key_providers", { includeKeys });
  },

  /**
   * 导入 Provider 配置
   */
  async importConfig(configJson: string): Promise<ImportResult> {
    return safeInvoke("import_api_key_providers", { configJson });
  },

  // ============================================================================
  // 旧凭证迁移 API
  // ============================================================================

  /**
   * 获取需要迁移的旧 API Key 凭证列表
   */
  async getLegacyApiKeyCredentials(): Promise<LegacyApiKeyCredential[]> {
    return safeInvoke("get_legacy_api_key_credentials");
  },

  /**
   * 迁移旧的 API Key 凭证到新的 API Key Provider 系统
   * @param deleteAfterMigration 迁移后是否删除旧凭证
   */
  async migrateLegacyCredentials(
    deleteAfterMigration: boolean,
  ): Promise<LegacyMigrationResult> {
    return safeInvoke("migrate_legacy_api_key_credentials", {
      deleteAfterMigration,
    });
  },

  /**
   * 删除单个旧的 API Key 凭证
   */
  async deleteLegacyCredential(uuid: string): Promise<boolean> {
    return safeInvoke("delete_legacy_api_key_credential", { uuid });
  },
};

/**
 * 旧的 API Key 凭证信息
 */
export interface LegacyApiKeyCredential {
  uuid: string;
  provider_type: string;
  name?: string;
  api_key_masked: string;
  base_url?: string;
  usage_count: number;
  error_count: number;
  created_at: string;
}

/**
 * 迁移结果
 */
export interface LegacyMigrationResult {
  migrated_count: number;
  skipped_count: number;
  deleted_count: number;
  errors: string[];
}
