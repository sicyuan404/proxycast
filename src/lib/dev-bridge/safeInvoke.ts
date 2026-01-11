/**
 * @file Safe Tauri Invoke 封装
 * @description 提供安全的 Tauri invoke 调用，支持三层 fallback：
 *   1. Tauri IPC (生产环境或 Tauri webview)
 *   2. HTTP Bridge (开发模式，浏览器 + Tauri 后端)
 *   3. Mock (纯浏览器开发)
 *
 * @module dev-bridge/safeInvoke
 */

import { invoke as baseInvoke } from "@tauri-apps/api/core";
import { listen as baseListen, emit as baseEmit } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { invokeViaHttp, isDevBridgeAvailable } from "./http-client";

/**
 * 安全的 Tauri invoke 封装
 * 支持三种模式：Tauri IPC → HTTP Bridge → Mock
 */
export async function safeInvoke<T = any>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  // 1. 优先使用 Tauri IPC (生产环境或 Tauri webview 可用时)
  if (
    typeof window !== "undefined" &&
    (window as any).__TAURI__?.core?.invoke
  ) {
    return (window as any).__TAURI__.core.invoke(cmd, args);
  }

  // Legacy check for older Tauri versions
  if (typeof window !== "undefined" && (window as any).__TAURI__?.invoke) {
    return (window as any).__TAURI__.invoke(cmd, args);
  }

  // 2. Dev 模式下尝试 HTTP 桥接（浏览器环境，Tauri 后端在运行）
  if (isDevBridgeAvailable()) {
    try {
      const result = await invokeViaHttp(cmd, args);
      return result as T;
    } catch {
      // 继续尝试 mock
    }
  }

  // 3. Fallback 到 mock（Vite alias 会替换 @tauri-apps 导入）
  return baseInvoke(cmd, args);
}

/**
 * 安全的 Tauri listen 封装
 * 优先使用真实的 Tauri event API
 */
export async function safeListen<T = any>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> {
  // 1. 优先使用 Tauri event API
  if (
    typeof window !== "undefined" &&
    (window as any).__TAURI__?.event?.listen
  ) {
    return (window as any).__TAURI__.event.listen(event, handler);
  }

  // 2. Fallback 到 mock（Vite alias 会替换 @tauri-apps 导入）
  return baseListen(event, handler);
}

/**
 * 安全的 Tauri emit 封装
 * 优先使用真实的 Tauri event API
 */
export async function safeEmit(
  event: string,
  payload?: unknown,
): Promise<void> {
  // 1. 优先使用 Tauri event API
  if (typeof window !== "undefined" && (window as any).__TAURI__?.event?.emit) {
    return (window as any).__TAURI__.event.emit(event, payload);
  }

  // 2. Fallback 到 mock
  return baseEmit(event, payload);
}

export default safeInvoke;
