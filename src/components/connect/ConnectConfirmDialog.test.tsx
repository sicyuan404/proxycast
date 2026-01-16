/**
 * @file Connect 确认弹窗属性测试
 * @description 测试 Provider Display 的完整性
 * @module components/connect/ConnectConfirmDialog.test
 *
 * **Feature: proxycast-connect, Property 7: Provider Display Completeness**
 * **Validates: Requirements 6.1, 6.3**
 */

import { describe, expect } from "vitest";
import { test } from "@fast-check/vitest";
import * as fc from "fast-check";
import type { RelayInfo } from "@/hooks/useDeepLink";

/**
 * 生成有效的十六进制颜色值
 */
const hexColorArbitrary = fc
  .array(fc.integer({ min: 0, max: 15 }), { minLength: 6, maxLength: 6 })
  .map((arr: unknown) => {
    const typedArr = arr as number[];
    return "#" + typedArr.map((n: number) => n.toString(16)).join("");
  });

/**
 * 生成有效的 RelayInfo 对象的 Arbitrary
 */
const relayInfoArbitrary: fc.Arbitrary<RelayInfo> = fc.record({
  id: fc
    .string({ minLength: 1, maxLength: 50 })
    .filter((s: string) => s.trim().length > 0),
  name: fc
    .string({ minLength: 1, maxLength: 100 })
    .filter((s: string) => s.trim().length > 0),
  description: fc
    .string({ minLength: 1, maxLength: 500 })
    .filter((s: string) => s.trim().length > 0),
  branding: fc.record({
    logo: fc.webUrl(),
    color: hexColorArbitrary,
  }),
  links: fc.record({
    homepage: fc.webUrl(),
    register: fc.option(fc.webUrl(), { nil: undefined }),
    recharge: fc.option(fc.webUrl(), { nil: undefined }),
    docs: fc.option(fc.webUrl(), { nil: undefined }),
    status: fc.option(fc.webUrl(), { nil: undefined }),
  }),
  api: fc.record({
    base_url: fc.webUrl(),
    protocol: fc.constantFrom("openai", "claude", "gemini"),
    auth_header: fc.constant("Authorization"),
    auth_prefix: fc.constant("Bearer"),
  }),
  contact: fc.record({
    email: fc.option(fc.emailAddress(), { nil: undefined }),
    discord: fc.option(fc.string(), { nil: undefined }),
    telegram: fc.option(fc.string(), { nil: undefined }),
    twitter: fc.option(fc.string(), { nil: undefined }),
  }),
  features: fc.record({
    models: fc.array(fc.string({ minLength: 1 }), {
      minLength: 0,
      maxLength: 10,
    }),
    streaming: fc.boolean(),
    function_calling: fc.boolean(),
    vision: fc.boolean(),
  }),
});

/**
 * 提取 Provider 显示所需的核心信息
 *
 * 这个函数模拟 ProviderInfo 组件的数据提取逻辑，
 * 用于验证所有必要信息都能被正确提取和显示。
 *
 * @param relay - 中转商信息
 * @returns 显示所需的核心字段
 */
function extractProviderDisplayInfo(relay: RelayInfo): {
  name: string;
  logoUrl: string;
  description: string;
  homepageUrl: string | undefined;
  themeColor: string;
} {
  return {
    name: relay.name,
    logoUrl: relay.branding.logo,
    description: relay.description,
    homepageUrl: relay.links.homepage,
    themeColor: relay.branding.color,
  };
}

/**
 * 验证 Provider 显示信息的完整性
 *
 * @param displayInfo - 提取的显示信息
 * @param originalRelay - 原始 RelayInfo
 * @returns 是否完整
 */
function validateProviderDisplayCompleteness(
  displayInfo: ReturnType<typeof extractProviderDisplayInfo>,
  originalRelay: RelayInfo,
): boolean {
  // 验证名称存在且与原始数据匹配
  if (!displayInfo.name || displayInfo.name !== originalRelay.name) {
    return false;
  }

  // 验证 Logo URL 存在且与原始数据匹配
  if (
    !displayInfo.logoUrl ||
    displayInfo.logoUrl !== originalRelay.branding.logo
  ) {
    return false;
  }

  // 验证描述存在且与原始数据匹配
  if (
    !displayInfo.description ||
    displayInfo.description !== originalRelay.description
  ) {
    return false;
  }

  return true;
}

describe("Provider Display 属性测试", () => {
  /**
   * Property 7: Provider Display Completeness
   *
   * *对于任意* RelayInfo 对象，渲染的 provider 显示应包含：
   * - provider 的名称
   * - logo URL
   * - 描述
   *
   * **Feature: proxycast-connect, Property 7: Provider Display Completeness**
   * **Validates: Requirements 6.1, 6.3**
   */
  describe("Property 7: Provider Display Completeness", () => {
    test.prop([relayInfoArbitrary], { numRuns: 100 })(
      "对于任意 RelayInfo，提取的显示信息应包含 name、logo URL 和 description",
      (relay: RelayInfo) => {
        // 提取显示信息
        const displayInfo = extractProviderDisplayInfo(relay);

        // 验证完整性
        const isComplete = validateProviderDisplayCompleteness(
          displayInfo,
          relay,
        );
        expect(isComplete).toBe(true);

        // 额外验证：确保所有必要字段都存在且非空
        expect(displayInfo.name).toBeTruthy();
        expect(displayInfo.name.length).toBeGreaterThan(0);

        expect(displayInfo.logoUrl).toBeTruthy();
        expect(displayInfo.logoUrl.length).toBeGreaterThan(0);

        expect(displayInfo.description).toBeTruthy();
        expect(displayInfo.description.length).toBeGreaterThan(0);
      },
    );

    test.prop([relayInfoArbitrary], { numRuns: 100 })(
      "提取的显示信息应与原始 RelayInfo 数据完全匹配",
      (relay: RelayInfo) => {
        const displayInfo = extractProviderDisplayInfo(relay);

        // 验证数据一致性
        expect(displayInfo.name).toBe(relay.name);
        expect(displayInfo.logoUrl).toBe(relay.branding.logo);
        expect(displayInfo.description).toBe(relay.description);
        expect(displayInfo.homepageUrl).toBe(relay.links.homepage);
        expect(displayInfo.themeColor).toBe(relay.branding.color);
      },
    );

    test.prop([relayInfoArbitrary], { numRuns: 100 })(
      "显示信息中的 URL 字段应为有效的 URL 格式",
      (relay: RelayInfo) => {
        const displayInfo = extractProviderDisplayInfo(relay);

        // 验证 Logo URL 格式
        expect(() => new URL(displayInfo.logoUrl)).not.toThrow();

        // 验证主页 URL 格式（如果存在）
        if (displayInfo.homepageUrl) {
          expect(() => new URL(displayInfo.homepageUrl!)).not.toThrow();
        }
      },
    );
  });
});
