/**
 * @file Connect 统计回调 Hook
 * @description 提供统计回调功能，让中转商追踪推广效果
 * @module hooks/useConnectCallback
 *
 * _Requirements: 5.3_
 */

import { useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";

/**
 * 回调状态类型
 */
export type CallbackStatus = "success" | "cancelled" | "error";

/**
 * 发送回调的参数
 */
export interface SendCallbackParams {
  /** 中转商 ID */
  relayId: string;
  /** API Key */
  apiKey: string;
  /** 回调状态 */
  status: CallbackStatus;
  /** 推广码（可选） */
  refCode?: string;
  /** 错误码（仅 status=error 时） */
  errorCode?: string;
  /** 错误信息（仅 status=error 时） */
  errorMessage?: string;
}

/**
 * useConnectCallback Hook 返回值
 */
export interface UseConnectCallbackReturn {
  /** 发送成功回调 */
  sendSuccessCallback: (
    relayId: string,
    apiKey: string,
    refCode?: string,
  ) => Promise<boolean>;
  /** 发送取消回调 */
  sendCancelledCallback: (
    relayId: string,
    apiKey: string,
    refCode?: string,
  ) => Promise<boolean>;
  /** 发送错误回调 */
  sendErrorCallback: (
    relayId: string,
    apiKey: string,
    errorCode: string,
    errorMessage: string,
    refCode?: string,
  ) => Promise<boolean>;
  /** 通用发送回调 */
  sendCallback: (params: SendCallbackParams) => Promise<boolean>;
}

/**
 * Connect 统计回调 Hook
 *
 * 提供统计回调功能，在用户确认/取消配置后向中转商发送回调。
 *
 * ## 功能
 *
 * - 发送成功回调（用户确认添加 Key）
 * - 发送取消回调（用户取消添加）
 * - 发送错误回调（配置失败）
 * - 异步发送，不阻塞 UI
 *
 * ## 使用示例
 *
 * ```tsx
 * function ConnectDialog() {
 *   const { sendSuccessCallback, sendCancelledCallback } = useConnectCallback();
 *
 *   const handleConfirm = async () => {
 *     // 保存 API Key...
 *     await sendSuccessCallback(relayId, apiKey, refCode);
 *   };
 *
 *   const handleCancel = async () => {
 *     await sendCancelledCallback(relayId, apiKey, refCode);
 *   };
 * }
 * ```
 *
 * @returns Hook 返回值
 */
export function useConnectCallback(): UseConnectCallbackReturn {
  /**
   * 通用发送回调
   */
  const sendCallback = useCallback(
    async (params: SendCallbackParams): Promise<boolean> => {
      try {
        const result = await safeInvoke<boolean>("send_connect_callback", {
          relayId: params.relayId,
          apiKey: params.apiKey,
          status: params.status,
          refCode: params.refCode ?? null,
          errorCode: params.errorCode ?? null,
          errorMessage: params.errorMessage ?? null,
        });

        console.log(
          `[useConnectCallback] 回调发送${result ? "成功" : "跳过"}: relay=${params.relayId}, status=${params.status}`,
        );
        return result;
      } catch (err) {
        // 回调失败不应该影响主流程，只记录日志
        console.warn("[useConnectCallback] 发送回调失败:", err);
        return false;
      }
    },
    [],
  );

  /**
   * 发送成功回调
   */
  const sendSuccessCallback = useCallback(
    async (
      relayId: string,
      apiKey: string,
      refCode?: string,
    ): Promise<boolean> => {
      return sendCallback({
        relayId,
        apiKey,
        status: "success",
        refCode,
      });
    },
    [sendCallback],
  );

  /**
   * 发送取消回调
   */
  const sendCancelledCallback = useCallback(
    async (
      relayId: string,
      apiKey: string,
      refCode?: string,
    ): Promise<boolean> => {
      return sendCallback({
        relayId,
        apiKey,
        status: "cancelled",
        refCode,
      });
    },
    [sendCallback],
  );

  /**
   * 发送错误回调
   */
  const sendErrorCallback = useCallback(
    async (
      relayId: string,
      apiKey: string,
      errorCode: string,
      errorMessage: string,
      refCode?: string,
    ): Promise<boolean> => {
      return sendCallback({
        relayId,
        apiKey,
        status: "error",
        refCode,
        errorCode,
        errorMessage,
      });
    },
    [sendCallback],
  );

  return {
    sendSuccessCallback,
    sendCancelledCallback,
    sendErrorCallback,
    sendCallback,
  };
}

export default useConnectCallback;
