/**
 * @file terminal-api.ts
 * @description 终端核心能力 API 封装
 * @module lib/terminal-api
 *
 * 提供终端会话管理的 Tauri 命令封装和事件监听。
 *
 * ## 架构说明
 * PTY 在后端预创建，使用默认大小 (24x80)。前端连接后通过 resize 同步实际大小。
 *
 * ## 功能
 * - 创建/关闭终端会话
 * - 发送输入到终端
 * - 调整终端大小
 * - 监听终端输出和状态事件
 *
 * ## 使用示例
 * ```typescript
 * import { createTerminalSession, writeToTerminal, resizeTerminal } from '@/lib/terminal-api';
 *
 * // 创建会话（使用默认大小）
 * const sessionId = await createTerminalSession();
 * // 同步实际大小
 * await resizeTerminal(sessionId, 40, 120);
 * // 发送输入
 * await writeToTerminal(sessionId, 'ls -la\n');
 * ```
 */

import { safeInvoke, safeListen } from "@/lib/dev-bridge";
import type { UnlistenFn } from "@tauri-apps/api/event";

// ============================================================================
// 类型定义
// ============================================================================

/** 会话状态 */
export type SessionStatus = "connecting" | "running" | "done" | "error";

/** 创建会话响应 */
export interface CreateSessionResponse {
  /** 会话 ID */
  session_id: string;
}

/** 会话元数据 */
export interface SessionMetadata {
  /** 会话 ID */
  id: string;
  /** 会话状态 */
  status: SessionStatus;
  /** 创建时间（Unix 时间戳，毫秒） */
  created_at: number;
  /** 终端行数 */
  rows: number;
  /** 终端列数 */
  cols: number;
}

/** 终端输出事件 */
export interface TerminalOutputEvent {
  /** 会话 ID */
  session_id: string;
  /** 输出数据（Base64 编码） */
  data: string;
}

/** 终端状态事件 */
export interface TerminalStatusEvent {
  /** 会话 ID */
  session_id: string;
  /** 会话状态 */
  status: SessionStatus;
  /** 退出码 */
  exit_code?: number;
  /** 错误信息 */
  error?: string;
}

// ============================================================================
// 事件名称
// ============================================================================

export const TERMINAL_OUTPUT_EVENT = "terminal:output";
export const TERMINAL_STATUS_EVENT = "terminal:status";

// ============================================================================
// API 函数
// ============================================================================

/**
 * 创建终端会话（使用默认大小）
 *
 * PTY 使用默认大小 (24x80) 预创建，前端连接后通过 resizeTerminal 同步实际大小。
 *
 * @param cwd - 工作目录（可选）
 * @returns 会话 ID
 */
export async function createTerminalSession(cwd?: string): Promise<string> {
  const response = await safeInvoke<CreateSessionResponse>(
    "terminal_create_session",
    { cwd },
  );
  return response.session_id;
}

/**
 * 向终端发送输入
 *
 * @param sessionId - 会话 ID
 * @param data - 输入数据（字符串，会自动 Base64 编码）
 */
export async function writeToTerminal(
  sessionId: string,
  data: string,
): Promise<void> {
  // 将字符串转换为 Base64
  const encoder = new TextEncoder();
  const bytes = encoder.encode(data);
  const base64 = btoa(String.fromCharCode(...bytes));

  await safeInvoke("terminal_write", {
    sessionId,
    data: base64,
  });
}

/**
 * 向终端发送原始字节数据
 *
 * @param sessionId - 会话 ID
 * @param data - Base64 编码的数据
 */
export async function writeToTerminalRaw(
  sessionId: string,
  data: string,
): Promise<void> {
  await safeInvoke("terminal_write", {
    sessionId,
    data,
  });
}

/**
 * 调整终端大小
 *
 * @param sessionId - 会话 ID
 * @param rows - 新的行数
 * @param cols - 新的列数
 */
export async function resizeTerminal(
  sessionId: string,
  rows: number,
  cols: number,
): Promise<void> {
  await safeInvoke("terminal_resize", {
    sessionId,
    rows,
    cols,
  });
}

/**
 * 关闭终端会话
 *
 * @param sessionId - 会话 ID
 */
export async function closeTerminal(sessionId: string): Promise<void> {
  await safeInvoke("terminal_close", {
    sessionId,
  });
}

/**
 * 获取所有终端会话
 *
 * @returns 会话元数据列表
 */
export async function listTerminalSessions(): Promise<SessionMetadata[]> {
  return safeInvoke<SessionMetadata[]>("terminal_list_sessions");
}

/**
 * 获取单个终端会话信息
 *
 * @param sessionId - 会话 ID
 * @returns 会话元数据，如果不存在则返回 null
 */
export async function getTerminalSession(
  sessionId: string,
): Promise<SessionMetadata | null> {
  return safeInvoke<SessionMetadata | null>("terminal_get_session", {
    sessionId,
  });
}

// ============================================================================
// 事件监听
// ============================================================================

/**
 * 监听终端输出事件
 *
 * @param callback - 回调函数，接收输出事件
 * @returns 取消监听函数
 */
export async function onTerminalOutput(
  callback: (event: TerminalOutputEvent) => void,
): Promise<UnlistenFn> {
  return safeListen<TerminalOutputEvent>(TERMINAL_OUTPUT_EVENT, (event) => {
    callback(event.payload);
  });
}

/**
 * 监听终端状态事件
 *
 * @param callback - 回调函数，接收状态事件
 * @returns 取消监听函数
 */
export async function onTerminalStatus(
  callback: (event: TerminalStatusEvent) => void,
): Promise<UnlistenFn> {
  return safeListen<TerminalStatusEvent>(TERMINAL_STATUS_EVENT, (event) => {
    callback(event.payload);
  });
}

/**
 * 监听特定会话的输出事件
 *
 * @param sessionId - 会话 ID
 * @param callback - 回调函数，接收解码后的输出数据
 * @returns 取消监听函数
 */
export async function onSessionOutput(
  sessionId: string,
  callback: (data: Uint8Array) => void,
): Promise<UnlistenFn> {
  return safeListen<TerminalOutputEvent>(TERMINAL_OUTPUT_EVENT, (event) => {
    if (event.payload.session_id === sessionId) {
      // 解码 Base64 数据
      const binaryString = atob(event.payload.data);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }
      callback(bytes);
    }
  });
}

/**
 * 监听特定会话的状态事件
 *
 * @param sessionId - 会话 ID
 * @param callback - 回调函数，接收状态事件
 * @returns 取消监听函数
 */
export async function onSessionStatus(
  sessionId: string,
  callback: (event: TerminalStatusEvent) => void,
): Promise<UnlistenFn> {
  return safeListen<TerminalStatusEvent>(TERMINAL_STATUS_EVENT, (event) => {
    if (event.payload.session_id === sessionId) {
      callback(event.payload);
    }
  });
}

// ============================================================================
// 工具函数
// ============================================================================

/**
 * 将字符串编码为 Base64
 */
export function encodeBase64(str: string): string {
  const encoder = new TextEncoder();
  const bytes = encoder.encode(str);
  return btoa(String.fromCharCode(...bytes));
}

/**
 * 将 Base64 解码为 Uint8Array
 */
export function decodeBase64(base64: string): Uint8Array {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}

/**
 * 将 Uint8Array 解码为字符串
 */
export function decodeBytes(bytes: Uint8Array): string {
  const decoder = new TextDecoder();
  return decoder.decode(bytes);
}
