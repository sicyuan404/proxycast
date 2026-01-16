/**
 * @file useElementOperations Hook 属性测试
 * @description 测试元素变换功能的正确性属性
 * @module components/content-creator/canvas/poster/hooks/useElementOperations.test
 */

import { describe, it, expect } from "vitest";
import { test } from "@fast-check/vitest";
import * as fc from "fast-check";
import { transformUtils } from "./useElementOperations";

/**
 * Feature: ai-content-creator-phase2, Property 2: 元素变换正确性
 * Validates: Requirements 2.2, 2.3, 2.4
 *
 * *For any* 元素和任意变换操作（移动、缩放、旋转），变换后元素的位置、尺寸、角度应该与预期值一致。
 * - 移动：最终位置 = 初始位置 + 拖拽距离
 * - 缩放：最终尺寸 = 初始尺寸 × 缩放因子
 * - 旋转：最终角度 = 初始角度 + 旋转角度
 */
describe("Property 2: 元素变换正确性", () => {
  /**
   * Property: 移动操作应该正确计算最终位置
   * 最终位置 = 初始位置 + 拖拽距离
   */
  test.prop([
    fc.record({
      left: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
      top: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
    }),
    fc.record({
      x: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
      y: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
    }),
  ])("移动操作：最终位置 = 初始位置 + 拖拽距离", (initial: unknown, delta: unknown) => {
    const typedInitial = initial as { left: number; top: number };
    const typedDelta = delta as { x: number; y: number };
    const result = transformUtils.calculateMove(typedInitial, typedDelta);

    // 验证移动公式
    expect(result.left).toBeCloseTo(typedInitial.left + typedDelta.x, 5);
    expect(result.top).toBeCloseTo(typedInitial.top + typedDelta.y, 5);
  });

  /**
   * Property: 缩放操作应该正确计算最终尺寸
   * 最终尺寸 = 初始尺寸 × 缩放因子
   */
  test.prop([
    fc.record({
      scaleX: fc.float({
        min: Math.fround(0.01),
        max: Math.fround(100),
        noNaN: true,
      }),
      scaleY: fc.float({
        min: Math.fround(0.01),
        max: Math.fround(100),
        noNaN: true,
      }),
    }),
    fc.float({ min: Math.fround(0.01), max: Math.fround(100), noNaN: true }),
  ])("缩放操作：最终尺寸 = 初始尺寸 × 缩放因子", (initial: unknown, factor: unknown) => {
    const typedInitial = initial as { scaleX: number; scaleY: number };
    const typedFactor = factor as number;
    const result = transformUtils.calculateScale(typedInitial, typedFactor);

    // 验证缩放公式
    expect(result.scaleX).toBeCloseTo(typedInitial.scaleX * typedFactor, 5);
    expect(result.scaleY).toBeCloseTo(typedInitial.scaleY * typedFactor, 5);

    // 缩放结果应该为正数
    expect(result.scaleX).toBeGreaterThan(0);
    expect(result.scaleY).toBeGreaterThan(0);
  });

  /**
   * Property: 旋转操作应该正确计算最终角度
   * 最终角度 = (初始角度 + 旋转角度) % 360
   */
  test.prop([
    fc.float({ min: Math.fround(0), max: Math.fround(360), noNaN: true }),
    fc.float({ min: Math.fround(-720), max: Math.fround(720), noNaN: true }),
  ])("旋转操作：最终角度应该在 [0, 360) 范围内", (initialAngle, deltaAngle) => {
    const result = transformUtils.calculateRotation(initialAngle, deltaAngle);

    // 结果应该在 [0, 360) 范围内
    expect(result).toBeGreaterThanOrEqual(0);
    expect(result).toBeLessThan(360);

    // 验证旋转公式（考虑模运算和浮点精度）
    let expected = (initialAngle + deltaAngle) % 360;
    if (expected < 0) expected += 360;
    // 标准化期望值到 [0, 360) 范围
    if (expected >= 360) expected = expected % 360;
    expect(result).toBeCloseTo(expected, 5);
  });

  /**
   * Property: 移动操作的可逆性
   * 移动 delta 后再移动 -delta 应该回到原点
   */
  test.prop([
    fc.record({
      left: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
      top: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
    }),
    fc.record({
      x: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
      y: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
    }),
  ])("移动操作应该是可逆的", (initial: unknown, delta: unknown) => {
    const typedInitial = initial as { left: number; top: number };
    const typedDelta = delta as { x: number; y: number };
    const moved = transformUtils.calculateMove(typedInitial, typedDelta);
    const reversed = transformUtils.calculateMove(moved, {
      x: -typedDelta.x,
      y: -typedDelta.y,
    });

    expect(reversed.left).toBeCloseTo(typedInitial.left, 5);
    expect(reversed.top).toBeCloseTo(typedInitial.top, 5);
  });

  /**
   * Property: 缩放操作的可逆性
   * 缩放 factor 后再缩放 1/factor 应该回到原始尺寸
   */
  test.prop([
    fc.record({
      scaleX: fc.float({
        min: Math.fround(0.01),
        max: Math.fround(100),
        noNaN: true,
      }),
      scaleY: fc.float({
        min: Math.fround(0.01),
        max: Math.fround(100),
        noNaN: true,
      }),
    }),
    fc.float({ min: Math.fround(0.1), max: Math.fround(10), noNaN: true }),
  ])("缩放操作应该是可逆的", (initial: unknown, factor: unknown) => {
    const typedInitial = initial as { scaleX: number; scaleY: number };
    const typedFactor = factor as number;
    const scaled = transformUtils.calculateScale(typedInitial, typedFactor);
    const reversed = transformUtils.calculateScale(scaled, 1 / typedFactor);

    expect(reversed.scaleX).toBeCloseTo(typedInitial.scaleX, 4);
    expect(reversed.scaleY).toBeCloseTo(typedInitial.scaleY, 4);
  });

  /**
   * Property: 旋转操作的可逆性
   * 旋转 angle 后再旋转 -angle 应该回到原始角度
   */
  test.prop([
    fc.float({ min: Math.fround(0), max: Math.fround(360), noNaN: true }),
    fc.float({ min: Math.fround(-360), max: Math.fround(360), noNaN: true }),
  ])("旋转操作应该是可逆的", (initialAngle, deltaAngle) => {
    const rotated = transformUtils.calculateRotation(initialAngle, deltaAngle);
    const reversed = transformUtils.calculateRotation(rotated, -deltaAngle);

    // 由于模运算，需要标准化比较
    const normalizedInitial = initialAngle % 360;
    const normalizedReversed = reversed % 360;

    expect(normalizedReversed).toBeCloseTo(normalizedInitial, 5);
  });

  /**
   * Property: 移动操作的恒等性
   * 移动 (0, 0) 应该保持位置不变
   */
  test.prop([
    fc.record({
      left: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
      top: fc.float({
        min: Math.fround(-10000),
        max: Math.fround(10000),
        noNaN: true,
      }),
    }),
  ])("移动 (0, 0) 应该保持位置不变", (initial: unknown) => {
    const typedInitial = initial as { left: number; top: number };
    const result = transformUtils.calculateMove(typedInitial, { x: 0, y: 0 });

    // 使用 toBeCloseTo 避免 -0 和 +0 的 Object.is 比较问题
    expect(result.left).toBeCloseTo(typedInitial.left, 10);
    expect(result.top).toBeCloseTo(typedInitial.top, 10);
  });

  /**
   * Property: 缩放操作的恒等性
   * 缩放因子为 1 应该保持尺寸不变
   */
  test.prop([
    fc.record({
      scaleX: fc.float({
        min: Math.fround(0.01),
        max: Math.fround(100),
        noNaN: true,
      }),
      scaleY: fc.float({
        min: Math.fround(0.01),
        max: Math.fround(100),
        noNaN: true,
      }),
    }),
  ])("缩放因子为 1 应该保持尺寸不变", (initial: unknown) => {
    const typedInitial = initial as { scaleX: number; scaleY: number };
    const result = transformUtils.calculateScale(typedInitial, 1);

    expect(result.scaleX).toBe(typedInitial.scaleX);
    expect(result.scaleY).toBe(typedInitial.scaleY);
  });

  /**
   * Property: 旋转操作的恒等性
   * 旋转 0 度应该保持角度不变
   */
  test.prop([
    fc.float({ min: Math.fround(0), max: Math.fround(360), noNaN: true }),
  ])("旋转 0 度应该保持角度不变", (initialAngle) => {
    const result = transformUtils.calculateRotation(initialAngle, 0);

    expect(result).toBeCloseTo(initialAngle % 360, 5);
  });

  /**
   * Property: 旋转 360 度应该回到原始角度
   */
  test.prop([
    fc.float({ min: Math.fround(0), max: Math.fround(360), noNaN: true }),
  ])("旋转 360 度应该回到原始角度", (initialAngle) => {
    const result = transformUtils.calculateRotation(initialAngle, 360);

    expect(result).toBeCloseTo(initialAngle % 360, 5);
  });
});

/**
 * 单元测试 - 变换验证
 */
describe("变换数据验证", () => {
  it("应该拒绝无效的缩放值（负数或零）", () => {
    expect(transformUtils.isValidTransform({ scaleX: -1 })).toBe(false);
    expect(transformUtils.isValidTransform({ scaleY: 0 })).toBe(false);
    expect(transformUtils.isValidTransform({ scaleX: 0, scaleY: 0 })).toBe(
      false,
    );
  });

  it("应该接受有效的缩放值", () => {
    expect(transformUtils.isValidTransform({ scaleX: 1 })).toBe(true);
    expect(transformUtils.isValidTransform({ scaleY: 2 })).toBe(true);
    expect(transformUtils.isValidTransform({ scaleX: 0.5, scaleY: 1.5 })).toBe(
      true,
    );
  });

  it("应该拒绝负数的宽度和高度", () => {
    expect(transformUtils.isValidTransform({ width: -100 })).toBe(false);
    expect(transformUtils.isValidTransform({ height: -50 })).toBe(false);
  });

  it("应该接受有效的宽度和高度", () => {
    expect(transformUtils.isValidTransform({ width: 100 })).toBe(true);
    expect(transformUtils.isValidTransform({ height: 50 })).toBe(true);
    expect(transformUtils.isValidTransform({ width: 0 })).toBe(true); // 零宽度是有效的
  });

  it("应该接受空的变换对象", () => {
    expect(transformUtils.isValidTransform({})).toBe(true);
  });

  it("应该接受只有位置的变换", () => {
    expect(transformUtils.isValidTransform({ left: 100, top: 200 })).toBe(true);
    expect(transformUtils.isValidTransform({ left: -100, top: -200 })).toBe(
      true,
    );
  });
});

/**
 * 单元测试 - 边界情况
 */
describe("变换边界情况", () => {
  it("应该正确处理极大的移动距离", () => {
    const result = transformUtils.calculateMove(
      { left: 0, top: 0 },
      { x: 1000000, y: 1000000 },
    );
    expect(result.left).toBe(1000000);
    expect(result.top).toBe(1000000);
  });

  it("应该正确处理极小的缩放因子", () => {
    const result = transformUtils.calculateScale(
      { scaleX: 1, scaleY: 1 },
      0.001,
    );
    expect(result.scaleX).toBeCloseTo(0.001, 5);
    expect(result.scaleY).toBeCloseTo(0.001, 5);
  });

  it("应该正确处理负角度旋转", () => {
    const result = transformUtils.calculateRotation(45, -90);
    expect(result).toBeCloseTo(315, 5);
  });

  it("应该正确处理超过 360 度的旋转", () => {
    const result = transformUtils.calculateRotation(0, 450);
    expect(result).toBeCloseTo(90, 5);
  });
});

import { selectionUtils, groupUtils } from "./useElementOperations";

/**
 * Feature: ai-content-creator-phase2, Property 4: 多选操作正确性
 * Validates: Requirements 2.5
 *
 * *For any* 元素集合，Shift+点击操作应该将元素添加到选中集合。
 * 选中集合应该包含所有被 Shift+点击的元素，且不包含未被点击的元素。
 */
describe("Property 4: 多选操作正确性", () => {
  /**
   * Property: Shift+点击未选中元素应该将其添加到选择集合
   */
  test.prop([fc.array(fc.uuid(), { minLength: 0, maxLength: 10 }), fc.uuid()])(
    "Shift+点击未选中元素应该将其添加到选择集合",
    (currentSelection: unknown, newId: unknown) => {
      const typedCurrentSelection = currentSelection as string[];
      const typedNewId = newId as string;
      // 确保 newId 不在当前选择中
      fc.pre(!typedCurrentSelection.includes(typedNewId));

      const result = selectionUtils.calculateToggleSelection(
        typedCurrentSelection,
        typedNewId,
      );

      // 新元素应该在结果中
      expect(selectionUtils.selectionContains(result, typedNewId)).toBe(true);

      // 原有选择应该保留
      typedCurrentSelection.forEach((id) => {
        expect(selectionUtils.selectionContains(result, id)).toBe(true);
      });

      // 结果大小应该增加 1
      expect(selectionUtils.selectionSize(result)).toBe(
        selectionUtils.selectionSize(typedCurrentSelection) + 1,
      );
    },
  );

  /**
   * Property: Shift+点击已选中元素应该将其从选择集合移除
   */
  test.prop([fc.array(fc.uuid(), { minLength: 1, maxLength: 10 })])(
    "Shift+点击已选中元素应该将其从选择集合移除",
    (currentSelection: unknown) => {
      const typedCurrentSelection = currentSelection as string[];
      // 选择一个已存在的元素
      const existingId = typedCurrentSelection[0];

      const result = selectionUtils.calculateToggleSelection(
        typedCurrentSelection,
        existingId,
      );

      // 被点击的元素应该不在结果中
      expect(selectionUtils.selectionExcludes(result, existingId)).toBe(true);

      // 其他元素应该保留
      typedCurrentSelection.slice(1).forEach((id) => {
        expect(selectionUtils.selectionContains(result, id)).toBe(true);
      });

      // 结果大小应该减少 1
      expect(selectionUtils.selectionSize(result)).toBe(
        selectionUtils.selectionSize(typedCurrentSelection) - 1,
      );
    },
  );

  /**
   * Property: 连续两次 Shift+点击同一元素应该恢复原状态
   */
  test.prop([fc.array(fc.uuid(), { minLength: 0, maxLength: 10 }), fc.uuid()])(
    "连续两次 Shift+点击同一元素应该恢复原状态",
    (currentSelection: unknown, clickedId: unknown) => {
      const typedCurrentSelection = currentSelection as string[];
      const typedClickedId = clickedId as string;
      const afterFirstClick = selectionUtils.calculateToggleSelection(
        typedCurrentSelection,
        typedClickedId,
      );
      const afterSecondClick = selectionUtils.calculateToggleSelection(
        afterFirstClick,
        typedClickedId,
      );

      // 两次点击后应该恢复原状态
      expect(afterSecondClick.sort()).toEqual(typedCurrentSelection.sort());
    },
  );

  /**
   * Property: 选择操作应该保持元素唯一性
   */
  test.prop([fc.array(fc.uuid(), { minLength: 0, maxLength: 10 }), fc.uuid()])(
    "选择操作应该保持元素唯一性",
    (currentSelection: unknown, clickedId: unknown) => {
      const typedCurrentSelection = currentSelection as string[];
      const typedClickedId = clickedId as string;
      const result = selectionUtils.calculateToggleSelection(
        typedCurrentSelection,
        typedClickedId,
      );

      // 结果中不应该有重复元素
      const uniqueResult = [...new Set(result)];
      expect(result.length).toBe(uniqueResult.length);
    },
  );
});

/**
 * Feature: ai-content-creator-phase2, Property 5: 组合操作正确性
 * Validates: Requirements 2.6
 *
 * *For any* 选中的元素集合，执行组合操作后应该创建一个组对象，
 * 该组对象应该包含所有原始元素，且原始元素的相对位置保持不变。
 */
describe("Property 5: 组合操作正确性", () => {
  /**
   * Property: 只有选中 2 个或更多元素时才能组合
   */
  test.prop([fc.integer({ min: 0, max: 100 })])(
    "只有选中 2 个或更多元素时才能组合",
    (selectedCount: unknown) => {
      const typedSelectedCount = selectedCount as number;
      const canGroup = groupUtils.canGroup(typedSelectedCount);

      if (typedSelectedCount >= 2) {
        expect(canGroup).toBe(true);
      } else {
        expect(canGroup).toBe(false);
      }
    },
  );

  /**
   * Property: 组合后元素数量应该正确减少
   */
  test.prop([
    fc.integer({ min: 2, max: 100 }),
    fc.integer({ min: 2, max: 50 }),
  ])("组合后元素数量应该正确减少", (originalCount: unknown, groupedCount: unknown) => {
    const typedOriginalCount = originalCount as number;
    const typedGroupedCount = groupedCount as number;
    // 确保被组合的数量不超过原始数量
    fc.pre(typedGroupedCount <= typedOriginalCount);

    const resultCount = groupUtils.calculateGroupedCount(
      typedOriginalCount,
      typedGroupedCount,
    );

    // 组合后数量 = 原始数量 - 被组合数量 + 1
    expect(resultCount).toBe(typedOriginalCount - typedGroupedCount + 1);

    // 结果应该为正数
    expect(resultCount).toBeGreaterThan(0);
  });

  /**
   * Property: 取消组合后元素数量应该正确增加
   */
  test.prop([
    fc.integer({ min: 1, max: 100 }),
    fc.integer({ min: 2, max: 50 }),
  ])("取消组合后元素数量应该正确增加", (originalCount: unknown, ungroupedCount: unknown) => {
    const typedOriginalCount = originalCount as number;
    const typedUngroupedCount = ungroupedCount as number;
    const resultCount = groupUtils.calculateUngroupedCount(
      typedOriginalCount,
      typedUngroupedCount,
    );

    // 取消组合后数量 = 原始数量 - 1 + 组内元素数量
    expect(resultCount).toBe(typedOriginalCount - 1 + typedUngroupedCount);

    // 结果应该大于原始数量（因为组被拆分）
    expect(resultCount).toBeGreaterThan(typedOriginalCount);
  });

  /**
   * Property: 组合然后取消组合应该恢复原始元素数量
   */
  test.prop([fc.integer({ min: 2, max: 50 }), fc.integer({ min: 2, max: 20 })])(
    "组合然后取消组合应该恢复原始元素数量",
    (originalCount: unknown, groupedCount: unknown) => {
      const typedOriginalCount = originalCount as number;
      const typedGroupedCount = groupedCount as number;
      // 确保被组合的数量不超过原始数量
      fc.pre(typedGroupedCount <= typedOriginalCount);

      // 组合
      const afterGroup = groupUtils.calculateGroupedCount(
        typedOriginalCount,
        typedGroupedCount,
      );

      // 取消组合（组内元素数量等于被组合的数量）
      const afterUngroup = groupUtils.calculateUngroupedCount(
        afterGroup,
        typedGroupedCount,
      );

      // 应该恢复原始数量
      expect(afterUngroup).toBe(typedOriginalCount);
    },
  );

  /**
   * Property: 只有组对象才能取消组合
   */
  test.prop([fc.boolean()])("只有组对象才能取消组合", (isGroup: unknown) => {
    const typedIsGroup = isGroup as boolean;
    const canUngroup = groupUtils.canUngroup(typedIsGroup);

    expect(canUngroup).toBe(typedIsGroup);
  });
});

/**
 * 单元测试 - 多选边界情况
 */
describe("多选边界情况", () => {
  it("空选择集合添加元素", () => {
    const result = selectionUtils.calculateToggleSelection([], "element-1");
    expect(result).toEqual(["element-1"]);
  });

  it("单元素选择集合移除元素", () => {
    const result = selectionUtils.calculateToggleSelection(
      ["element-1"],
      "element-1",
    );
    expect(result).toEqual([]);
  });

  it("多元素选择集合添加新元素", () => {
    const result = selectionUtils.calculateToggleSelection(
      ["element-1", "element-2"],
      "element-3",
    );
    expect(result).toContain("element-1");
    expect(result).toContain("element-2");
    expect(result).toContain("element-3");
    expect(result.length).toBe(3);
  });
});

/**
 * 单元测试 - 组合边界情况
 */
describe("组合边界情况", () => {
  it("0 个元素不能组合", () => {
    expect(groupUtils.canGroup(0)).toBe(false);
  });

  it("1 个元素不能组合", () => {
    expect(groupUtils.canGroup(1)).toBe(false);
  });

  it("2 个元素可以组合", () => {
    expect(groupUtils.canGroup(2)).toBe(true);
  });

  it("组合 2 个元素后数量减少 1", () => {
    expect(groupUtils.calculateGroupedCount(5, 2)).toBe(4);
  });

  it("取消组合 2 个元素后数量增加 1", () => {
    expect(groupUtils.calculateUngroupedCount(4, 2)).toBe(5);
  });
});

import { deleteUtils } from "./useElementOperations";

/**
 * Feature: ai-content-creator-phase2, Property 6: 删除操作正确性
 * Validates: Requirements 2.8
 *
 * *For any* 选中的元素集合，执行删除操作后：
 * - 被删除的元素应该从画布中移除
 * - 未选中的元素应该保持不变
 * - 选择状态应该被清空
 */
describe("Property 6: 删除操作正确性", () => {
  /**
   * Property: 只有选中元素时才能删除
   */
  test.prop([fc.integer({ min: 0, max: 100 })])(
    "只有选中元素时才能删除",
    (selectedCount: unknown) => {
      const typedSelectedCount = selectedCount as number;
      const canDelete = deleteUtils.canDelete(typedSelectedCount);

      if (typedSelectedCount > 0) {
        expect(canDelete).toBe(true);
      } else {
        expect(canDelete).toBe(false);
      }
    },
  );

  /**
   * Property: 删除后元素数量应该正确减少
   */
  test.prop([
    fc.integer({ min: 1, max: 100 }),
    fc.integer({ min: 1, max: 50 }),
  ])("删除后元素数量应该正确减少", (originalCount: unknown, deletedCount: unknown) => {
    const typedOriginalCount = originalCount as number;
    const typedDeletedCount = deletedCount as number;
    // 确保删除数量不超过原始数量
    fc.pre(typedDeletedCount <= typedOriginalCount);

    const resultCount = deleteUtils.calculateDeletedCount(
      typedOriginalCount,
      typedDeletedCount,
    );

    // 删除后数量 = 原始数量 - 删除数量
    expect(resultCount).toBe(typedOriginalCount - typedDeletedCount);

    // 结果应该非负
    expect(resultCount).toBeGreaterThanOrEqual(0);
  });

  /**
   * Property: 删除后被删除的元素不应该存在
   */
  test.prop([
    fc.array(fc.uuid(), { minLength: 1, maxLength: 20 }),
    fc.array(fc.uuid(), { minLength: 1, maxLength: 10 }),
  ])("删除后被删除的元素不应该存在", (originalElements: unknown, deletedElements: unknown) => {
    const typedOriginalElements = originalElements as string[];
    const typedDeletedElements = deletedElements as string[];
    // 确保删除的元素是原始元素的子集
    const validDeletedElements = typedDeletedElements.filter((id: string) =>
      typedOriginalElements.includes(id),
    );

    const remaining = deleteUtils.calculateRemainingElements(
      typedOriginalElements,
      validDeletedElements,
    );

    // 被删除的元素不应该在剩余列表中
    validDeletedElements.forEach((id: string) => {
      expect(deleteUtils.isDeleted(remaining, id)).toBe(true);
    });
  });

  /**
   * Property: 删除后未选中的元素应该保持不变
   */
  test.prop([fc.array(fc.uuid(), { minLength: 2, maxLength: 20 }), fc.nat()])(
    "删除后未选中的元素应该保持不变",
    (originalElements: unknown, deleteSeed: unknown) => {
      const typedOriginalElements = originalElements as string[];
      const typedDeleteSeed = deleteSeed as number;
      // 选择一部分元素删除
      const deleteCount = (typedDeleteSeed % (typedOriginalElements.length - 1)) + 1;
      const deletedElements = typedOriginalElements.slice(0, deleteCount);
      const remainingExpected = typedOriginalElements.slice(deleteCount);

      const remaining = deleteUtils.calculateRemainingElements(
        typedOriginalElements,
        deletedElements,
      );

      // 未删除的元素应该保留
      remainingExpected.forEach((id: string) => {
        expect(remaining).toContain(id);
      });
    },
  );

  /**
   * Property: 锁定的元素不应该被删除
   */
  test.prop([
    fc.array(fc.uuid(), { minLength: 2, maxLength: 20 }),
    fc.array(fc.uuid(), { minLength: 1, maxLength: 10 }),
  ])("锁定的元素不应该被删除", (selectedElements: unknown, lockedIds: unknown) => {
    const typedSelectedElements = selectedElements as string[];
    const typedLockedIds = lockedIds as string[];
    const deletable = deleteUtils.filterDeletableElements(
      typedSelectedElements,
      typedLockedIds,
    );

    // 锁定的元素不应该在可删除列表中
    typedLockedIds.forEach((id: string) => {
      expect(deletable).not.toContain(id);
    });

    // 未锁定的元素应该在可删除列表中
    typedSelectedElements.forEach((id: string) => {
      if (!typedLockedIds.includes(id)) {
        expect(deletable).toContain(id);
      }
    });
  });

  /**
   * Property: 删除后选择状态应该正确更新
   */
  test.prop([
    fc.array(fc.uuid(), { minLength: 1, maxLength: 10 }),
    fc.array(fc.uuid(), { minLength: 1, maxLength: 10 }),
  ])("删除后选择状态应该正确更新", (selectedIds: unknown, deletedIds: unknown) => {
    const typedSelectedIds = selectedIds as string[];
    const typedDeletedIds = deletedIds as string[];
    const remainingSelection = deleteUtils.calculateRemainingSelection(
      typedSelectedIds,
      typedDeletedIds,
    );

    // 被删除的元素不应该在选择中
    typedDeletedIds.forEach((id: string) => {
      expect(remainingSelection).not.toContain(id);
    });

    // 未删除的选中元素应该保留
    typedSelectedIds.forEach((id: string) => {
      if (!typedDeletedIds.includes(id)) {
        expect(remainingSelection).toContain(id);
      }
    });
  });

  /**
   * Property: 删除所有选中元素后选择应该为空
   */
  test.prop([fc.array(fc.uuid(), { minLength: 1, maxLength: 10 })])(
    "删除所有选中元素后选择应该为空",
    (selectedIds: unknown) => {
      const typedSelectedIds = selectedIds as string[];
      const remainingSelection = deleteUtils.calculateRemainingSelection(
        typedSelectedIds,
        typedSelectedIds,
      );

      expect(remainingSelection).toHaveLength(0);
    },
  );
});

/**
 * 单元测试 - 删除边界情况
 */
describe("删除边界情况", () => {
  it("0 个元素不能删除", () => {
    expect(deleteUtils.canDelete(0)).toBe(false);
  });

  it("1 个元素可以删除", () => {
    expect(deleteUtils.canDelete(1)).toBe(true);
  });

  it("删除后数量不能为负", () => {
    expect(deleteUtils.calculateDeletedCount(5, 10)).toBe(0);
  });

  it("空锁定列表不影响删除", () => {
    const elements = ["a", "b", "c"];
    const deletable = deleteUtils.filterDeletableElements(elements, []);
    expect(deletable).toEqual(elements);
  });

  it("全部锁定时无法删除任何元素", () => {
    const elements = ["a", "b", "c"];
    const deletable = deleteUtils.filterDeletableElements(elements, elements);
    expect(deletable).toHaveLength(0);
  });
});
