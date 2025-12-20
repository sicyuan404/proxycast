/**
 * 添加凭证模态框
 * 根据 Provider 类型显示不同的表单
 */

import { useState } from "react";
import { X, Key, FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { providerPoolApi, PoolProviderType } from "@/lib/api/providerPool";
import { AntigravityForm } from "./credential-forms/AntigravityForm";
import { CodexForm } from "./credential-forms/CodexForm";
import { ClaudeOAuthForm } from "./credential-forms/ClaudeOAuthForm";
import { QwenForm } from "./credential-forms/QwenForm";
import { IFlowForm } from "./credential-forms/IFlowForm";
import { GeminiForm } from "./credential-forms/GeminiForm";
import { defaultCredsPath, providerLabels } from "./credential-forms/types";

interface AddCredentialModalProps {
  providerType: PoolProviderType;
  onClose: () => void;
  onSuccess: () => void;
}

export function AddCredentialModal({
  providerType,
  onClose,
  onSuccess,
}: AddCredentialModalProps) {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // OAuth 字段
  const [credsFilePath, setCredsFilePath] = useState(
    defaultCredsPath[providerType] || "",
  );
  const [projectId, setProjectId] = useState("");

  // API Key 字段
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");

  // 判断是否为 OAuth 类型（不包括有特殊表单的 antigravity、codex、claude_oauth、qwen、iflow、gemini）
  const isSimpleOAuth = ["kiro"].includes(providerType);
  const isApiKey = ["openai", "claude"].includes(providerType);

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (selected) {
        setCredsFilePath(selected as string);
      }
    } catch (e) {
      console.error("Failed to open file dialog:", e);
    }
  };

  // Antigravity 表单
  const antigravityForm = AntigravityForm({
    name,
    credsFilePath,
    setCredsFilePath,
    projectId,
    setProjectId,
    onSelectFile: handleSelectFile,
    loading,
    setLoading,
    setError,
    onSuccess,
  });

  // Codex 表单
  const codexForm = CodexForm({
    name,
    credsFilePath,
    setCredsFilePath,
    onSelectFile: handleSelectFile,
    loading,
    setLoading,
    setError,
    onSuccess,
  });

  // Claude OAuth 表单
  const claudeOAuthForm = ClaudeOAuthForm({
    name,
    credsFilePath,
    setCredsFilePath,
    onSelectFile: handleSelectFile,
    loading,
    setLoading,
    setError,
    onSuccess,
  });

  // Qwen 表单
  const qwenForm = QwenForm({
    name,
    credsFilePath,
    setCredsFilePath,
    onSelectFile: handleSelectFile,
    loading,
    setLoading,
    setError,
    onSuccess,
  });

  // iFlow 表单
  const iflowForm = IFlowForm({
    name,
    credsFilePath,
    setCredsFilePath,
    onSelectFile: handleSelectFile,
    loading,
    setLoading,
    setError,
    onSuccess,
  });

  // Gemini 表单
  const geminiForm = GeminiForm({
    name,
    credsFilePath,
    setCredsFilePath,
    projectId,
    setProjectId,
    onSelectFile: handleSelectFile,
    loading,
    setLoading,
    setError,
    onSuccess,
  });

  // 简单 OAuth 和 API Key 的提交处理
  const handleSubmit = async () => {
    setLoading(true);
    setError(null);

    try {
      const trimmedName = name.trim() || undefined;

      if (isSimpleOAuth) {
        if (!credsFilePath) {
          setError("请选择凭证文件");
          setLoading(false);
          return;
        }

        switch (providerType) {
          case "kiro":
            await providerPoolApi.addKiroOAuth(credsFilePath, trimmedName);
            break;
          case "gemini":
            await providerPoolApi.addGeminiOAuth(
              credsFilePath,
              projectId.trim() || undefined,
              trimmedName,
            );
            break;
        }
      } else if (isApiKey) {
        if (!apiKey) {
          setError("请输入 API Key");
          setLoading(false);
          return;
        }

        switch (providerType) {
          case "openai":
            await providerPoolApi.addOpenAIKey(
              apiKey,
              baseUrl.trim() || undefined,
              trimmedName,
            );
            break;
          case "claude":
            await providerPoolApi.addClaudeKey(
              apiKey,
              baseUrl.trim() || undefined,
              trimmedName,
            );
            break;
        }
      }

      onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  // 渲染简单 OAuth 表单
  const renderSimpleOAuthForm = () => (
    <>
      <div>
        <label className="mb-1 block text-sm font-medium">
          凭证文件路径 <span className="text-red-500">*</span>
        </label>
        <div className="flex gap-2">
          <input
            type="text"
            value={credsFilePath}
            onChange={(e) => setCredsFilePath(e.target.value)}
            placeholder="输入凭证文件的完整路径..."
            className="flex-1 rounded-lg border bg-background px-3 py-2 text-sm"
          />
          <button
            type="button"
            onClick={handleSelectFile}
            className="flex items-center gap-1 rounded-lg border px-3 py-2 text-sm hover:bg-muted"
          >
            <FolderOpen className="h-4 w-4" />
            浏览
          </button>
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          {providerType === "kiro" &&
            "默认路径: ~/.aws/sso/cache/kiro-auth-token.json"}
          {providerType === "gemini" && "默认路径: ~/.gemini/oauth_creds.json"}
        </p>
      </div>

      {providerType === "gemini" && (
        <div>
          <label className="mb-1 block text-sm font-medium">
            Project ID (可选)
          </label>
          <input
            type="text"
            value={projectId}
            onChange={(e) => setProjectId(e.target.value)}
            placeholder="Google Cloud Project ID..."
            className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
          />
        </div>
      )}
    </>
  );

  // 渲染 API Key 表单
  const renderApiKeyForm = () => (
    <>
      <div>
        <label className="mb-1 block text-sm font-medium">
          API Key <span className="text-red-500">*</span>
        </label>
        <div className="relative">
          <Key className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="sk-..."
            className="w-full rounded-lg border bg-background pl-10 pr-3 py-2 text-sm"
          />
        </div>
      </div>

      <div>
        <label className="mb-1 block text-sm font-medium">
          Base URL (可选)
        </label>
        <input
          type="text"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder={
            providerType === "openai"
              ? "https://api.openai.com/v1"
              : "https://api.anthropic.com/v1"
          }
          className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
        />
        <p className="mt-1 text-xs text-muted-foreground">
          留空使用默认 URL，或输入自定义代理地址
        </p>
      </div>
    </>
  );

  // 渲染底部按钮
  const renderFooterButton = () => {
    // Antigravity 登录模式
    if (providerType === "antigravity" && antigravityForm.mode === "login") {
      if (!antigravityForm.authUrl) {
        return (
          <button
            onClick={antigravityForm.handleGetAuthUrl}
            disabled={loading}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "获取中..." : "获取授权 URL"}
          </button>
        );
      }
      return null;
    }

    // Antigravity 文件模式
    if (providerType === "antigravity" && antigravityForm.mode === "file") {
      return (
        <button
          onClick={antigravityForm.handleFileSubmit}
          disabled={loading}
          className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {loading ? "添加中..." : "添加凭证"}
        </button>
      );
    }

    // Codex 登录模式
    if (providerType === "codex" && codexForm.mode === "login") {
      if (!codexForm.authUrl) {
        return (
          <button
            onClick={codexForm.handleGetAuthUrl}
            disabled={loading}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "获取中..." : "获取授权 URL"}
          </button>
        );
      }
      return null;
    }

    // Codex 文件模式
    if (providerType === "codex" && codexForm.mode === "file") {
      return (
        <button
          onClick={codexForm.handleFileSubmit}
          disabled={loading}
          className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {loading ? "添加中..." : "添加凭证"}
        </button>
      );
    }

    // Claude OAuth 登录模式
    if (providerType === "claude_oauth" && claudeOAuthForm.mode === "login") {
      if (!claudeOAuthForm.authUrl) {
        return (
          <button
            onClick={claudeOAuthForm.handleGetAuthUrl}
            disabled={loading}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "获取中..." : "获取授权 URL"}
          </button>
        );
      }
      return null;
    }

    // Claude OAuth 文件模式
    if (providerType === "claude_oauth" && claudeOAuthForm.mode === "file") {
      return (
        <button
          onClick={claudeOAuthForm.handleFileSubmit}
          disabled={loading}
          className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {loading ? "添加中..." : "添加凭证"}
        </button>
      );
    }

    // Qwen 登录模式
    if (providerType === "qwen" && qwenForm.mode === "login") {
      if (!qwenForm.deviceCode) {
        return (
          <button
            onClick={qwenForm.handleGetDeviceCode}
            disabled={loading}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "获取中..." : "获取设备码"}
          </button>
        );
      }
      return null;
    }

    // Qwen 文件模式
    if (providerType === "qwen" && qwenForm.mode === "file") {
      return (
        <button
          onClick={qwenForm.handleFileSubmit}
          disabled={loading}
          className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {loading ? "添加中..." : "添加凭证"}
        </button>
      );
    }

    // iFlow 登录模式
    if (providerType === "iflow" && iflowForm.mode === "login") {
      if (!iflowForm.authUrl) {
        return (
          <button
            onClick={iflowForm.handleGetAuthUrl}
            disabled={loading}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "获取中..." : "获取授权 URL"}
          </button>
        );
      }
      return null;
    }

    // iFlow 文件模式
    if (providerType === "iflow" && iflowForm.mode === "file") {
      return (
        <button
          onClick={iflowForm.handleFileSubmit}
          disabled={loading}
          className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {loading ? "添加中..." : "添加凭证"}
        </button>
      );
    }

    // Gemini 登录模式
    if (providerType === "gemini" && geminiForm.mode === "login") {
      if (!geminiForm.authUrl) {
        return (
          <button
            onClick={geminiForm.handleGetAuthUrl}
            disabled={loading}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "获取中..." : "获取授权 URL"}
          </button>
        );
      }
      return null;
    }

    // Gemini 文件模式
    if (providerType === "gemini" && geminiForm.mode === "file") {
      return (
        <button
          onClick={geminiForm.handleFileSubmit}
          disabled={loading}
          className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {loading ? "添加中..." : "添加凭证"}
        </button>
      );
    }

    // 其他类型
    return (
      <button
        onClick={handleSubmit}
        disabled={loading}
        className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
      >
        {loading ? "添加中..." : "添加凭证"}
      </button>
    );
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg bg-background p-6 shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b pb-4">
          <h3 className="text-lg font-semibold">
            添加 {providerLabels[providerType]} 凭证
          </h3>
          <button onClick={onClose} className="rounded-lg p-1 hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="mt-4 space-y-4">
          {/* 名称字段 */}
          <div>
            <label className="mb-1 block text-sm font-medium">
              名称 (可选)
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="给这个凭证起个名字..."
              className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
            />
          </div>

          {/* 根据类型渲染不同表单 */}
          {providerType === "antigravity" && antigravityForm.render()}
          {providerType === "codex" && codexForm.render()}
          {providerType === "claude_oauth" && claudeOAuthForm.render()}
          {providerType === "qwen" && qwenForm.render()}
          {providerType === "iflow" && iflowForm.render()}
          {providerType === "gemini" && geminiForm.render()}
          {isSimpleOAuth && renderSimpleOAuthForm()}
          {isApiKey && renderApiKeyForm()}

          {/* 错误提示 */}
          {error && (
            <div className="rounded-lg border border-red-500 bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950/30">
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="mt-6 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-lg border px-4 py-2 text-sm hover:bg-muted"
          >
            取消
          </button>
          {renderFooterButton()}
        </div>
      </div>
    </div>
  );
}
