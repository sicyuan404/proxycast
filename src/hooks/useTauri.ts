// 使用共享的 safeInvoke
import { safeInvoke } from "@/lib/dev-bridge";

export interface ServerStatus {
  running: boolean;
  host: string;
  port: number;
  requests: number;
  uptime_secs: number;
}

// TLS Configuration
export interface TlsConfig {
  enable: boolean;
  cert_path: string | null;
  key_path: string | null;
}

// Remote Management Configuration
export interface RemoteManagementConfig {
  allow_remote: boolean;
  secret_key: string | null;
  disable_control_panel: boolean;
}

// Quota Exceeded Configuration
export interface QuotaExceededConfig {
  switch_project: boolean;
  switch_preview_model: boolean;
  cooldown_seconds: number;
}

// Amp Model Mapping
export interface AmpModelMapping {
  from: string;
  to: string;
}

// Amp CLI Configuration
export interface AmpConfig {
  upstream_url: string | null;
  model_mappings: AmpModelMapping[];
  restrict_management_to_localhost: boolean;
}

// Gemini API Key Entry
export interface GeminiApiKeyEntry {
  id: string;
  api_key: string;
  base_url: string | null;
  proxy_url: string | null;
  excluded_models: string[];
  disabled: boolean;
}

// Vertex Model Alias
export interface VertexModelAlias {
  name: string;
  alias: string;
}

// Vertex API Key Entry
export interface VertexApiKeyEntry {
  id: string;
  api_key: string;
  base_url: string | null;
  models: VertexModelAlias[];
  proxy_url: string | null;
  disabled: boolean;
}

// iFlow Credential Entry
export interface IFlowCredentialEntry {
  id: string;
  token_file: string | null;
  auth_type: string;
  cookies: string | null;
  proxy_url: string | null;
  disabled: boolean;
}

// Credential Entry (OAuth)
export interface CredentialEntry {
  id: string;
  token_file: string;
  disabled: boolean;
  proxy_url: string | null;
}

// Credential Pool Configuration
export interface CredentialPoolConfig {
  kiro: CredentialEntry[];
  gemini: CredentialEntry[];
  qwen: CredentialEntry[];
  openai: ApiKeyEntry[];
  claude: ApiKeyEntry[];
  gemini_api_keys: GeminiApiKeyEntry[];
  vertex_api_keys: VertexApiKeyEntry[];
  codex: CredentialEntry[];
  iflow: IFlowCredentialEntry[];
}

// API Key Entry
export interface ApiKeyEntry {
  id: string;
  api_key: string;
  base_url: string | null;
  disabled: boolean;
  proxy_url: string | null;
}

// ============ 实验室功能配置 ============

/**
 * 截图对话功能配置
 */
export interface ScreenshotChatConfig {
  /** 是否启用截图对话功能 */
  enabled: boolean;
  /** 触发截图的全局快捷键 */
  shortcut: string;
}

/**
 * 实验室功能配置
 */
export interface ExperimentalFeatures {
  /** 截图对话功能配置 */
  screenshot_chat: ScreenshotChatConfig;
}

export interface Config {
  server: {
    host: string;
    port: number;
    api_key: string;
    tls: TlsConfig;
  };
  providers: {
    kiro: {
      enabled: boolean;
      credentials_path: string | null;
      region: string | null;
    };
    gemini: {
      enabled: boolean;
      credentials_path: string | null;
    };
    qwen: {
      enabled: boolean;
      credentials_path: string | null;
    };
    openai: {
      enabled: boolean;
      api_key: string | null;
      base_url: string | null;
    };
    claude: {
      enabled: boolean;
      api_key: string | null;
      base_url: string | null;
    };
  };
  default_provider: string;
  remote_management: RemoteManagementConfig;
  quota_exceeded: QuotaExceededConfig;
  ampcode: AmpConfig;
  credential_pool: CredentialPoolConfig;
  proxy_url: string | null;
  /** 关闭时最小化到托盘（而不是退出应用） */
  minimize_to_tray: boolean;
  /** 用户界面语言 ("zh" 或 "en") */
  language: string;
  /** 实验室功能配置 */
  experimental?: ExperimentalFeatures;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

export async function startServer(): Promise<string> {
  return safeInvoke("start_server");
}

export async function stopServer(): Promise<string> {
  return safeInvoke("stop_server");
}

export async function getServerStatus(): Promise<ServerStatus> {
  return safeInvoke("get_server_status");
}

export async function getConfig(): Promise<Config> {
  return safeInvoke("get_config");
}

export async function saveConfig(config: Config): Promise<void> {
  return safeInvoke("save_config", { config });
}

export async function getDefaultProvider(): Promise<string> {
  return safeInvoke("get_default_provider");
}

export async function setDefaultProvider(provider: string): Promise<string> {
  return safeInvoke("set_default_provider", { provider });
}

export async function refreshKiroToken(): Promise<string> {
  return safeInvoke("refresh_kiro_token");
}

export async function reloadCredentials(): Promise<string> {
  return safeInvoke("reload_credentials");
}

export async function getLogs(): Promise<LogEntry[]> {
  try {
    return await safeInvoke("get_logs");
  } catch {
    return [];
  }
}

export async function clearLogs(): Promise<void> {
  try {
    await safeInvoke("clear_logs");
  } catch {
    // ignore
  }
}

export interface TestResult {
  success: boolean;
  status: number;
  body: string;
  time_ms: number;
}

export async function testApi(
  method: string,
  path: string,
  body: string | null,
  auth: boolean,
): Promise<TestResult> {
  return safeInvoke("test_api", { method, path, body, auth });
}

export interface KiroCredentialStatus {
  loaded: boolean;
  has_access_token: boolean;
  has_refresh_token: boolean;
  region: string | null;
  auth_method: string | null;
  expires_at: string | null;
  creds_path: string;
}

export async function getKiroCredentials(): Promise<KiroCredentialStatus> {
  return safeInvoke("get_kiro_credentials");
}

export interface EnvVariable {
  key: string;
  value: string;
  masked: string;
}

export async function getEnvVariables(): Promise<EnvVariable[]> {
  return safeInvoke("get_env_variables");
}

export async function getTokenFileHash(): Promise<string> {
  return safeInvoke("get_token_file_hash");
}

export interface CheckResult {
  changed: boolean;
  new_hash: string;
  reloaded: boolean;
}

export async function checkAndReloadCredentials(
  lastHash: string,
): Promise<CheckResult> {
  return safeInvoke("check_and_reload_credentials", { last_hash: lastHash });
}

// ============ Gemini Provider ============

export interface GeminiCredentialStatus {
  loaded: boolean;
  has_access_token: boolean;
  has_refresh_token: boolean;
  expiry_date: number | null;
  is_valid: boolean;
  creds_path: string;
}

export async function getGeminiCredentials(): Promise<GeminiCredentialStatus> {
  return safeInvoke("get_gemini_credentials");
}

export async function reloadGeminiCredentials(): Promise<string> {
  return safeInvoke("reload_gemini_credentials");
}

export async function refreshGeminiToken(): Promise<string> {
  return safeInvoke("refresh_gemini_token");
}

export async function getGeminiEnvVariables(): Promise<EnvVariable[]> {
  return safeInvoke("get_gemini_env_variables");
}

export async function getGeminiTokenFileHash(): Promise<string> {
  return safeInvoke("get_gemini_token_file_hash");
}

export async function checkAndReloadGeminiCredentials(
  lastHash: string,
): Promise<CheckResult> {
  return safeInvoke("check_and_reload_gemini_credentials", {
    last_hash: lastHash,
  });
}

// ============ Qwen Provider ============

export interface QwenCredentialStatus {
  loaded: boolean;
  has_access_token: boolean;
  has_refresh_token: boolean;
  expiry_date: number | null;
  is_valid: boolean;
  creds_path: string;
}

export async function getQwenCredentials(): Promise<QwenCredentialStatus> {
  return safeInvoke("get_qwen_credentials");
}

export async function reloadQwenCredentials(): Promise<string> {
  return safeInvoke("reload_qwen_credentials");
}

export async function refreshQwenToken(): Promise<string> {
  return safeInvoke("refresh_qwen_token");
}

export async function getQwenEnvVariables(): Promise<EnvVariable[]> {
  return safeInvoke("get_qwen_env_variables");
}

export async function getQwenTokenFileHash(): Promise<string> {
  return safeInvoke("get_qwen_token_file_hash");
}

export async function checkAndReloadQwenCredentials(
  lastHash: string,
): Promise<CheckResult> {
  return safeInvoke("check_and_reload_qwen_credentials", {
    last_hash: lastHash,
  });
}

// ============ OpenAI Custom Provider ============

export interface OpenAICustomStatus {
  enabled: boolean;
  has_api_key: boolean;
  base_url: string;
}

export async function getOpenAICustomStatus(): Promise<OpenAICustomStatus> {
  return safeInvoke("get_openai_custom_status");
}

export async function setOpenAICustomConfig(
  apiKey: string | null,
  baseUrl: string | null,
  enabled: boolean,
): Promise<string> {
  return safeInvoke("set_openai_custom_config", {
    api_key: apiKey,
    base_url: baseUrl,
    enabled,
  });
}

// ============ Claude Custom Provider ============

export interface ClaudeCustomStatus {
  enabled: boolean;
  has_api_key: boolean;
  base_url: string;
}

export async function getClaudeCustomStatus(): Promise<ClaudeCustomStatus> {
  return safeInvoke("get_claude_custom_status");
}

export async function setClaudeCustomConfig(
  apiKey: string | null,
  baseUrl: string | null,
  enabled: boolean,
): Promise<string> {
  return safeInvoke("set_claude_custom_config", {
    api_key: apiKey,
    base_url: baseUrl,
    enabled,
  });
}

// ============ Models ============

export interface ModelInfo {
  id: string;
  object: string;
  owned_by: string;
}

export async function getAvailableModels(): Promise<ModelInfo[]> {
  return safeInvoke("get_available_models");
}

// ============ API Compatibility Check ============

export interface ApiCheckResult {
  model: string;
  available: boolean;
  status: number;
  error_type: string | null;
  error_message: string | null;
  time_ms: number;
}

export interface ApiCompatibilityResult {
  provider: string;
  overall_status: string;
  checked_at: string;
  results: ApiCheckResult[];
  warnings: string[];
}

export async function checkApiCompatibility(
  provider: string,
): Promise<ApiCompatibilityResult> {
  return safeInvoke("check_api_compatibility", { provider });
}

// ============ Endpoint Provider Configuration ============

/**
 * 端点 Provider 配置
 * 为不同客户端类型配置不同的 LLM Provider
 */
export interface EndpointProvidersConfig {
  /** Cursor 客户端使用的 Provider */
  cursor?: string | null;
  /** Claude Code 客户端使用的 Provider */
  claude_code?: string | null;
  /** Codex 客户端使用的 Provider */
  codex?: string | null;
  /** Windsurf 客户端使用的 Provider */
  windsurf?: string | null;
  /** Kiro 客户端使用的 Provider */
  kiro?: string | null;
  /** 其他客户端使用的 Provider */
  other?: string | null;
}

/**
 * 获取端点 Provider 配置
 * @returns 端点 Provider 配置对象
 */
export async function getEndpointProviders(): Promise<EndpointProvidersConfig> {
  return safeInvoke("get_endpoint_providers");
}

/**
 * 设置端点 Provider 配置
 * @param clientType 客户端类型 (cursor, claude_code, codex, windsurf, kiro, other)
 * @param provider Provider 名称，传 null 表示使用默认 Provider
 * @returns 设置后的 Provider 名称
 */
export async function setEndpointProvider(
  clientType: string,
  provider: string | null,
): Promise<string> {
  return safeInvoke("set_endpoint_provider", {
    endpoint: clientType,
    provider,
  });
}

// Network Info
export interface NetworkInfo {
  localhost: string;
  lan_ip: string | null;
  all_ips: string[];
}

/**
 * 获取本地网络信息
 * @returns 本地和内网 IP 地址
 */
export async function getNetworkInfo(): Promise<NetworkInfo> {
  return safeInvoke("get_network_info");
}

// ============ 实验室功能 API ============

/**
 * 获取实验室功能配置
 * @returns 实验室功能配置对象
 */
export async function getExperimentalConfig(): Promise<ExperimentalFeatures> {
  return safeInvoke("get_experimental_config");
}

/**
 * 保存实验室功能配置
 * @param config 实验室功能配置对象
 */
export async function saveExperimentalConfig(
  config: ExperimentalFeatures,
): Promise<void> {
  return safeInvoke("save_experimental_config", {
    experimentalConfig: config,
  });
}

/**
 * 验证快捷键格式
 * @param shortcut 快捷键字符串
 * @returns 是否有效
 */
export async function validateShortcut(shortcut: string): Promise<boolean> {
  return safeInvoke("validate_shortcut", { shortcutStr: shortcut });
}

/**
 * 更新截图快捷键
 * @param shortcut 新的快捷键字符串
 */
export async function updateScreenshotShortcut(
  shortcut: string,
): Promise<void> {
  return safeInvoke("update_screenshot_shortcut", { newShortcut: shortcut });
}
