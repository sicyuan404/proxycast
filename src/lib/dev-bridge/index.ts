/**
 * 开发桥接模块
 *
 * 提供浏览器开发服务器与 Tauri 后端的 HTTP 通信桥接。
 *
 * @module dev-bridge
 */

export {
  invokeViaHttp,
  isDevBridgeAvailable,
  healthCheck,
  getBridgeStatus,
} from "./http-client";
export type {
  InvokeRequest,
  InvokeResponse,
  BridgeStatus,
} from "./http-client";

export { safeInvoke, safeListen, safeEmit } from "./safeInvoke";
