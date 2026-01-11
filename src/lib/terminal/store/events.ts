/**
 * @file events.ts
 * @description 终端后端事件订阅和状态同步
 * @module lib/terminal/store/events
 *
 * 订阅后端 Tauri 事件，自动更新 Jotai 原子状态。
 *
 * _Requirements: 9.6_
 */

import { safeListen } from "@/lib/dev-bridge";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type {
  ControllerStatusEvent,
  ConnChangeEvent,
  ConnStatus,
} from "./types";

// ============================================================================
// 后端原始事件类型（snake_case）
// ============================================================================

/**
 * 后端连接状态（snake_case 格式）
 */
interface RawConnStatus {
  status: string;
  connected: boolean;
  connection: string;
  has_connected: boolean;
  active_conn_num: number;
  error?: string;
  wsh_enabled: boolean;
  wsh_error?: string;
  no_wsh_reason?: string;
  wsh_version?: string;
}

/**
 * 后端连接变更事件（snake_case 格式）
 */
interface RawConnChangeEvent {
  connection: string;
  status: RawConnStatus;
}

// ============================================================================
// 事件名称常量
// ============================================================================

/**
 * 控制器状态事件名称
 *
 * 对应后端的 CONTROLLER_STATUS_EVENT
 */
export const CONTROLLER_STATUS_EVENT = "controller:status";

/**
 * 连接状态变更事件名称
 *
 * 对应后端的 CONN_CHANGE
 */
export const CONN_CHANGE_EVENT = "terminal:conn-change";

// ============================================================================
// 事件处理器类型
// ============================================================================

/**
 * 控制器状态事件处理器
 */
export type ControllerStatusHandler = (event: ControllerStatusEvent) => void;

/**
 * 连接变更事件处理器
 */
export type ConnChangeHandler = (event: ConnChangeEvent) => void;

// ============================================================================
// 事件订阅函数
// ============================================================================

/**
 * 订阅控制器状态事件
 *
 * 监听后端发送的控制器状态更新事件。
 *
 * @param handler - 事件处理函数
 * @returns 取消订阅函数
 *
 * _Requirements: 9.6_
 */
export async function subscribeControllerStatus(
  handler: ControllerStatusHandler,
): Promise<UnlistenFn> {
  return safeListen<ControllerStatusEvent>(CONTROLLER_STATUS_EVENT, (event) => {
    handler(event.payload);
  });
}

/**
 * 订阅连接状态变更事件
 *
 * 监听后端发送的连接状态变更事件。
 *
 * @param handler - 事件处理函数
 * @returns 取消订阅函数
 *
 * _Requirements: 9.6_
 */
export async function subscribeConnChange(
  handler: ConnChangeHandler,
): Promise<UnlistenFn> {
  return safeListen<RawConnChangeEvent>(CONN_CHANGE_EVENT, (event) => {
    // 转换后端的 snake_case 字段为前端的 camelCase
    const payload = event.payload;
    const status: ConnStatus = {
      status: payload.status.status as ConnStatus["status"],
      connected: payload.status.connected,
      connection: payload.status.connection,
      hasConnected: payload.status.has_connected,
      activeConnNum: payload.status.active_conn_num,
      error: payload.status.error,
      wshEnabled: payload.status.wsh_enabled,
      wshError: payload.status.wsh_error,
      noWshReason: payload.status.no_wsh_reason,
      wshVersion: payload.status.wsh_version,
    };

    handler({
      connection: payload.connection,
      status,
    });
  });
}

/**
 * 订阅特定块的控制器状态事件
 *
 * @param blockId - 块 ID
 * @param handler - 事件处理函数
 * @returns 取消订阅函数
 */
export async function subscribeBlockControllerStatus(
  blockId: string,
  handler: ControllerStatusHandler,
): Promise<UnlistenFn> {
  return safeListen<ControllerStatusEvent>(CONTROLLER_STATUS_EVENT, (event) => {
    if (event.payload.blockId === blockId) {
      handler(event.payload);
    }
  });
}

/**
 * 订阅特定连接的状态变更事件
 *
 * @param connection - 连接名称
 * @param handler - 事件处理函数
 * @returns 取消订阅函数
 */
export async function subscribeConnectionStatus(
  connection: string,
  handler: ConnChangeHandler,
): Promise<UnlistenFn> {
  return safeListen<RawConnChangeEvent>(CONN_CHANGE_EVENT, (event) => {
    if (event.payload.connection === connection) {
      // 转换字段名
      const payload = event.payload;
      const status: ConnStatus = {
        status: payload.status.status as ConnStatus["status"],
        connected: payload.status.connected,
        connection: payload.status.connection,
        hasConnected: payload.status.has_connected,
        activeConnNum: payload.status.active_conn_num,
        error: payload.status.error,
        wshEnabled: payload.status.wsh_enabled,
        wshError: payload.status.wsh_error,
        noWshReason: payload.status.no_wsh_reason,
        wshVersion: payload.status.wsh_version,
      };

      handler({
        connection: payload.connection,
        status,
      });
    }
  });
}

// ============================================================================
// 事件管理器
// ============================================================================

/**
 * 终端事件管理器
 *
 * 管理所有终端相关的事件订阅，提供统一的订阅和清理接口。
 *
 * _Requirements: 9.6_
 */
export class TerminalEventManager {
  private unlisteners: UnlistenFn[] = [];
  private controllerStatusHandlers: Map<string, ControllerStatusHandler[]> =
    new Map();
  private connChangeHandlers: Map<string, ConnChangeHandler[]> = new Map();
  private globalControllerStatusHandlers: ControllerStatusHandler[] = [];
  private globalConnChangeHandlers: ConnChangeHandler[] = [];
  private initialized = false;

  /**
   * 初始化事件管理器
   *
   * 订阅全局事件并分发到各个处理器。
   */
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }

    // 订阅控制器状态事件
    const unlistenController = await subscribeControllerStatus((event) => {
      // 调用全局处理器
      this.globalControllerStatusHandlers.forEach((handler) => handler(event));

      // 调用特定块的处理器
      const handlers = this.controllerStatusHandlers.get(event.blockId);
      if (handlers) {
        handlers.forEach((handler) => handler(event));
      }
    });
    this.unlisteners.push(unlistenController);

    // 订阅连接变更事件
    const unlistenConn = await subscribeConnChange((event) => {
      // 调用全局处理器
      this.globalConnChangeHandlers.forEach((handler) => handler(event));

      // 调用特定连接的处理器
      const handlers = this.connChangeHandlers.get(event.connection);
      if (handlers) {
        handlers.forEach((handler) => handler(event));
      }
    });
    this.unlisteners.push(unlistenConn);

    this.initialized = true;
    console.log("[TerminalEventManager] 已初始化事件订阅");
  }

  /**
   * 注册全局控制器状态处理器
   */
  onControllerStatus(handler: ControllerStatusHandler): () => void {
    this.globalControllerStatusHandlers.push(handler);
    return () => {
      const index = this.globalControllerStatusHandlers.indexOf(handler);
      if (index !== -1) {
        this.globalControllerStatusHandlers.splice(index, 1);
      }
    };
  }

  /**
   * 注册全局连接变更处理器
   */
  onConnChange(handler: ConnChangeHandler): () => void {
    this.globalConnChangeHandlers.push(handler);
    return () => {
      const index = this.globalConnChangeHandlers.indexOf(handler);
      if (index !== -1) {
        this.globalConnChangeHandlers.splice(index, 1);
      }
    };
  }

  /**
   * 注册特定块的控制器状态处理器
   */
  onBlockControllerStatus(
    blockId: string,
    handler: ControllerStatusHandler,
  ): () => void {
    if (!this.controllerStatusHandlers.has(blockId)) {
      this.controllerStatusHandlers.set(blockId, []);
    }
    this.controllerStatusHandlers.get(blockId)!.push(handler);

    return () => {
      const handlers = this.controllerStatusHandlers.get(blockId);
      if (handlers) {
        const index = handlers.indexOf(handler);
        if (index !== -1) {
          handlers.splice(index, 1);
        }
      }
    };
  }

  /**
   * 注册特定连接的状态变更处理器
   */
  onConnectionStatus(
    connection: string,
    handler: ConnChangeHandler,
  ): () => void {
    if (!this.connChangeHandlers.has(connection)) {
      this.connChangeHandlers.set(connection, []);
    }
    this.connChangeHandlers.get(connection)!.push(handler);

    return () => {
      const handlers = this.connChangeHandlers.get(connection);
      if (handlers) {
        const index = handlers.indexOf(handler);
        if (index !== -1) {
          handlers.splice(index, 1);
        }
      }
    };
  }

  /**
   * 清理特定块的所有处理器
   */
  cleanupBlock(blockId: string): void {
    this.controllerStatusHandlers.delete(blockId);
  }

  /**
   * 清理特定连接的所有处理器
   */
  cleanupConnection(connection: string): void {
    this.connChangeHandlers.delete(connection);
  }

  /**
   * 销毁事件管理器
   *
   * 取消所有事件订阅并清理资源。
   */
  dispose(): void {
    this.unlisteners.forEach((unlisten) => unlisten());
    this.unlisteners = [];
    this.controllerStatusHandlers.clear();
    this.connChangeHandlers.clear();
    this.globalControllerStatusHandlers = [];
    this.globalConnChangeHandlers = [];
    this.initialized = false;
    console.log("[TerminalEventManager] 已销毁事件订阅");
  }
}

/**
 * 全局终端事件管理器实例
 */
export const terminalEventManager = new TerminalEventManager();
