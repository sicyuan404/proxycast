/**
 * 凭证表单共享类型定义
 */

import { PoolProviderType } from "@/lib/api/providerPool";

/** OAuth 登录表单的通用 Props */
export interface OAuthLoginFormProps {
  name: string;
  loading: boolean;
  error: string | null;
  onSuccess: () => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

/** 文件导入表单的通用 Props */
export interface FileImportFormProps {
  credsFilePath: string;
  setCredsFilePath: (path: string) => void;
  projectId?: string;
  setProjectId?: (id: string) => void;
  onSelectFile: () => void;
  defaultPathHint?: string;
  fileHint?: string;
}

/** API Key 表单的通用 Props */
export interface ApiKeyFormProps {
  apiKey: string;
  setApiKey: (key: string) => void;
  baseUrl: string;
  setBaseUrl: (url: string) => void;
  providerType: PoolProviderType;
}

/** 默认凭证文件路径 */
export const defaultCredsPath: Record<string, string> = {
  kiro: "~/.aws/sso/cache/kiro-auth-token.json",
  gemini: "~/.gemini/oauth_creds.json",
  qwen: "~/.qwen/oauth_creds.json",
  antigravity: "",
  codex: "~/.codex/auth.json",
  claude_oauth: "~/.claude/oauth.json",
  iflow: "~/.iflow/oauth_creds.json",
};

/** Provider 显示名称 */
export const providerLabels: Record<PoolProviderType, string> = {
  kiro: "Kiro (AWS)",
  gemini: "Gemini (Google)",
  qwen: "Qwen (阿里)",
  openai: "OpenAI",
  claude: "Claude (Anthropic)",
  antigravity: "Antigravity (Gemini 3 Pro)",
  codex: "Codex (OpenAI OAuth)",
  claude_oauth: "Claude OAuth",
  iflow: "iFlow",
};
