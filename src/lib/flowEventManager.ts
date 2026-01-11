/**
 * Flow 事件全局管理器
 *
 * 在应用级别管理 Flow 事件订阅，避免页面切换时丢失状态。
 */

import { safeInvoke, safeListen } from "@/lib/dev-bridge";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { FlowEvent, FlowSummary } from "@/lib/api/flowMonitor";

type FlowEventCallback = (event: FlowEvent) => void;

class FlowEventManager {
  private static instance: FlowEventManager;
  private unlisten: UnlistenFn | null = null;
  private subscribed = false;
  private subscribing = false;
  private callbacks: Set<FlowEventCallback> = new Set();
  private activeFlows: Map<string, FlowSummary> = new Map();
  private lastEvent: FlowEvent | null = null;
  private error: string | null = null;

  private constructor() {}

  static getInstance(): FlowEventManager {
    if (!FlowEventManager.instance) {
      FlowEventManager.instance = new FlowEventManager();
    }
    return FlowEventManager.instance;
  }

  /**
   * 订阅 Flow 事件（全局只订阅一次）
   */
  async subscribe(): Promise<void> {
    if (this.subscribed || this.subscribing) {
      return;
    }

    this.subscribing = true;
    this.error = null;

    try {
      // 调用后端命令启动事件订阅
      await safeInvoke("subscribe_flow_events");

      // 监听 Tauri 事件
      this.unlisten = await safeListen<FlowEvent>("flow-event", (event) => {
        this.handleEvent(event.payload);
      });

      this.subscribed = true;
      this.subscribing = false;
      console.log("[FlowEventManager] 已订阅 Flow 事件");
    } catch (e) {
      this.subscribing = false;
      this.error = e instanceof Error ? e.message : "订阅失败";
      console.error("[FlowEventManager] 订阅 Flow 事件失败:", e);
    }
  }

  /**
   * 取消订阅
   */
  unsubscribe(): void {
    if (this.unlisten) {
      this.unlisten();
      this.unlisten = null;
    }
    this.subscribed = false;
    this.subscribing = false;
    console.log("[FlowEventManager] 已取消订阅 Flow 事件");
  }

  /**
   * 添加事件回调
   */
  addCallback(callback: FlowEventCallback): () => void {
    this.callbacks.add(callback);
    return () => {
      this.callbacks.delete(callback);
    };
  }

  /**
   * 处理事件
   */
  private handleEvent(event: FlowEvent): void {
    this.lastEvent = event;

    // 更新活跃 Flow 状态
    switch (event.type) {
      case "FlowStarted":
        this.activeFlows.set(event.flow.id, event.flow);
        break;
      case "FlowUpdated":
        {
          const existing = this.activeFlows.get(event.id);
          if (existing && event.update.state) {
            this.activeFlows.set(event.id, {
              ...existing,
              state: event.update.state,
            });
          }
        }
        break;
      case "FlowCompleted":
      case "FlowFailed":
        this.activeFlows.delete(event.id);
        break;
    }

    // 通知所有回调
    this.callbacks.forEach((callback) => {
      try {
        callback(event);
      } catch (e) {
        console.error("[FlowEventManager] 回调执行失败:", e);
      }
    });
  }

  /**
   * 获取当前状态
   */
  getState() {
    return {
      subscribed: this.subscribed,
      subscribing: this.subscribing,
      error: this.error,
      activeFlows: new Map(this.activeFlows),
      lastEvent: this.lastEvent,
    };
  }

  /**
   * 获取活跃 Flow 数量
   */
  getActiveFlowCount(): number {
    return this.activeFlows.size;
  }

  /**
   * 检查是否已订阅
   */
  isSubscribed(): boolean {
    return this.subscribed;
  }
}

// 导出单例
export const flowEventManager = FlowEventManager.getInstance();
export default flowEventManager;
