import { invoke } from "@tauri-apps/api/core";

// Provider types supported by the pool
export type PoolProviderType =
  | "kiro"
  | "gemini"
  | "qwen"
  | "antigravity"
  | "openai"
  | "claude"
  | "codex"
  | "claude_oauth"
  | "iflow";

// Credential data types
export interface KiroOAuthCredential {
  type: "kiro_oauth";
  creds_file_path: string;
}

export interface GeminiOAuthCredential {
  type: "gemini_oauth";
  creds_file_path: string;
  project_id?: string;
}

export interface QwenOAuthCredential {
  type: "qwen_oauth";
  creds_file_path: string;
}

export interface AntigravityOAuthCredential {
  type: "antigravity_oauth";
  creds_file_path: string;
  project_id?: string;
}

export interface OpenAIKeyCredential {
  type: "openai_key";
  api_key: string;
  base_url?: string;
}

export interface ClaudeKeyCredential {
  type: "claude_key";
  api_key: string;
  base_url?: string;
}

export interface CodexOAuthCredential {
  type: "codex_oauth";
  creds_file_path: string;
}

export interface ClaudeOAuthCredential {
  type: "claude_oauth";
  creds_file_path: string;
}

export interface IFlowOAuthCredential {
  type: "iflow_oauth";
  creds_file_path: string;
}

export interface IFlowCookieCredential {
  type: "iflow_cookie";
  creds_file_path: string;
}

export type CredentialData =
  | KiroOAuthCredential
  | GeminiOAuthCredential
  | QwenOAuthCredential
  | AntigravityOAuthCredential
  | OpenAIKeyCredential
  | ClaudeKeyCredential
  | CodexOAuthCredential
  | ClaudeOAuthCredential
  | IFlowOAuthCredential
  | IFlowCookieCredential;

// Provider credential
export interface ProviderCredential {
  uuid: string;
  provider_type: PoolProviderType;
  credential: CredentialData;
  name?: string;
  is_healthy: boolean;
  is_disabled: boolean;
  check_health: boolean;
  check_model_name?: string;
  not_supported_models: string[];
  usage_count: number;
  error_count: number;
  last_used?: string;
  last_error_time?: string;
  last_error_message?: string;
  last_health_check_time?: string;
  last_health_check_model?: string;
  created_at: string;
  updated_at: string;
}

// Credential source type
export type CredentialSource = "manual" | "imported" | "private";

// Credential display (for UI, hides sensitive data)
export interface CredentialDisplay {
  uuid: string;
  provider_type: PoolProviderType;
  credential_type: string;
  name?: string;
  display_credential: string;
  is_healthy: boolean;
  is_disabled: boolean;
  check_health: boolean;
  check_model_name?: string;
  not_supported_models: string[];
  usage_count: number;
  error_count: number;
  last_used?: string;
  last_error_time?: string;
  last_error_message?: string;
  last_health_check_time?: string;
  last_health_check_model?: string;
  oauth_status?: OAuthStatus;
  token_cache_status?: TokenCacheStatus;
  created_at: string;
  updated_at: string;
  // 凭证来源（手动添加/导入/私有）
  source: CredentialSource;
  // API Key 凭证的 base_url（仅用于 OpenAI/Claude API Key 类型）
  base_url?: string;
  // API Key 凭证的完整 api_key（仅用于 OpenAI/Claude API Key 类型，用于编辑）
  api_key?: string;
  // 凭证级代理 URL（可覆盖全局代理设置）
  proxy_url?: string;
}

// Pool statistics
export interface PoolStats {
  total: number;
  healthy: number;
  unhealthy: number;
  disabled: number;
  total_usage: number;
  total_errors: number;
}

// Provider pool overview
export interface ProviderPoolOverview {
  provider_type: string;
  stats: PoolStats;
  credentials: CredentialDisplay[];
}

// Health check result
export interface HealthCheckResult {
  uuid: string;
  success: boolean;
  model?: string;
  message?: string;
  duration_ms: number;
}

// OAuth status
export interface OAuthStatus {
  has_access_token: boolean;
  has_refresh_token: boolean;
  is_token_valid: boolean;
  expiry_info?: string;
  creds_path: string;
}

// Token cache status (from database cache)
export interface TokenCacheStatus {
  has_cached_token: boolean;
  is_valid: boolean;
  is_expiring_soon: boolean;
  expiry_time?: string;
  last_refresh?: string;
  refresh_error_count: number;
  last_refresh_error?: string;
}

// Request types
export interface AddCredentialRequest {
  provider_type: string;
  credential: CredentialData;
  name?: string;
  check_health?: boolean;
  check_model_name?: string;
}

export interface UpdateCredentialRequest {
  name?: string;
  is_disabled?: boolean;
  check_health?: boolean;
  check_model_name?: string;
  not_supported_models?: string[];
  /// 新的凭证文件路径（仅适用于OAuth凭证，用于重新上传文件）
  new_creds_file_path?: string;
  /// OAuth相关：新的project_id（仅适用于Gemini）
  new_project_id?: string;
  /// API Key 相关：新的 base_url（仅适用于 API Key 凭证）
  new_base_url?: string;
  /// API Key 相关：新的 api_key（仅适用于 API Key 凭证）
  new_api_key?: string;
  /// 新的代理 URL（可覆盖全局代理设置）
  new_proxy_url?: string;
}

export const providerPoolApi = {
  // Get overview of all provider pools
  async getOverview(): Promise<ProviderPoolOverview[]> {
    return invoke("get_provider_pool_overview");
  },

  // Get credentials for a specific provider type
  async getCredentials(
    providerType: PoolProviderType,
  ): Promise<CredentialDisplay[]> {
    return invoke("get_provider_pool_credentials", { providerType });
  },

  // Add a generic credential
  async addCredential(
    request: AddCredentialRequest,
  ): Promise<ProviderCredential> {
    return invoke("add_provider_pool_credential", { request });
  },

  // Update a credential
  async updateCredential(
    uuid: string,
    request: UpdateCredentialRequest,
  ): Promise<ProviderCredential> {
    return invoke("update_provider_pool_credential", { uuid, request });
  },

  // Delete a credential
  async deleteCredential(
    uuid: string,
    providerType?: PoolProviderType,
  ): Promise<boolean> {
    return invoke("delete_provider_pool_credential", { uuid, providerType });
  },

  // Toggle credential enabled/disabled
  async toggleCredential(
    uuid: string,
    isDisabled: boolean,
  ): Promise<ProviderCredential> {
    return invoke("toggle_provider_pool_credential", { uuid, isDisabled });
  },

  // Reset credential counters
  async resetCredential(uuid: string): Promise<void> {
    return invoke("reset_provider_pool_credential", { uuid });
  },

  // Reset health status for all credentials of a type
  async resetHealth(providerType: PoolProviderType): Promise<number> {
    return invoke("reset_provider_pool_health", { providerType });
  },

  // Check health of a single credential
  async checkCredentialHealth(uuid: string): Promise<HealthCheckResult> {
    return invoke("check_provider_pool_credential_health", { uuid });
  },

  // Check health of all credentials of a type
  async checkTypeHealth(
    providerType: PoolProviderType,
  ): Promise<HealthCheckResult[]> {
    return invoke("check_provider_pool_type_health", { providerType });
  },

  // Provider-specific add methods
  async addKiroOAuth(
    credsFilePath: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_kiro_oauth_credential", { credsFilePath, name });
  },

  async addGeminiOAuth(
    credsFilePath: string,
    projectId?: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_gemini_oauth_credential", {
      credsFilePath,
      projectId,
      name,
    });
  },

  async addQwenOAuth(
    credsFilePath: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_qwen_oauth_credential", { credsFilePath, name });
  },

  async addOpenAIKey(
    apiKey: string,
    baseUrl?: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_openai_key_credential", { apiKey, baseUrl, name });
  },

  async addClaudeKey(
    apiKey: string,
    baseUrl?: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_claude_key_credential", { apiKey, baseUrl, name });
  },

  async addAntigravityOAuth(
    credsFilePath: string,
    projectId?: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_antigravity_oauth_credential", {
      credsFilePath,
      projectId,
      name,
    });
  },

  async addCodexOAuth(
    credsFilePath: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_codex_oauth_credential", { credsFilePath, name });
  },

  async addClaudeOAuth(
    credsFilePath: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_claude_oauth_credential", { credsFilePath, name });
  },

  async addIFlowOAuth(
    credsFilePath: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_iflow_oauth_credential", { credsFilePath, name });
  },

  async addIFlowCookie(
    credsFilePath: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("add_iflow_cookie_credential", { credsFilePath, name });
  },

  // Antigravity OAuth 登录（打开浏览器授权）
  async startAntigravityOAuthLogin(
    name?: string,
    skipProjectIdFetch?: boolean,
  ): Promise<ProviderCredential> {
    return invoke("start_antigravity_oauth_login", {
      name,
      skipProjectIdFetch,
    });
  },

  // 获取 Antigravity OAuth 授权 URL 并等待回调（不自动打开浏览器）
  // 服务器会在后台等待回调，成功后返回凭证
  // 如果需要显示 URL，错误信息会包含 AUTH_URL: 前缀
  async getAntigravityAuthUrlAndWait(
    name?: string,
    skipProjectIdFetch?: boolean,
  ): Promise<ProviderCredential> {
    return invoke("get_antigravity_auth_url_and_wait", {
      name,
      skipProjectIdFetch,
    });
  },

  // Codex OAuth 登录（打开浏览器授权）
  async startCodexOAuthLogin(name?: string): Promise<ProviderCredential> {
    return invoke("start_codex_oauth_login", { name });
  },

  // 获取 Codex OAuth 授权 URL 并等待回调（不自动打开浏览器）
  // 服务器会在后台等待回调，成功后返回凭证
  async getCodexAuthUrlAndWait(name?: string): Promise<ProviderCredential> {
    return invoke("get_codex_auth_url_and_wait", { name });
  },

  // Claude OAuth 登录（打开浏览器授权）
  async startClaudeOAuthLogin(name?: string): Promise<ProviderCredential> {
    return invoke("start_claude_oauth_login", { name });
  },

  // 获取 Claude OAuth 授权 URL 并等待回调（不自动打开浏览器）
  // 服务器会在后台等待回调，成功后返回凭证
  async getClaudeOAuthAuthUrlAndWait(
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("get_claude_oauth_auth_url_and_wait", { name });
  },

  // Qwen Device Code Flow 登录（打开浏览器授权）
  async startQwenDeviceCodeLogin(name?: string): Promise<ProviderCredential> {
    return invoke("start_qwen_device_code_login", { name });
  },

  // 获取 Qwen Device Code 并等待用户授权（不自动打开浏览器）
  // 服务器会在后台轮询等待授权，成功后返回凭证
  async getQwenDeviceCodeAndWait(name?: string): Promise<ProviderCredential> {
    return invoke("get_qwen_device_code_and_wait", { name });
  },

  // iFlow OAuth 登录（打开浏览器授权）
  async startIFlowOAuthLogin(name?: string): Promise<ProviderCredential> {
    return invoke("start_iflow_oauth_login", { name });
  },

  // 获取 iFlow OAuth 授权 URL 并等待回调（不自动打开浏览器）
  // 服务器会在后台等待回调，成功后返回凭证
  async getIFlowAuthUrlAndWait(name?: string): Promise<ProviderCredential> {
    return invoke("get_iflow_auth_url_and_wait", { name });
  },

  // Gemini OAuth 登录（打开浏览器授权）
  async startGeminiOAuthLogin(name?: string): Promise<ProviderCredential> {
    return invoke("start_gemini_oauth_login", { name });
  },

  // 获取 Gemini OAuth 授权 URL 并等待回调（不自动打开浏览器）
  // 服务器会在后台等待回调，成功后返回凭证
  async getGeminiAuthUrlAndWait(name?: string): Promise<ProviderCredential> {
    return invoke("get_gemini_auth_url_and_wait", { name });
  },

  // 用 Gemini 授权码交换 token
  async exchangeGeminiCode(
    code: string,
    sessionId?: string,
    name?: string,
  ): Promise<ProviderCredential> {
    return invoke("exchange_gemini_code", { code, sessionId, name });
  },

  // OAuth token management
  async refreshCredentialToken(uuid: string): Promise<string> {
    return invoke("refresh_pool_credential_token", { uuid });
  },

  async getCredentialOAuthStatus(uuid: string): Promise<OAuthStatus> {
    return invoke("get_pool_credential_oauth_status", { uuid });
  },

  // Migration API
  async migratePrivateConfig(config: unknown): Promise<MigrationResult> {
    return invoke("migrate_private_config_to_pool", { config });
  },
};

// Migration result
export interface MigrationResult {
  migrated_count: number;
  skipped_count: number;
  errors: string[];
}

// Kiro 凭证指纹信息
export interface KiroFingerprintInfo {
  /** Machine ID（SHA256 哈希，64 字符） */
  machine_id: string;
  /** Machine ID 的短格式（前 16 字符） */
  machine_id_short: string;
  /** 指纹来源（profileArn / clientId / system） */
  source: string;
  /** 认证方式 */
  auth_method: string;
}

// 获取 Kiro 凭证的指纹信息
export async function getKiroCredentialFingerprint(
  uuid: string,
): Promise<KiroFingerprintInfo> {
  return invoke("get_kiro_credential_fingerprint", { uuid });
}
