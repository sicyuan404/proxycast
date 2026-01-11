/**
 * Webview 管理 API
 *
 * 提供与 Tauri 后端 webview 命令交互的 TypeScript 接口。
 * 使用 Tauri 2.x 的 multiwebview 功能创建独立的浏览器窗口。
 *
 * @module lib/webview-api
 */

import { safeInvoke } from "@/lib/dev-bridge";
import { Webview } from "@tauri-apps/api/webview";

/**
 * Webview 面板信息
 */
export interface WebviewPanelInfo {
  /** 面板 ID */
  id: string;
  /** 当前 URL */
  url: string;
  /** 面板标题 */
  title: string;
  /** X 坐标 */
  x: number;
  /** Y 坐标 */
  y: number;
  /** 宽度 */
  width: number;
  /** 高度 */
  height: number;
}

/**
 * 创建 webview 面板的请求参数
 */
export interface CreateWebviewRequest {
  /** 面板 ID（唯一标识） */
  panel_id: string;
  /** 要加载的 URL */
  url: string;
  /** 面板标题（可选） */
  title?: string;
  /** X 坐标（相对于主窗口） */
  x: number;
  /** Y 坐标（相对于主窗口） */
  y: number;
  /** 宽度 */
  width: number;
  /** 高度 */
  height: number;
}

/**
 * 创建 webview 面板的响应
 */
export interface CreateWebviewResponse {
  /** 是否成功 */
  success: boolean;
  /** 面板 ID */
  panel_id: string;
  /** 错误信息（如果有） */
  error?: string;
}

/**
 * 创建一个新的 webview 窗口来显示外部 URL
 *
 * @param request - 创建请求参数
 * @returns 创建结果
 */
export async function createWebviewPanel(
  request: CreateWebviewRequest,
): Promise<CreateWebviewResponse> {
  return safeInvoke<CreateWebviewResponse>("create_webview_panel", { request });
}

/**
 * 关闭 webview 面板
 *
 * 尝试多种方法关闭 webview：
 * 1. 使用 Tauri JavaScript API 直接关闭
 * 2. 使用后端命令关闭
 *
 * @param panelId - 面板 ID
 * @returns 是否成功
 */
export async function closeWebviewPanel(panelId: string): Promise<boolean> {
  console.log("[webview-api] 尝试关闭 webview:", panelId);

  // 方法 1: 尝试使用 Tauri JavaScript API 直接关闭
  try {
    const webview = await Webview.getByLabel(panelId);
    if (webview) {
      console.log("[webview-api] 找到 webview，尝试关闭");
      await webview.close();
      console.log("[webview-api] Tauri API 关闭成功");
      // 也调用后端清理状态
      await safeInvoke<boolean>("close_webview_panel", {
        panel_id: panelId,
      }).catch(() => {});
      return true;
    }
  } catch (e) {
    console.warn("[webview-api] Tauri API 关闭失败:", e);
  }

  // 方法 2: 使用后端命令关闭
  try {
    const result = await safeInvoke<boolean>("close_webview_panel", {
      panel_id: panelId,
    });
    console.log("[webview-api] 后端命令关闭结果:", result);
    return result;
  } catch (e) {
    console.error("[webview-api] 后端命令关闭失败:", e);
    return false;
  }
}

/**
 * 导航到新 URL
 *
 * @param panelId - 面板 ID
 * @param url - 新 URL
 * @returns 是否成功
 */
export async function navigateWebviewPanel(
  panelId: string,
  url: string,
): Promise<boolean> {
  return safeInvoke<boolean>("navigate_webview_panel", {
    panel_id: panelId,
    url,
  });
}

/**
 * 获取所有活跃的 webview 面板
 *
 * @returns 面板列表
 */
export async function getWebviewPanels(): Promise<WebviewPanelInfo[]> {
  return safeInvoke<WebviewPanelInfo[]>("get_webview_panels");
}

/**
 * 聚焦指定的 webview 面板
 *
 * @param panelId - 面板 ID
 * @returns 是否成功
 */
export async function focusWebviewPanel(panelId: string): Promise<boolean> {
  return safeInvoke<boolean>("focus_webview_panel", { panel_id: panelId });
}

/**
 * 调整 webview 面板大小和位置
 *
 * @param panelId - 面板 ID
 * @param x - 新的 X 坐标
 * @param y - 新的 Y 坐标
 * @param width - 新的宽度
 * @param height - 新的高度
 * @returns 是否成功
 */
export async function resizeWebviewPanel(
  panelId: string,
  x: number,
  y: number,
  width: number,
  height: number,
): Promise<boolean> {
  return safeInvoke<boolean>("resize_webview_panel", {
    panel_id: panelId,
    x,
    y,
    width,
    height,
  });
}

/**
 * 生成唯一的面板 ID
 *
 * @returns 唯一 ID
 */
export function generatePanelId(): string {
  return `webview-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
}
