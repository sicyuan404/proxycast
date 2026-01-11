/**
 * 连接管理 API
 *
 * 提供与 Tauri 后端连接命令交互的 TypeScript 接口。
 *
 * @module lib/connection-api
 */

import { safeInvoke } from "@/lib/dev-bridge";

/**
 * 连接类型
 */
export type ConnectionType = "local" | "ssh" | "wsl";

/**
 * 连接来源
 */
export type ConnectionSource = "builtIn" | "userConfig" | "sshConfig";

/**
 * 连接列表条目
 */
export interface ConnectionListEntry {
  /** 连接名称/标识 */
  name: string;
  /** 连接类型 */
  type: ConnectionType;
  /** 显示标签 */
  label: string;
  /** 配置来源 */
  source: ConnectionSource;
  /** 主机名 */
  host?: string;
  /** 用户名 */
  user?: string;
  /** 端口 */
  port?: number;
}

/**
 * 连接配置
 */
export interface ConnectionConfig {
  /** 连接类型 */
  type: ConnectionType;
  /** 用户名 */
  user?: string;
  /** 主机名 */
  host?: string;
  /** 端口 */
  port?: number;
  /** 身份文件 */
  identityFile?: string;
  /** 身份文件列表 */
  identityFiles?: string[];
  /** 跳板机 */
  proxyJump?: string;
  /** 显示顺序 */
  displayOrder?: number;
  /** 是否隐藏 */
  hidden?: boolean;
  /** WSL 发行版 */
  wslDistro?: string;
}

/**
 * 添加连接请求
 */
export interface AddConnectionRequest {
  /** 连接名称 */
  name: string;
  /** 连接类型 */
  type: ConnectionType;
  /** 用户名 */
  user?: string;
  /** 主机名 */
  host?: string;
  /** 端口 */
  port?: number;
  /** 身份文件 */
  identityFile?: string;
  /** 跳板机 */
  proxyJump?: string;
  /** WSL 发行版 */
  wslDistro?: string;
}

/**
 * 通用响应
 */
export interface ConnectionResponse {
  /** 是否成功 */
  success: boolean;
  /** 错误信息 */
  error?: string;
}

/**
 * 获取所有可用连接
 */
export async function listConnections(): Promise<ConnectionListEntry[]> {
  return safeInvoke<ConnectionListEntry[]>("connection_list");
}

/**
 * 添加新连接
 */
export async function addConnection(
  request: AddConnectionRequest,
): Promise<ConnectionResponse> {
  return safeInvoke<ConnectionResponse>("connection_add", { request });
}

/**
 * 更新连接
 */
export async function updateConnection(
  name: string,
  config: ConnectionConfig,
): Promise<ConnectionResponse> {
  return safeInvoke<ConnectionResponse>("connection_update", {
    request: { name, config },
  });
}

/**
 * 删除连接
 */
export async function deleteConnection(
  name: string,
): Promise<ConnectionResponse> {
  return safeInvoke<ConnectionResponse>("connection_delete", { name });
}

/**
 * 获取连接配置
 */
export async function getConnection(
  name: string,
): Promise<ConnectionConfig | null> {
  return safeInvoke<ConnectionConfig | null>("connection_get", { name });
}

/**
 * 获取配置文件路径
 */
export async function getConnectionConfigPath(): Promise<string> {
  return safeInvoke<string>("connection_get_config_path");
}

/**
 * 获取原始配置内容
 */
export async function getRawConnectionConfig(): Promise<string> {
  return safeInvoke<string>("connection_get_raw_config");
}

/**
 * 保存原始配置内容
 */
export async function saveRawConnectionConfig(
  content: string,
): Promise<ConnectionResponse> {
  return safeInvoke<ConnectionResponse>("connection_save_raw_config", {
    content,
  });
}

/**
 * 测试连接
 */
export async function testConnection(
  name: string,
): Promise<ConnectionResponse> {
  return safeInvoke<ConnectionResponse>("connection_test", { name });
}

/**
 * 导入 SSH Host 到用户配置
 */
export async function importSSHHost(
  hostName: string,
): Promise<ConnectionResponse> {
  return safeInvoke<ConnectionResponse>("connection_import_ssh_host", {
    host_name: hostName,
  });
}

/**
 * 将连接名称转换为 terminal 会话的 connection 字符串
 *
 * @param entry - 连接列表条目
 * @returns 用于创建终端会话的连接字符串
 */
export function connectionToSessionString(entry: ConnectionListEntry): string {
  if (entry.type === "local") {
    return "";
  }

  if (entry.type === "ssh") {
    // 如果来自 SSH 配置，直接使用名称
    if (entry.source === "sshConfig") {
      return entry.name;
    }

    // 用户配置的 SSH 连接
    const user = entry.user || "root";
    const host = entry.host || entry.name;
    const port = entry.port;

    if (port && port !== 22) {
      return `${user}@${host}:${port}`;
    }
    return `${user}@${host}`;
  }

  if (entry.type === "wsl") {
    return `wsl://${entry.name}`;
  }

  return "";
}
