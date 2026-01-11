/**
 * @file 插件 UI Hook
 * @description 提供插件 UI 状态管理和消息处理的 React Hook
 * @module lib/plugin-ui/usePluginUI
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { safeInvoke, safeListen } from "@/lib/dev-bridge";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { surfaceManager, SurfaceManager } from "./SurfaceManager";
import { initPluginUI } from "./index";
import type {
  PluginId,
  SurfaceId,
  SurfaceState,
  ServerMessage,
  UserAction,
} from "./types";

/** Hook 配置选项 */
interface UsePluginUIOptions {
  /** 插件 ID */
  pluginId: PluginId;
  /** 是否自动初始化 */
  autoInit?: boolean;
  /** 自定义 Surface 管理器 */
  manager?: SurfaceManager;
}

/** Hook 返回值 */
interface UsePluginUIResult {
  /** 插件的所有 Surface */
  surfaces: SurfaceState[];
  /** 是否正在加载 */
  loading: boolean;
  /** 错误信息 */
  error: string | null;
  /** 处理用户操作 */
  handleAction: (action: UserAction) => Promise<void>;
  /** 刷新 Surface */
  refresh: () => Promise<void>;
}

/**
 * 插件 UI Hook
 * 管理插件的 UI Surface 状态
 */
export function usePluginUI(options: UsePluginUIOptions): UsePluginUIResult {
  const { pluginId, autoInit = true, manager = surfaceManager } = options;

  const [surfaces, setSurfaces] = useState<SurfaceState[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const initializedRef = useRef(false);

  // 初始化
  useEffect(() => {
    if (!initializedRef.current) {
      initPluginUI();
      initializedRef.current = true;
    }
  }, []);

  // 订阅 Surface 变化
  useEffect(() => {
    const unsubscribe = manager.subscribe((allSurfaces) => {
      const pluginSurfaces = Array.from(allSurfaces.values()).filter(
        (s) => s.pluginId === pluginId,
      );
      setSurfaces(pluginSurfaces);
    });

    return unsubscribe;
  }, [pluginId, manager]);

  // 监听来自 Rust 的 UI 消息
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      try {
        unlisten = await safeListen<{
          pluginId: string;
          message: ServerMessage;
        }>("plugin-ui-message", (event) => {
          if (event.payload.pluginId === pluginId) {
            manager.processMessage(pluginId, event.payload.message);
          }
        });
      } catch (err) {
        console.error("[usePluginUI] 监听事件失败:", err);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [pluginId, manager]);

  // 加载初始 UI
  const loadInitialUI = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      // 调用 Rust 获取插件的初始 UI 定义
      const messages = await safeInvoke<ServerMessage[]>("get_plugin_ui", {
        pluginId,
      });

      // 处理所有消息
      for (const message of messages) {
        manager.processMessage(pluginId, message);
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      console.error("[usePluginUI] 加载 UI 失败:", err);
    } finally {
      setLoading(false);
    }
  }, [pluginId, manager]);

  // 自动初始化
  useEffect(() => {
    if (autoInit) {
      loadInitialUI();
    }
  }, [autoInit, loadInitialUI]);

  // 处理用户操作
  const handleAction = useCallback(
    async (action: UserAction) => {
      try {
        // 特殊处理数据更新操作
        if (action.name === "__data_update__") {
          const { path, value } = action.context as {
            path: string;
            value: unknown;
          };
          const dataStore = manager.getDataStore(action.surfaceId);
          if (dataStore) {
            dataStore.setValue(path, value);
          }
          return;
        }

        // 发送操作到 Rust
        const responses = await safeInvoke<ServerMessage[]>(
          "handle_plugin_action",
          {
            pluginId,
            action,
          },
        );

        // 处理响应消息
        for (const message of responses) {
          manager.processMessage(pluginId, message);
        }
      } catch (err) {
        console.error("[usePluginUI] 处理操作失败:", err);
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [pluginId, manager],
  );

  // 刷新
  const refresh = useCallback(async () => {
    manager.clearPlugin(pluginId);
    await loadInitialUI();
  }, [pluginId, manager, loadInitialUI]);

  return {
    surfaces,
    loading,
    error,
    handleAction,
    refresh,
  };
}

/**
 * 单个 Surface 的 Hook
 */
export function usePluginSurface(
  pluginId: PluginId,
  surfaceId: SurfaceId,
): {
  surface: SurfaceState | undefined;
  handleAction: (action: UserAction) => Promise<void>;
} {
  const { surfaces, handleAction } = usePluginUI({ pluginId });

  const surface = surfaces.find((s) => s.surfaceId === surfaceId);

  return { surface, handleAction };
}
