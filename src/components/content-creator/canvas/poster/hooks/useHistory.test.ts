/**
 * @file useHistory Hook 属性测试
 * @description 测试撤销/重做功能的正确性属性
 * @module components/content-creator/canvas/poster/hooks/useHistory.test
 */

import { describe, it, expect } from "vitest";
import { test } from "@fast-check/vitest";
import * as fc from "fast-check";
import { historyUtils } from "./useHistory";

/**
 * Feature: ai-content-creator-phase2, Property 3: 撤销/重做往返
 * Validates: Requirements 2.7
 *
 * *For any* 画布操作序列，执行撤销后画布状态应该恢复到操作前的状态。
 * 执行重做后应该恢复到撤销前的状态。
 * 这是一个往返属性：`undo(do(state, action)) == state`。
 */
describe("Property 3: 撤销/重做往返", () => {
  /**
   * Property: 撤销后重做应该恢复到撤销前的状态
   * redo(undo(state)) == state
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 2,
      maxLength: 20,
    }),
    fc.integer({ min: 1, max: 19 }),
  ])("撤销后重做应该恢复到撤销前的状态", (stack: unknown, indexOffset: unknown) => {
    const typedStack = stack as string[];
    const typedIndexOffset = indexOffset as number;
    // 确保索引在有效范围内
    const currentIndex = Math.min(typedIndexOffset, typedStack.length - 1);
    fc.pre(currentIndex > 0); // 必须能撤销

    // 执行撤销
    const afterUndo = historyUtils.simulateUndo(typedStack, currentIndex);

    // 确保撤销成功
    fc.pre(afterUndo.state !== null);

    // 执行重做
    const afterRedo = historyUtils.simulateRedo(typedStack, afterUndo.currentIndex);

    // 重做后应该恢复到撤销前的索引
    expect(afterRedo.currentIndex).toBe(currentIndex);

    // 重做后应该恢复到撤销前的状态
    expect(afterRedo.state).toBe(typedStack[currentIndex]);
  });

  /**
   * Property: 添加状态后撤销应该恢复到添加前的状态
   * undo(addState(state, newState)) == state
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 1,
      maxLength: 20,
    }),
    fc.string({ minLength: 1, maxLength: 100 }),
    fc.integer({ min: 10, max: 100 }),
  ])(
    "添加状态后撤销应该恢复到添加前的状态",
    (initialStack: unknown, newState: unknown, maxHistory: unknown) => {
      const typedInitialStack = initialStack as string[];
      const typedNewState = newState as string;
      const typedMaxHistory = maxHistory as number;
      // 初始索引在栈顶
      const initialIndex = typedInitialStack.length - 1;

      // 添加新状态
      const afterAdd = historyUtils.addState(
        typedInitialStack,
        initialIndex,
        typedNewState,
        typedMaxHistory,
      );

      // 执行撤销
      const afterUndo = historyUtils.simulateUndo(
        afterAdd.stack,
        afterAdd.currentIndex,
      );

      // 撤销后应该恢复到添加前的状态
      expect(afterUndo.state).toBe(typedInitialStack[initialIndex]);
    },
  );

  /**
   * Property: 连续撤销然后连续重做应该恢复到原始状态
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 3,
      maxLength: 20,
    }),
    fc.integer({ min: 1, max: 10 }),
  ])("连续撤销然后连续重做应该恢复到原始状态", (stack: unknown, undoCount: unknown) => {
    const typedStack = stack as string[];
    const typedUndoCount = undoCount as number;
    // 从栈顶开始
    const initialIndex = typedStack.length - 1;

    // 确保撤销次数不超过可撤销的次数
    const actualUndoCount = Math.min(typedUndoCount, initialIndex);
    fc.pre(actualUndoCount > 0);

    // 连续撤销
    let currentIndex = initialIndex;
    for (let i = 0; i < actualUndoCount; i++) {
      const result = historyUtils.simulateUndo(typedStack, currentIndex);
      currentIndex = result.currentIndex;
    }

    // 连续重做相同次数
    for (let i = 0; i < actualUndoCount; i++) {
      const result = historyUtils.simulateRedo(typedStack, currentIndex);
      currentIndex = result.currentIndex;
    }

    // 应该恢复到原始索引
    expect(currentIndex).toBe(initialIndex);
  });

  /**
   * Property: 撤销操作应该正确减少索引
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 2,
      maxLength: 20,
    }),
    fc.integer({ min: 1, max: 19 }),
  ])("撤销操作应该将索引减少 1", (stack: unknown, indexOffset: unknown) => {
    const typedStack = stack as string[];
    const typedIndexOffset = indexOffset as number;
    const currentIndex = Math.min(typedIndexOffset, typedStack.length - 1);
    fc.pre(currentIndex > 0); // 必须能撤销

    const result = historyUtils.simulateUndo(typedStack, currentIndex);

    expect(result.currentIndex).toBe(currentIndex - 1);
  });

  /**
   * Property: 重做操作应该正确增加索引
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 2,
      maxLength: 20,
    }),
    fc.integer({ min: 0, max: 18 }),
  ])("重做操作应该将索引增加 1", (stack: unknown, indexOffset: unknown) => {
    const typedStack = stack as string[];
    const typedIndexOffset = indexOffset as number;
    const currentIndex = Math.min(typedIndexOffset, typedStack.length - 2);
    fc.pre(currentIndex < typedStack.length - 1); // 必须能重做

    const result = historyUtils.simulateRedo(typedStack, currentIndex);

    expect(result.currentIndex).toBe(currentIndex + 1);
  });

  /**
   * Property: 在栈底时撤销应该无效
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 1,
      maxLength: 20,
    }),
  ])("在栈底时撤销应该无效", (stack: unknown) => {
    const typedStack = stack as string[];
    const result = historyUtils.simulateUndo(typedStack, 0);

    // 索引应该保持不变
    expect(result.currentIndex).toBe(0);

    // 状态应该为 null（表示无法撤销）
    expect(result.state).toBeNull();
  });

  /**
   * Property: 在栈顶时重做应该无效
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 1,
      maxLength: 20,
    }),
  ])("在栈顶时重做应该无效", (stack: unknown) => {
    const typedStack = stack as string[];
    const topIndex = typedStack.length - 1;
    const result = historyUtils.simulateRedo(typedStack, topIndex);

    // 索引应该保持不变
    expect(result.currentIndex).toBe(topIndex);

    // 状态应该为 null（表示无法重做）
    expect(result.state).toBeNull();
  });

  /**
   * Property: canUndo 和 canRedo 应该正确反映可操作性
   */
  test.prop([
    fc.integer({ min: 0, max: 100 }),
    fc.integer({ min: 1, max: 100 }),
  ])("canUndo 和 canRedo 应该正确反映可操作性", (currentIndex: unknown, stackLength: unknown) => {
    const typedCurrentIndex = currentIndex as number;
    const typedStackLength = stackLength as number;
    fc.pre(typedCurrentIndex < typedStackLength);

    const canUndo = historyUtils.canUndo(typedCurrentIndex);
    const canRedo = historyUtils.canRedo(typedCurrentIndex, typedStackLength);

    // canUndo 应该在索引 > 0 时为 true
    expect(canUndo).toBe(typedCurrentIndex > 0);

    // canRedo 应该在索引 < stackLength - 1 时为 true
    expect(canRedo).toBe(typedCurrentIndex < typedStackLength - 1);
  });

  /**
   * Property: 添加状态后应该清除重做栈
   */
  test.prop([
    fc.array(fc.string({ minLength: 1, maxLength: 100 }), {
      minLength: 3,
      maxLength: 20,
    }),
    fc.string({ minLength: 1, maxLength: 100 }),
    fc.integer({ min: 10, max: 100 }),
  ])("添加状态后应该清除重做栈", (stack: unknown, newState: unknown, maxHistory: unknown) => {
    const typedStack = stack as string[];
    const typedNewState = newState as string;
    const typedMaxHistory = maxHistory as number;
    // 从中间位置添加（模拟撤销后添加新状态）
    const middleIndex = Math.floor(typedStack.length / 2);
    fc.pre(middleIndex > 0 && middleIndex < typedStack.length - 1);

    const result = historyUtils.addState(
      typedStack,
      middleIndex,
      typedNewState,
      typedMaxHistory,
    );

    // 新栈长度应该是 middleIndex + 2（原来的 0 到 middleIndex，加上新状态）
    expect(result.stack.length).toBe(middleIndex + 2);

    // 新索引应该在栈顶
    expect(result.currentIndex).toBe(result.stack.length - 1);

    // 新状态应该在栈顶
    expect(result.stack[result.currentIndex]).toBe(typedNewState);
  });

  /**
   * Property: 历史记录应该受 maxHistory 限制
   */
  test.prop([fc.integer({ min: 5, max: 20 }), fc.integer({ min: 1, max: 10 })])(
    "历史记录应该受 maxHistory 限制",
    (maxHistory: unknown, extraStates: unknown) => {
      const typedMaxHistory = maxHistory as number;
      const typedExtraStates = extraStates as number;
      let stack: string[] = [];
      let currentIndex = -1;

      // 添加超过 maxHistory 的状态
      const totalStates = typedMaxHistory + typedExtraStates;
      for (let i = 0; i < totalStates; i++) {
        const result = historyUtils.addState(
          stack,
          currentIndex,
          `state-${i}`,
          typedMaxHistory,
        );
        stack = result.stack;
        currentIndex = result.currentIndex;
      }

      // 栈长度不应该超过 maxHistory
      expect(stack.length).toBeLessThanOrEqual(typedMaxHistory);

      // 最新的状态应该保留
      expect(stack[stack.length - 1]).toBe(`state-${totalStates - 1}`);
    },
  );
});

/**
 * 单元测试 - 边界情况
 */
describe("撤销/重做边界情况", () => {
  it("空栈时 canUndo 应该返回 false", () => {
    expect(historyUtils.canUndo(-1)).toBe(false);
    expect(historyUtils.canUndo(0)).toBe(false);
  });

  it("空栈时 canRedo 应该返回 false", () => {
    expect(historyUtils.canRedo(-1, 0)).toBe(false);
    expect(historyUtils.canRedo(0, 1)).toBe(false);
  });

  it("单状态栈时不能撤销也不能重做", () => {
    expect(historyUtils.canUndo(0)).toBe(false);
    expect(historyUtils.canRedo(0, 1)).toBe(false);
  });

  it("两状态栈在索引 1 时可以撤销但不能重做", () => {
    expect(historyUtils.canUndo(1)).toBe(true);
    expect(historyUtils.canRedo(1, 2)).toBe(false);
  });

  it("两状态栈在索引 0 时不能撤销但可以重做", () => {
    expect(historyUtils.canUndo(0)).toBe(false);
    expect(historyUtils.canRedo(0, 2)).toBe(true);
  });

  it("calculateUndoIndex 应该正确计算", () => {
    expect(historyUtils.calculateUndoIndex(5)).toBe(4);
    expect(historyUtils.calculateUndoIndex(1)).toBe(0);
    expect(historyUtils.calculateUndoIndex(0)).toBe(0);
  });

  it("calculateRedoIndex 应该正确计算", () => {
    expect(historyUtils.calculateRedoIndex(0, 5)).toBe(1);
    expect(historyUtils.calculateRedoIndex(3, 5)).toBe(4);
    expect(historyUtils.calculateRedoIndex(4, 5)).toBe(4);
  });

  it("addState 应该正确处理空栈", () => {
    const result = historyUtils.addState([], -1, "first-state", 50);

    expect(result.stack).toEqual(["first-state"]);
    expect(result.currentIndex).toBe(0);
  });

  it("addState 应该正确处理从中间添加", () => {
    const stack = ["state-0", "state-1", "state-2", "state-3"];
    const result = historyUtils.addState(stack, 1, "new-state", 50);

    expect(result.stack).toEqual(["state-0", "state-1", "new-state"]);
    expect(result.currentIndex).toBe(2);
  });
});
