/**
 * @file useFabricCanvas Hook 属性测试
 * @description 测试缩放功能的正确性属性
 * @module components/content-creator/canvas/poster/hooks/useFabricCanvas.test
 */

import { describe, it, expect } from "vitest";
import { test } from "@fast-check/vitest";
import * as fc from "fast-check";
import { zoomUtils } from "./useFabricCanvas";
import { clampZoom, ZOOM_MIN, ZOOM_MAX, ZOOM_STEP } from "../types";

/**
 * Feature: ai-content-creator-phase2, Property 1: 缩放范围不变量
 * Validates: Requirements 1.5
 *
 * *For any* 缩放操作（滚轮或按钮），画布缩放值应该始终在 10%-200% 范围内，不会超出边界。
 */
describe("Property 1: 缩放范围不变量", () => {
  /**
   * Property: clampZoom 函数应该始终返回 [ZOOM_MIN, ZOOM_MAX] 范围内的值
   */
  test.prop([fc.integer({ min: -1000, max: 1000 })])(
    "clampZoom 应该将任意整数限制在 [10, 200] 范围内",
    (inputZoom: unknown) => {
      const typedInputZoom = inputZoom as number;
      const result = clampZoom(typedInputZoom);

      // 结果应该在有效范围内
      expect(result).toBeGreaterThanOrEqual(ZOOM_MIN);
      expect(result).toBeLessThanOrEqual(ZOOM_MAX);

      // 如果输入在范围内，结果应该等于输入
      if (typedInputZoom >= ZOOM_MIN && typedInputZoom <= ZOOM_MAX) {
        expect(result).toBe(typedInputZoom);
      }

      // 如果输入小于最小值，结果应该等于最小值
      if (typedInputZoom < ZOOM_MIN) {
        expect(result).toBe(ZOOM_MIN);
      }

      // 如果输入大于最大值，结果应该等于最大值
      if (typedInputZoom > ZOOM_MAX) {
        expect(result).toBe(ZOOM_MAX);
      }
    },
  );

  /**
   * Property: calculateZoomIn 应该增加缩放值但不超过 ZOOM_MAX
   */
  test.prop([fc.integer({ min: ZOOM_MIN, max: ZOOM_MAX })])(
    "放大操作后缩放值应该增加但不超过 200%",
    (currentZoom: unknown) => {
      const typedCurrentZoom = currentZoom as number;
      const result = zoomUtils.calculateZoomIn(typedCurrentZoom);

      // 结果应该在有效范围内
      expect(result).toBeGreaterThanOrEqual(ZOOM_MIN);
      expect(result).toBeLessThanOrEqual(ZOOM_MAX);

      // 如果当前值小于最大值，结果应该增加
      if (typedCurrentZoom < ZOOM_MAX) {
        expect(result).toBeGreaterThan(typedCurrentZoom);
      }

      // 如果当前值已经是最大值，结果应该保持不变
      if (typedCurrentZoom >= ZOOM_MAX) {
        expect(result).toBe(ZOOM_MAX);
      }
    },
  );

  /**
   * Property: calculateZoomOut 应该减少缩放值但不低于 ZOOM_MIN
   */
  test.prop([fc.integer({ min: ZOOM_MIN, max: ZOOM_MAX })])(
    "缩小操作后缩放值应该减少但不低于 10%",
    (currentZoom: unknown) => {
      const typedCurrentZoom = currentZoom as number;
      const result = zoomUtils.calculateZoomOut(typedCurrentZoom);

      // 结果应该在有效范围内
      expect(result).toBeGreaterThanOrEqual(ZOOM_MIN);
      expect(result).toBeLessThanOrEqual(ZOOM_MAX);

      // 如果当前值大于最小值，结果应该减少
      if (typedCurrentZoom > ZOOM_MIN) {
        expect(result).toBeLessThan(typedCurrentZoom);
      }

      // 如果当前值已经是最小值，结果应该保持不变
      if (typedCurrentZoom <= ZOOM_MIN) {
        expect(result).toBe(ZOOM_MIN);
      }
    },
  );

  /**
   * Property: 连续放大操作最终会达到最大值并保持不变
   */
  test.prop([fc.integer({ min: ZOOM_MIN, max: ZOOM_MAX })])(
    "连续放大最终会达到最大值 200%",
    (startZoom: unknown) => {
      let currentZoom = startZoom as number;

      // 执行足够多次放大操作
      const maxIterations = Math.ceil((ZOOM_MAX - ZOOM_MIN) / ZOOM_STEP) + 1;
      for (let i = 0; i < maxIterations; i++) {
        currentZoom = zoomUtils.calculateZoomIn(currentZoom);
      }

      // 最终应该达到最大值
      expect(currentZoom).toBe(ZOOM_MAX);
    },
  );

  /**
   * Property: 连续缩小操作最终会达到最小值并保持不变
   */
  test.prop([fc.integer({ min: ZOOM_MIN, max: ZOOM_MAX })])(
    "连续缩小最终会达到最小值 10%",
    (startZoom: unknown) => {
      let currentZoom = startZoom as number;

      // 执行足够多次缩小操作
      const maxIterations = Math.ceil((ZOOM_MAX - ZOOM_MIN) / ZOOM_STEP) + 1;
      for (let i = 0; i < maxIterations; i++) {
        currentZoom = zoomUtils.calculateZoomOut(currentZoom);
      }

      // 最终应该达到最小值
      expect(currentZoom).toBe(ZOOM_MIN);
    },
  );

  /**
   * Property: isValidZoom 应该正确验证缩放值
   */
  test.prop([fc.integer({ min: -1000, max: 1000 })])(
    "isValidZoom 应该正确判断缩放值是否在有效范围内",
    (zoom: unknown) => {
      const typedZoom = zoom as number;
      const isValid = zoomUtils.isValidZoom(typedZoom);
      const expectedValid = typedZoom >= ZOOM_MIN && typedZoom <= ZOOM_MAX;

      expect(isValid).toBe(expectedValid);
    },
  );

  /**
   * Property: clampZoom 的幂等性 - 对已经在范围内的值再次 clamp 应该返回相同值
   */
  test.prop([fc.integer({ min: -1000, max: 1000 })])(
    "clampZoom 应该是幂等的",
    (inputZoom: unknown) => {
      const typedInputZoom = inputZoom as number;
      const firstClamp = clampZoom(typedInputZoom);
      const secondClamp = clampZoom(firstClamp);

      expect(secondClamp).toBe(firstClamp);
    },
  );
});

/**
 * 单元测试 - 边界情况
 */
describe("缩放边界情况", () => {
  it("应该正确处理边界值 ZOOM_MIN", () => {
    expect(clampZoom(ZOOM_MIN)).toBe(ZOOM_MIN);
    expect(zoomUtils.calculateZoomOut(ZOOM_MIN)).toBe(ZOOM_MIN);
    expect(zoomUtils.calculateZoomIn(ZOOM_MIN)).toBe(ZOOM_MIN + ZOOM_STEP);
  });

  it("应该正确处理边界值 ZOOM_MAX", () => {
    expect(clampZoom(ZOOM_MAX)).toBe(ZOOM_MAX);
    expect(zoomUtils.calculateZoomIn(ZOOM_MAX)).toBe(ZOOM_MAX);
    expect(zoomUtils.calculateZoomOut(ZOOM_MAX)).toBe(ZOOM_MAX - ZOOM_STEP);
  });

  it("应该正确处理极端负值", () => {
    expect(clampZoom(-1000)).toBe(ZOOM_MIN);
    expect(clampZoom(Number.MIN_SAFE_INTEGER)).toBe(ZOOM_MIN);
  });

  it("应该正确处理极端正值", () => {
    expect(clampZoom(1000)).toBe(ZOOM_MAX);
    expect(clampZoom(Number.MAX_SAFE_INTEGER)).toBe(ZOOM_MAX);
  });

  it("常量值应该正确", () => {
    expect(ZOOM_MIN).toBe(10);
    expect(ZOOM_MAX).toBe(200);
    expect(ZOOM_STEP).toBe(10);
  });
});
