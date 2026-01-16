/**
 * @file usePageOperations Hook 属性测试
 * @description 测试页面操作功能的正确性属性
 * @module components/content-creator/canvas/poster/hooks/usePageOperations.test
 */

import { describe, it, expect } from "vitest";
import { test } from "@fast-check/vitest";
import * as fc from "fast-check";
import { pageUtils } from "./usePageOperations";
import type { PosterPage } from "../types";

/**
 * 生成有效的 PosterPage
 */
const posterPageArb = fc.record({
  id: fc.uuid(),
  name: fc.string({ minLength: 1, maxLength: 20 }),
  width: fc.integer({ min: 100, max: 2000 }),
  height: fc.integer({ min: 100, max: 2000 }),
  backgroundColor: fc.constantFrom(
    "#ffffff",
    "#f5f5f5",
    "#e0e0e0",
    "#333333",
    "#000000",
  ),
  layers: fc.array(
    fc.record({
      id: fc.uuid(),
      name: fc.string({ minLength: 1, maxLength: 20 }),
      type: fc.constantFrom("text", "image", "shape", "group") as fc.Arbitrary<
        "text" | "image" | "shape" | "group"
      >,
      visible: fc.boolean(),
      locked: fc.boolean(),
    }),
    { minLength: 0, maxLength: 5 },
  ),
}) as fc.Arbitrary<PosterPage>;

/**
 * 生成非空页面列表和有效的当前索引
 */
const pagesWithIndexArb = fc
  .array(posterPageArb, { minLength: 1, maxLength: 10 })
  .chain((pages) =>
    fc.record({
      pages: fc.constant(pages),
      currentIndex: fc.integer({ min: 0, max: pages.length - 1 }),
    }),
  );

/**
 * Feature: ai-content-creator-phase2, Property 9: 页面操作正确性
 * Validates: Requirements 8.2, 8.3, 8.4, 8.5, 8.6
 *
 * *For any* 多页海报，页面操作应该正确更新页面列表：
 * - 添加页面：页面数量 +1
 * - 删除页面：页面数量 -1，当前页面切换到相邻页面
 * - 复制页面：新页面内容与原页面相同
 * - 页面切换：当前编辑页面正确更新
 */
describe("Property 9: 页面操作正确性", () => {
  /**
   * Property: 添加页面后页面数量应该增加 1
   * Validates: Requirements 8.3
   */
  test.prop([pagesWithIndexArb])(
    "添加页面后页面数量应该增加 1",
    (input: unknown) => {
      const { pages, currentIndex } = input as { pages: any[]; currentIndex: number };
      const result = pageUtils.addPage(pages, currentIndex);

      // 页面数量应该增加 1
      expect(result.pages.length).toBe(pages.length + 1);

      // 新页面应该在当前页面之后
      expect(result.currentIndex).toBe(currentIndex + 1);

      // 新页面应该有唯一的 ID
      const newPage = result.pages[result.currentIndex];
      const originalIds = pages.map((p: any) => p.id);
      expect(originalIds).not.toContain(newPage.id);
    },
  );

  /**
   * Property: 添加页面应该继承当前页面的尺寸
   * Validates: Requirements 8.3
   */
  test.prop([pagesWithIndexArb])(
    "添加页面应该继承当前页面的尺寸",
    (input: unknown) => {
      const { pages, currentIndex } = input as { pages: any[]; currentIndex: number };
      const result = pageUtils.addPage(pages, currentIndex);
      const currentPage = pages[currentIndex];
      const newPage = result.pages[result.currentIndex];

      // 新页面尺寸应该与当前页面相同
      expect(newPage.width).toBe(currentPage.width);
      expect(newPage.height).toBe(currentPage.height);
    },
  );

  /**
   * Property: 删除页面后页面数量应该减少 1（多于一页时）
   * Validates: Requirements 8.4
   */
  test.prop([
    fc.array(posterPageArb, { minLength: 2, maxLength: 10 }).chain((pages: any[]) =>
      fc.record({
        pages: fc.constant(pages),
        currentIndex: fc.integer({ min: 0, max: pages.length - 1 }),
        deleteIndex: fc.integer({ min: 0, max: pages.length - 1 }),
      }),
    ),
  ])(
    "删除页面后页面数量应该减少 1（多于一页时）",
    (input: unknown) => {
      const { pages, currentIndex, deleteIndex } = input as { pages: any[]; currentIndex: number; deleteIndex: number };
      const result = pageUtils.deletePage(pages, currentIndex, deleteIndex);

      // 应该成功删除
      expect(result).not.toBeNull();
      if (result) {
        // 页面数量应该减少 1
        expect(result.pages.length).toBe(pages.length - 1);

        // 被删除的页面不应该存在
        const deletedPageId = pages[deleteIndex].id;
        expect(result.pages.map((p: any) => p.id)).not.toContain(deletedPageId);
      }
    },
  );

  /**
   * Property: 只有一页时不能删除
   * Validates: Requirements 8.4
   */
  test.prop([posterPageArb])("只有一页时不能删除", (page: unknown) => {
    const typedPage = page as any;
    const pages = [typedPage];
    const result = pageUtils.deletePage(pages, 0, 0);

    // 应该返回 null，表示无法删除
    expect(result).toBeNull();
  });

  /**
   * Property: 删除当前页面后应该切换到相邻页面
   * Validates: Requirements 8.4
   */
  test.prop([
    fc.array(posterPageArb, { minLength: 2, maxLength: 10 }).chain((pages: any[]) =>
      fc.record({
        pages: fc.constant(pages),
        currentIndex: fc.integer({ min: 0, max: pages.length - 1 }),
      }),
    ),
  ])("删除当前页面后应该切换到相邻页面", (input: unknown) => {
    const { pages, currentIndex } = input as { pages: any[]; currentIndex: number };
    const result = pageUtils.deletePage(pages, currentIndex, currentIndex);

    expect(result).not.toBeNull();
    if (result) {
      // 新的当前索引应该在有效范围内
      expect(result.currentIndex).toBeGreaterThanOrEqual(0);
      expect(result.currentIndex).toBeLessThan(result.pages.length);

      // 如果删除的不是最后一页，索引应该保持不变或减少
      if (currentIndex < pages.length - 1) {
        expect(result.currentIndex).toBeLessThanOrEqual(currentIndex);
      }
    }
  });

  /**
   * Property: 复制页面后页面数量应该增加 1
   * Validates: Requirements 8.5
   */
  test.prop([
    pagesWithIndexArb.chain(({ pages, currentIndex }) =>
      fc.record({
        pages: fc.constant(pages),
        currentIndex: fc.constant(currentIndex),
        copyIndex: fc.integer({ min: 0, max: pages.length - 1 }),
      }),
    ),
  ])("复制页面后页面数量应该增加 1", ({ pages, currentIndex, copyIndex }) => {
    const result = pageUtils.duplicatePage(pages, currentIndex, copyIndex);

    expect(result).not.toBeNull();
    if (result) {
      // 页面数量应该增加 1
      expect(result.pages.length).toBe(pages.length + 1);
    }
  });

  /**
   * Property: 复制的页面内容应该与原页面相同（除了 ID 和名称）
   * Validates: Requirements 8.5
   */
  test.prop([
    pagesWithIndexArb.chain(({ pages, currentIndex }) =>
      fc.record({
        pages: fc.constant(pages),
        currentIndex: fc.constant(currentIndex),
        copyIndex: fc.integer({ min: 0, max: pages.length - 1 }),
      }),
    ),
  ])(
    "复制的页面内容应该与原页面相同（除了 ID 和名称）",
    ({ pages, currentIndex, copyIndex }) => {
      const result = pageUtils.duplicatePage(pages, currentIndex, copyIndex);
      const sourcePage = pages[copyIndex];

      expect(result).not.toBeNull();
      if (result) {
        // 副本应该在源页面之后
        const duplicatedPage = result.pages[copyIndex + 1];

        // ID 应该不同
        expect(duplicatedPage.id).not.toBe(sourcePage.id);

        // 名称应该包含 "副本"
        expect(duplicatedPage.name).toContain("副本");

        // 尺寸应该相同
        expect(duplicatedPage.width).toBe(sourcePage.width);
        expect(duplicatedPage.height).toBe(sourcePage.height);

        // 背景颜色应该相同
        expect(duplicatedPage.backgroundColor).toBe(sourcePage.backgroundColor);

        // 图层数量应该相同
        expect(duplicatedPage.layers.length).toBe(sourcePage.layers.length);
      }
    },
  );

  /**
   * Property: 页面切换应该正确更新当前索引
   * Validates: Requirements 8.2
   */
  test.prop([
    pagesWithIndexArb.chain(({ pages }) =>
      fc.record({
        pages: fc.constant(pages),
        targetIndex: fc.integer({ min: 0, max: pages.length - 1 }),
      }),
    ),
  ])("页面切换应该正确更新当前索引", ({ pages, targetIndex }) => {
    const result = pageUtils.selectPage(pages, targetIndex);

    // 应该返回目标索引
    expect(result).toBe(targetIndex);
  });

  /**
   * Property: 无效的页面索引应该返回 null
   * Validates: Requirements 8.2
   */
  test.prop([
    pagesWithIndexArb,
    fc.oneof(
      fc.integer({ min: -100, max: -1 }),
      fc.integer({ min: 10, max: 100 }),
    ),
  ])("无效的页面索引应该返回 null", ({ pages }, invalidIndex) => {
    // 确保索引确实无效
    fc.pre(invalidIndex < 0 || invalidIndex >= pages.length);

    const result = pageUtils.selectPage(pages, invalidIndex);

    // 应该返回 null
    expect(result).toBeNull();
  });

  /**
   * Property: 页面重排序后页面数量应该保持不变
   * Validates: Requirements 8.6
   */
  test.prop([
    fc.array(posterPageArb, { minLength: 2, maxLength: 10 }).chain((pages) =>
      fc.record({
        pages: fc.constant(pages),
        currentIndex: fc.integer({ min: 0, max: pages.length - 1 }),
        fromIndex: fc.integer({ min: 0, max: pages.length - 1 }),
        toIndex: fc.integer({ min: 0, max: pages.length - 1 }),
      }),
    ),
  ])(
    "页面重排序后页面数量应该保持不变",
    ({ pages, currentIndex, fromIndex, toIndex }) => {
      const result = pageUtils.reorderPages(
        pages,
        currentIndex,
        fromIndex,
        toIndex,
      );

      expect(result).not.toBeNull();
      if (result) {
        // 页面数量应该保持不变
        expect(result.pages.length).toBe(pages.length);

        // 所有页面 ID 应该保持不变
        const originalIds = new Set(pages.map((p) => p.id));
        const resultIds = new Set(result.pages.map((p) => p.id));
        expect(resultIds).toEqual(originalIds);
      }
    },
  );

  /**
   * Property: 页面重排序应该正确移动页面
   * Validates: Requirements 8.6
   */
  test.prop([
    fc.array(posterPageArb, { minLength: 2, maxLength: 10 }).chain((pages) =>
      fc.record({
        pages: fc.constant(pages),
        currentIndex: fc.integer({ min: 0, max: pages.length - 1 }),
        fromIndex: fc.integer({ min: 0, max: pages.length - 1 }),
        toIndex: fc.integer({ min: 0, max: pages.length - 1 }),
      }),
    ),
  ])(
    "页面重排序应该正确移动页面",
    ({ pages, currentIndex, fromIndex, toIndex }) => {
      fc.pre(fromIndex !== toIndex); // 只测试实际移动的情况

      const result = pageUtils.reorderPages(
        pages,
        currentIndex,
        fromIndex,
        toIndex,
      );

      expect(result).not.toBeNull();
      if (result) {
        // 被移动的页面应该在新位置
        const movedPage = pages[fromIndex];
        expect(result.pages[toIndex].id).toBe(movedPage.id);
      }
    },
  );

  /**
   * Property: 移动当前页面时，当前索引应该跟随移动
   * Validates: Requirements 8.6
   */
  test.prop([
    fc.array(posterPageArb, { minLength: 2, maxLength: 10 }).chain((pages) =>
      fc
        .record({
          pages: fc.constant(pages),
          currentIndex: fc.integer({ min: 0, max: pages.length - 1 }),
        })
        .chain(({ pages, currentIndex }) =>
          fc.record({
            pages: fc.constant(pages),
            currentIndex: fc.constant(currentIndex),
            toIndex: fc.integer({ min: 0, max: pages.length - 1 }),
          }),
        ),
    ),
  ])(
    "移动当前页面时，当前索引应该跟随移动",
    ({ pages, currentIndex, toIndex }) => {
      fc.pre(currentIndex !== toIndex); // 只测试实际移动的情况

      const result = pageUtils.reorderPages(
        pages,
        currentIndex,
        currentIndex,
        toIndex,
      );

      expect(result).not.toBeNull();
      if (result) {
        // 当前索引应该更新为目标位置
        expect(result.currentIndex).toBe(toIndex);

        // 当前页面应该是原来的当前页面
        const originalCurrentPage = pages[currentIndex];
        expect(result.pages[result.currentIndex].id).toBe(
          originalCurrentPage.id,
        );
      }
    },
  );
});

/**
 * 单元测试 - 边界情况
 */
describe("页面操作边界情况", () => {
  const createTestPage = (id: string, name: string): PosterPage => ({
    id,
    name,
    width: 1080,
    height: 1080,
    backgroundColor: "#ffffff",
    layers: [],
  });

  it("添加页面到空列表应该创建第一页", () => {
    // 注意：实际上不应该有空列表，但测试边界情况
    const result = pageUtils.addPage([], -1);

    expect(result.pages.length).toBe(1);
    expect(result.currentIndex).toBe(0);
  });

  it("删除无效索引应该返回 null", () => {
    const pages = [
      createTestPage("1", "Page 1"),
      createTestPage("2", "Page 2"),
    ];

    expect(pageUtils.deletePage(pages, 0, -1)).toBeNull();
    expect(pageUtils.deletePage(pages, 0, 10)).toBeNull();
  });

  it("复制无效索引应该返回 null", () => {
    const pages = [createTestPage("1", "Page 1")];

    expect(pageUtils.duplicatePage(pages, 0, -1)).toBeNull();
    expect(pageUtils.duplicatePage(pages, 0, 10)).toBeNull();
  });

  it("重排序无效索引应该返回 null", () => {
    const pages = [
      createTestPage("1", "Page 1"),
      createTestPage("2", "Page 2"),
    ];

    expect(pageUtils.reorderPages(pages, 0, -1, 0)).toBeNull();
    expect(pageUtils.reorderPages(pages, 0, 0, -1)).toBeNull();
    expect(pageUtils.reorderPages(pages, 0, 10, 0)).toBeNull();
    expect(pageUtils.reorderPages(pages, 0, 0, 10)).toBeNull();
  });

  it("重排序相同位置应该返回原数组", () => {
    const pages = [
      createTestPage("1", "Page 1"),
      createTestPage("2", "Page 2"),
    ];

    const result = pageUtils.reorderPages(pages, 0, 1, 1);

    expect(result).not.toBeNull();
    if (result) {
      expect(result.pages).toBe(pages); // 应该是同一个引用
      expect(result.currentIndex).toBe(0);
    }
  });

  it("删除当前页面之前的页面应该更新当前索引", () => {
    const pages = [
      createTestPage("1", "Page 1"),
      createTestPage("2", "Page 2"),
      createTestPage("3", "Page 3"),
    ];

    // 当前在第 3 页（索引 2），删除第 1 页（索引 0）
    const result = pageUtils.deletePage(pages, 2, 0);

    expect(result).not.toBeNull();
    if (result) {
      expect(result.currentIndex).toBe(1); // 索引应该减 1
      expect(result.pages[result.currentIndex].id).toBe("3"); // 仍然是原来的页面
    }
  });

  it("删除当前页面之后的页面不应该改变当前索引", () => {
    const pages = [
      createTestPage("1", "Page 1"),
      createTestPage("2", "Page 2"),
      createTestPage("3", "Page 3"),
    ];

    // 当前在第 1 页（索引 0），删除第 3 页（索引 2）
    const result = pageUtils.deletePage(pages, 0, 2);

    expect(result).not.toBeNull();
    if (result) {
      expect(result.currentIndex).toBe(0); // 索引不变
      expect(result.pages[result.currentIndex].id).toBe("1"); // 仍然是原来的页面
    }
  });

  it("复制当前页面之前的页面应该更新当前索引", () => {
    const pages = [
      createTestPage("1", "Page 1"),
      createTestPage("2", "Page 2"),
    ];

    // 当前在第 2 页（索引 1），复制第 1 页（索引 0）
    const result = pageUtils.duplicatePage(pages, 1, 0);

    expect(result).not.toBeNull();
    if (result) {
      expect(result.currentIndex).toBe(2); // 索引应该加 1
      expect(result.pages[result.currentIndex].id).toBe("2"); // 仍然是原来的页面
    }
  });
});
