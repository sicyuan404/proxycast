/**
 * @file 导出功能属性测试
 * @description 测试导出功能的正确性，包括 Property 10: 导出尺寸正确性
 * @module components/content-creator/canvas/poster/hooks/useExport.test
 */

import { describe, it, expect } from "vitest";
import { test } from "@fast-check/vitest";
import * as fc from "fast-check";
import { exportUtils } from "./useExport";
import type { ExportConfig, ExportFormat } from "../types";

/**
 * 生成有效的导出格式
 */
const exportFormatArb = fc.constantFrom<ExportFormat>("png", "jpg");

/**
 * 生成有效的导出倍率
 */
const exportScaleArb = fc.constantFrom<1 | 2 | 3>(1, 2, 3);

/**
 * 生成有效的质量值（60-100）
 */
const qualityArb = fc.integer({ min: 60, max: 100 });

/**
 * 生成有效的页面尺寸
 */
const pageSizeArb = fc.record({
  width: fc.integer({ min: 100, max: 4000 }),
  height: fc.integer({ min: 100, max: 4000 }),
});

/**
 * 生成有效的页面
 */
const pageArb = fc
  .record({
    width: fc.integer({ min: 100, max: 4000 }),
    height: fc.integer({ min: 100, max: 4000 }),
    name: fc.string({ minLength: 1, maxLength: 20 }),
  })
  .map(({ width, height, name }: { width: number; height: number; name: string }) => ({
    id: crypto.randomUUID(),
    name: name || "页面",
    width,
    height,
    backgroundColor: "#ffffff",
    layers: [],
  }));

/**
 * 生成有效的页面列表
 */
const _pagesArb = fc.array(pageArb, { minLength: 1, maxLength: 10 });

/**
 * 生成有效的导出配置
 */
const _exportConfigArb = (maxPages: number) =>
  fc.record({
    format: exportFormatArb,
    quality: qualityArb,
    scale: exportScaleArb,
    pageIndices: fc.array(
      fc.integer({ min: 0, max: Math.max(0, maxPages - 1) }),
      {
        minLength: 1,
        maxLength: maxPages,
      },
    ),
  });

/**
 * Feature: ai-content-creator-phase2, Property 10: 导出尺寸正确性
 * Validates: Requirements 10.4
 *
 * *For any* 导出配置，导出图片的尺寸应该等于 原始尺寸 × 倍率。
 */
describe("Property 10: 导出尺寸正确性", () => {
  /**
   * Property: 导出尺寸应该等于原始尺寸乘以倍率
   * Validates: Requirements 10.4
   */
  test.prop([pageSizeArb, exportScaleArb])(
    "导出尺寸应该等于原始尺寸乘以倍率",
    (pageSize: unknown, scale: unknown) => {
      const { width, height } = pageSize as { width: number; height: number };
      const typedScale = scale as 1 | 2 | 3;
      const result = exportUtils.calculateExportSize(width, height, typedScale);

      expect(result.width).toBe(width * typedScale);
      expect(result.height).toBe(height * typedScale);
    },
  );

  /**
   * Property: 1x 倍率导出尺寸等于原始尺寸
   * Validates: Requirements 10.4
   */
  test.prop([pageSizeArb])(
    "1x 倍率导出尺寸等于原始尺寸",
    (pageSize: unknown) => {
      const { width, height } = pageSize as { width: number; height: number };
      const result = exportUtils.calculateExportSize(width, height, 1);

      expect(result.width).toBe(width);
      expect(result.height).toBe(height);
    },
  );

  /**
   * Property: 2x 倍率导出尺寸是原始尺寸的两倍
   * Validates: Requirements 10.4
   */
  test.prop([pageSizeArb])(
    "2x 倍率导出尺寸是原始尺寸的两倍",
    (pageSize: unknown) => {
      const { width, height } = pageSize as { width: number; height: number };
      const result = exportUtils.calculateExportSize(width, height, 2);

      expect(result.width).toBe(width * 2);
      expect(result.height).toBe(height * 2);
    },
  );

  /**
   * Property: 3x 倍率导出尺寸是原始尺寸的三倍
   * Validates: Requirements 10.4
   */
  test.prop([pageSizeArb])(
    "3x 倍率导出尺寸是原始尺寸的三倍",
    (pageSize: unknown) => {
      const { width, height } = pageSize as { width: number; height: number };
      const result = exportUtils.calculateExportSize(width, height, 3);

      expect(result.width).toBe(width * 3);
      expect(result.height).toBe(height * 3);
    },
  );

  /**
   * Property: 导出尺寸始终为正整数
   * Validates: Requirements 10.4
   */
  test.prop([pageSizeArb, exportScaleArb])(
    "导出尺寸始终为正整数",
    (pageSize: unknown, scale: unknown) => {
      const { width, height } = pageSize as { width: number; height: number };
      const typedScale = scale as 1 | 2 | 3;
      const result = exportUtils.calculateExportSize(width, height, typedScale);

      expect(result.width).toBeGreaterThan(0);
      expect(result.height).toBeGreaterThan(0);
      expect(Number.isInteger(result.width)).toBe(true);
      expect(Number.isInteger(result.height)).toBe(true);
    },
  );

  /**
   * Property: 宽高比在导出后保持不变
   * Validates: Requirements 10.4
   */
  test.prop([pageSizeArb, exportScaleArb])(
    "宽高比在导出后保持不变",
    (pageSize: unknown, scale: unknown) => {
      const { width, height } = pageSize as { width: number; height: number };
      const typedScale = scale as 1 | 2 | 3;
      const result = exportUtils.calculateExportSize(width, height, typedScale);
      const originalRatio = width / height;
      const exportRatio = result.width / result.height;

      expect(exportRatio).toBeCloseTo(originalRatio, 10);
    },
  );
});

/**
 * 导出配置验证测试
 */
describe("导出配置验证", () => {
  /**
   * Property: 有效配置应该通过验证
   */
  test.prop([
    exportFormatArb,
    qualityArb,
    exportScaleArb,
    fc.integer({ min: 1, max: 10 }),
  ])("有效配置应该通过验证", (format: unknown, quality: unknown, scale: unknown, pageCount: unknown) => {
    const typedFormat = format as ExportFormat;
    const typedQuality = quality as number;
    const typedScale = scale as 1 | 2 | 3;
    const typedPageCount = pageCount as number;
    const config: ExportConfig = {
      format: typedFormat,
      quality: typedQuality,
      scale: typedScale,
      pageIndices: Array.from({ length: typedPageCount }, (_, i) => i),
    };

    const result = exportUtils.validateConfig(config);

    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  /**
   * Property: 空页面索引应该验证失败
   */
  test.prop([exportFormatArb, qualityArb, exportScaleArb])(
    "空页面索引应该验证失败",
    (format: unknown, quality: unknown, scale: unknown) => {
      const typedFormat = format as ExportFormat;
      const typedQuality = quality as number;
      const typedScale = scale as 1 | 2 | 3;
      const config: ExportConfig = {
        format: typedFormat,
        quality: typedQuality,
        scale: typedScale,
        pageIndices: [],
      };

      const result = exportUtils.validateConfig(config);

      expect(result.valid).toBe(false);
      expect(result.errors).toContain("至少选择一个页面");
    },
  );

  /**
   * Property: 质量超出范围应该验证失败
   */
  test.prop([
    exportFormatArb,
    fc.oneof(
      fc.integer({ min: 0, max: 59 }),
      fc.integer({ min: 101, max: 200 }),
    ),
    exportScaleArb,
  ])("质量超出范围应该验证失败", (format: unknown, invalidQuality: unknown, scale: unknown) => {
    const typedFormat = format as ExportFormat;
    const typedInvalidQuality = invalidQuality as number;
    const typedScale = scale as 1 | 2 | 3;
    const config: ExportConfig = {
      format: typedFormat,
      quality: typedInvalidQuality,
      scale: typedScale,
      pageIndices: [0],
    };

    const result = exportUtils.validateConfig(config);

    expect(result.valid).toBe(false);
    expect(result.errors).toContain("质量必须在 60-100 之间");
  });

  /**
   * Property: 页面索引验证应该检测无效索引
   */
  test.prop([
    fc.array(fc.integer({ min: 0, max: 20 }), { minLength: 1, maxLength: 5 }),
    fc.integer({ min: 1, max: 10 }),
  ])("页面索引验证应该检测无效索引", (pageIndices: unknown, totalPages: unknown) => {
    const typedPageIndices = pageIndices as number[];
    const typedTotalPages = totalPages as number;
    const result = exportUtils.validatePageIndices(typedPageIndices, typedTotalPages);

    const expectedInvalid = typedPageIndices.filter((i: number) => i < 0 || i >= typedTotalPages);

    expect(result.valid).toBe(expectedInvalid.length === 0);
    expect(result.invalidIndices).toEqual(expectedInvalid);
  });
});

/**
 * 文件名生成测试
 */
describe("文件名生成", () => {
  /**
   * Property: 文件名应该包含页面名称
   */
  test.prop([fc.string({ minLength: 1, maxLength: 20 }), exportFormatArb])(
    "文件名应该包含页面名称",
    (pageName: unknown, format: unknown) => {
      const typedPageName = pageName as string;
      const typedFormat = format as ExportFormat;
      // 过滤掉可能导致问题的字符
      const safeName =
        typedPageName.replace(/[^a-zA-Z0-9\u4e00-\u9fa5]/g, "") || "页面";
      const fileName = exportUtils.generateFileName(safeName, typedFormat);

      expect(fileName).toContain(safeName);
    },
  );

  /**
   * Property: 文件名应该以正确的扩展名结尾
   */
  test.prop([fc.string({ minLength: 1, maxLength: 20 }), exportFormatArb])(
    "文件名应该以正确的扩展名结尾",
    (pageName: unknown, format: unknown) => {
      const typedPageName = pageName as string;
      const typedFormat = format as ExportFormat;
      const fileName = exportUtils.generateFileName(typedPageName, typedFormat);

      expect(fileName.endsWith(`.${typedFormat}`)).toBe(true);
    },
  );

  /**
   * Property: 文件名应该包含时间戳
   */
  test.prop([fc.string({ minLength: 1, maxLength: 20 }), exportFormatArb])(
    "文件名应该包含时间戳",
    (pageName: unknown, format: unknown) => {
      const typedPageName = pageName as string;
      const typedFormat = format as ExportFormat;
      const timestamp = new Date("2025-01-11T12:00:00Z");
      const fileName = exportUtils.generateFileName(
        typedPageName,
        typedFormat,
        timestamp,
      );

      // 时间戳格式: 2025-01-11T12-00-00
      expect(fileName).toContain("2025-01-11T12-00-00");
    },
  );
});

/**
 * MIME 类型测试
 */
describe("MIME 类型", () => {
  it("PNG 格式应该返回 image/png", () => {
    expect(exportUtils.getMimeType("png")).toBe("image/png");
  });

  it("JPG 格式应该返回 image/jpeg", () => {
    expect(exportUtils.getMimeType("jpg")).toBe("image/jpeg");
  });

  /**
   * Property: MIME 类型应该与格式对应
   */
  test.prop([exportFormatArb])("MIME 类型应该与格式对应", (format) => {
    const mimeType = exportUtils.getMimeType(format);

    if (format === "png") {
      expect(mimeType).toBe("image/png");
    } else {
      expect(mimeType).toBe("image/jpeg");
    }
  });
});

/**
 * 默认配置测试
 */
describe("默认配置", () => {
  it("默认配置应该有效", () => {
    const result = exportUtils.validateConfig(exportUtils.DEFAULT_CONFIG);

    expect(result.valid).toBe(true);
  });

  it("默认格式应该是 PNG", () => {
    expect(exportUtils.DEFAULT_CONFIG.format).toBe("png");
  });

  it("默认质量应该是 100", () => {
    expect(exportUtils.DEFAULT_CONFIG.quality).toBe(100);
  });

  it("默认倍率应该是 1x", () => {
    expect(exportUtils.DEFAULT_CONFIG.scale).toBe(1);
  });

  it("默认应该导出第一页", () => {
    expect(exportUtils.DEFAULT_CONFIG.pageIndices).toEqual([0]);
  });
});
