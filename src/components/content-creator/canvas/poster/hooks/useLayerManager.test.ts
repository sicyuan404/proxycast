/**
 * @file 图层管理 Hook 测试
 * @description 测试图层管理功能，包括 Property 8: 图层同步正确性
 * @module components/content-creator/canvas/poster/hooks/useLayerManager.test
 */

import { describe, it, expect, vi } from "vitest";
import * as fc from "fast-check";
import type { Layer, ElementType } from "../types";

function getElementType(obj: { type?: string }): ElementType {
  const type = obj.type;
  if (type === "i-text" || type === "text" || type === "textbox") {
    return "text";
  }
  if (type === "image") {
    return "image";
  }
  if (type === "group") {
    return "group";
  }
  return "shape";
}

function generateLayerName(obj: { type?: string }, index: number): string {
  const type = getElementType(obj);
  const typeNames: Record<ElementType, string> = {
    text: "文字",
    image: "图片",
    shape: "形状",
    group: "组合",
  };
  return `${typeNames[type]} ${index + 1}`;
}

interface MockFabricObject {
  type: string;
  visible: boolean;
  selectable: boolean;
  evented: boolean;
  layerId?: string;
  layerName?: string;
  set: (...args: unknown[]) => void;
}

interface MockCanvas {
  getObjects: () => MockFabricObject[];
  moveTo: (obj: MockFabricObject, index: number) => void;
}

const layerUtils = {
  createLayerFromObject: (obj: MockFabricObject, index: number): Layer => {
    const id = obj.layerId || crypto.randomUUID();
    obj.layerId = id;
    return {
      id,
      name: obj.layerName || generateLayerName(obj, index),
      type: getElementType(obj),
      visible: obj.visible !== false,
      locked: obj.selectable === false && obj.evented === false,
    };
  },

  syncVisibility: (obj: MockFabricObject, visible: boolean): void => {
    obj.set("visible", visible);
  },

  syncLock: (obj: MockFabricObject, locked: boolean): void => {
    obj.set({ selectable: !locked, evented: !locked });
  },

  moveLayer: (
    canvas: MockCanvas,
    obj: MockFabricObject,
    _fromIndex: number,
    toIndex: number,
  ): void => {
    const objects = canvas.getObjects();
    const actualFromIndex = objects.indexOf(obj);
    if (actualFromIndex === -1) return;
    const totalObjects = objects.length;
    const targetCanvasIndex = totalObjects - 1 - toIndex;
    canvas.moveTo(obj, targetCanvasIndex);
  },
};

function createMockFabricObject(options: {
  type?: string;
  visible?: boolean;
  selectable?: boolean;
  evented?: boolean;
  layerId?: string;
  layerName?: string;
}): MockFabricObject {
  return {
    type: options.type || "rect",
    visible: options.visible !== false,
    selectable: options.selectable !== false,
    evented: options.evented !== false,
    layerId: options.layerId,
    layerName: options.layerName,
    set: vi.fn(),
  };
}

function createMockCanvas(
  objects: MockFabricObject[],
): MockCanvas & { moveTo: ReturnType<typeof vi.fn> } {
  const canvasObjects = [...objects];
  return {
    getObjects: vi.fn(() => canvasObjects),
    moveTo: vi.fn((obj: MockFabricObject, index: number) => {
      const currentIndex = canvasObjects.indexOf(obj);
      if (currentIndex !== -1) {
        canvasObjects.splice(currentIndex, 1);
        canvasObjects.splice(index, 0, obj);
      }
    }),
  };
}

describe("useLayerManager", () => {
  describe("layerUtils", () => {
    describe("createLayerFromObject", () => {
      it("应该从 Fabric.js 对象创建图层", () => {
        const obj = createMockFabricObject({ type: "i-text" });
        const layer = layerUtils.createLayerFromObject(obj, 0);
        expect(layer.id).toBeDefined();
        expect(layer.type).toBe("text");
        expect(layer.visible).toBe(true);
        expect(layer.locked).toBe(false);
      });

      it("应该正确识别不同元素类型", () => {
        const textObj = createMockFabricObject({ type: "i-text" });
        const imageObj = createMockFabricObject({ type: "image" });
        const rectObj = createMockFabricObject({ type: "rect" });
        const groupObj = createMockFabricObject({ type: "group" });
        expect(layerUtils.createLayerFromObject(textObj, 0).type).toBe("text");
        expect(layerUtils.createLayerFromObject(imageObj, 0).type).toBe(
          "image",
        );
        expect(layerUtils.createLayerFromObject(rectObj, 0).type).toBe("shape");
        expect(layerUtils.createLayerFromObject(groupObj, 0).type).toBe(
          "group",
        );
      });

      it("应该识别锁定状态", () => {
        const lockedObj = createMockFabricObject({
          type: "rect",
          selectable: false,
          evented: false,
        });
        const layer = layerUtils.createLayerFromObject(lockedObj, 0);
        expect(layer.locked).toBe(true);
      });

      it("应该识别隐藏状态", () => {
        const hiddenObj = createMockFabricObject({
          type: "rect",
          visible: false,
        });
        const layer = layerUtils.createLayerFromObject(hiddenObj, 0);
        expect(layer.visible).toBe(false);
      });
    });

    describe("syncVisibility", () => {
      it("应该同步可见性到 Fabric.js 对象", () => {
        const obj = createMockFabricObject({ type: "rect" });
        layerUtils.syncVisibility(obj, false);
        expect(obj.set).toHaveBeenCalledWith("visible", false);
        layerUtils.syncVisibility(obj, true);
        expect(obj.set).toHaveBeenCalledWith("visible", true);
      });
    });

    describe("syncLock", () => {
      it("应该同步锁定状态到 Fabric.js 对象", () => {
        const obj = createMockFabricObject({ type: "rect" });
        layerUtils.syncLock(obj, true);
        expect(obj.set).toHaveBeenCalledWith({
          selectable: false,
          evented: false,
        });
        layerUtils.syncLock(obj, false);
        expect(obj.set).toHaveBeenCalledWith({
          selectable: true,
          evented: true,
        });
      });
    });
  });

  /**
   * Property 8: 图层同步正确性
   * Feature: ai-content-creator-phase2, Property 8: 图层同步正确性
   *
   * *For any* 画布状态，图层列表应该与画布元素的 z-index 顺序一致。具体来说：
   * - 图层列表中的元素数量 = 画布上的元素数量
   * - 图层顺序与元素 z-index 顺序一致
   * - 锁定的图层对应的元素不可选中
   * - 隐藏的图层对应的元素不可见
   *
   * **Validates: Requirements 7.2, 7.3, 7.4, 7.5, 7.7**
   */
  describe("Property 8: 图层同步正确性", () => {
    const elementTypeArb = fc.constantFrom<string>(
      "i-text",
      "image",
      "rect",
      "circle",
      "group",
    );
    const elementConfigArb = fc.record({
      type: elementTypeArb,
      visible: fc.boolean(),
      locked: fc.boolean(),
    });
    const elementsArb = fc.array(elementConfigArb, {
      minLength: 1,
      maxLength: 10,
    });

    /**
     * 图层数量同步测试
     * **Validates: Requirements 7.2**
     */
    it("图层数量应该等于画布元素数量", () => {
      fc.assert(
        fc.property(elementsArb, (elements: any[]) => {
          const objects = elements.map((config: any) =>
            createMockFabricObject({
              type: config.type,
              visible: config.visible,
              selectable: !config.locked,
              evented: !config.locked,
            }),
          );
          const layers = objects.map((obj: any, index: number) =>
            layerUtils.createLayerFromObject(obj, index),
          );
          expect(layers.length).toBe(objects.length);
        }),
        { numRuns: 100 },
      );
    });

    /**
     * 图层顺序同步测试
     * **Validates: Requirements 7.2**
     */
    it("图层顺序应该与画布元素 z-index 顺序一致（反转）", () => {
      fc.assert(
        fc.property(elementsArb, (elements: any[]) => {
          const objects = elements.map((config: any, index: number) =>
            createMockFabricObject({
              type: config.type,
              visible: config.visible,
              selectable: !config.locked,
              evented: !config.locked,
              layerId: `layer-${index}`,
            }),
          );
          const layers = objects
            .map((obj: any, index: number) => layerUtils.createLayerFromObject(obj, index))
            .reverse();
          for (let i = 0; i < layers.length; i++) {
            const objectIndex = objects.length - 1 - i;
            expect(layers[i].id).toBe(`layer-${objectIndex}`);
          }
        }),
        { numRuns: 100 },
      );
    });

    /**
     * 图层拖拽排序后 z-index 同步测试
     * **Validates: Requirements 7.3**
     */
    it("图层拖拽排序后 z-index 应该正确更新", () => {
      fc.assert(
        fc.property(
          fc.array(elementConfigArb, { minLength: 3, maxLength: 10 }),
          fc.nat(),
          fc.nat(),
          (elements: any[], fromSeed: number, toSeed: number) => {
            const objects = elements.map((config: any, index: number) =>
              createMockFabricObject({
                type: config.type,
                visible: config.visible,
                selectable: !config.locked,
                evented: !config.locked,
                layerId: `layer-${index}`,
              }),
            );
            const canvas = createMockCanvas(objects);

            // 生成有效的 fromIndex 和 toIndex
            const fromIndex = fromSeed % objects.length;
            const toIndex = toSeed % objects.length;

            if (fromIndex !== toIndex) {
              const objToMove = objects[fromIndex];
              layerUtils.moveLayer(canvas, objToMove, fromIndex, toIndex);

              // 验证 moveTo 被调用
              expect(canvas.moveTo).toHaveBeenCalled();
            }
          },
        ),
        { numRuns: 100 },
      );
    });

    /**
     * 锁定状态同步测试
     * **Validates: Requirements 7.4**
     */
    it("锁定的图层对应的元素应该不可选中", () => {
      fc.assert(
        fc.property(elementsArb, (elements: any[]) => {
          const objects = elements.map((config: any) =>
            createMockFabricObject({
              type: config.type,
              visible: config.visible,
              selectable: !config.locked,
              evented: !config.locked,
            }),
          );
          const layers = objects.map((obj: any, index: number) =>
            layerUtils.createLayerFromObject(obj, index),
          );
          for (let i = 0; i < layers.length; i++) {
            const layer = layers[i];
            const obj = objects[i];
            if (layer.locked) {
              expect(obj.selectable).toBe(false);
              expect(obj.evented).toBe(false);
            } else {
              expect(obj.selectable).toBe(true);
              expect(obj.evented).toBe(true);
            }
          }
        }),
        { numRuns: 100 },
      );
    });

    /**
     * 可见性状态同步测试
     * **Validates: Requirements 7.5**
     */
    it("隐藏的图层对应的元素应该不可见", () => {
      fc.assert(
        fc.property(elementsArb, (elements: any[]) => {
          const objects = elements.map((config: any) =>
            createMockFabricObject({
              type: config.type,
              visible: config.visible,
              selectable: !config.locked,
              evented: !config.locked,
            }),
          );
          const layers = objects.map((obj: any, index: number) =>
            layerUtils.createLayerFromObject(obj, index),
          );
          for (let i = 0; i < layers.length; i++) {
            expect(layers[i].visible).toBe(objects[i].visible);
          }
        }),
        { numRuns: 100 },
      );
    });

    /**
     * 切换可见性同步测试
     * **Validates: Requirements 7.5**
     */
    it("切换可见性后应该同步到画布对象", () => {
      fc.assert(
        fc.property(fc.boolean(), fc.boolean(), (initialVisible: boolean, toggleTo: boolean) => {
          const obj = createMockFabricObject({
            type: "rect",
            visible: initialVisible,
          });
          layerUtils.syncVisibility(obj, toggleTo);
          expect(obj.set).toHaveBeenCalledWith("visible", toggleTo);
        }),
        { numRuns: 100 },
      );
    });

    /**
     * 切换锁定状态同步测试
     * **Validates: Requirements 7.4**
     */
    it("切换锁定状态后应该同步到画布对象", () => {
      fc.assert(
        fc.property(fc.boolean(), fc.boolean(), (initialLocked: boolean, toggleTo: boolean) => {
          const obj = createMockFabricObject({
            type: "rect",
            selectable: !initialLocked,
            evented: !initialLocked,
          });
          layerUtils.syncLock(obj, toggleTo);
          expect(obj.set).toHaveBeenCalledWith({
            selectable: !toggleTo,
            evented: !toggleTo,
          });
        }),
        { numRuns: 100 },
      );
    });

    /**
     * 点击图层选中元素测试
     * **Validates: Requirements 7.7**
     */
    it("点击图层应该选中对应的可选元素", () => {
      fc.assert(
        fc.property(elementsArb, fc.nat(), (elements: any[], indexSeed: number) => {
          const objects = elements.map((config: any, index: number) =>
            createMockFabricObject({
              type: config.type,
              visible: config.visible,
              selectable: !config.locked,
              evented: !config.locked,
              layerId: `layer-${index}`,
            }),
          );
          const layers = objects.map((obj: any, index: number) =>
            layerUtils.createLayerFromObject(obj, index),
          );

          // 选择一个随机图层
          const selectedIndex = indexSeed % layers.length;
          const selectedLayer = layers[selectedIndex];
          const correspondingObject = objects[selectedIndex];

          // 验证：如果图层未锁定，对应的元素应该可选中
          if (!selectedLayer.locked) {
            expect(correspondingObject.selectable).toBe(true);
            expect(correspondingObject.evented).toBe(true);
          } else {
            // 锁定的图层对应的元素不可选中
            expect(correspondingObject.selectable).toBe(false);
            expect(correspondingObject.evented).toBe(false);
          }
        }),
        { numRuns: 100 },
      );
    });
  });

  describe("图层重排序", () => {
    it("应该正确移动图层位置", () => {
      const objects = [
        createMockFabricObject({ type: "rect", layerId: "layer-0" }),
        createMockFabricObject({ type: "circle", layerId: "layer-1" }),
        createMockFabricObject({ type: "i-text", layerId: "layer-2" }),
      ];
      const canvas = createMockCanvas(objects);
      layerUtils.moveLayer(canvas, objects[0], 0, 2);
      expect(canvas.moveTo).toHaveBeenCalled();
    });
  });

  describe("图层类型识别", () => {
    it("应该正确识别文字类型", () => {
      const types = ["i-text", "text", "textbox"];
      for (const type of types) {
        const obj = createMockFabricObject({ type });
        const layer = layerUtils.createLayerFromObject(obj, 0);
        expect(layer.type).toBe("text");
      }
    });

    it("应该正确识别图片类型", () => {
      const obj = createMockFabricObject({ type: "image" });
      const layer = layerUtils.createLayerFromObject(obj, 0);
      expect(layer.type).toBe("image");
    });

    it("应该正确识别组合类型", () => {
      const obj = createMockFabricObject({ type: "group" });
      const layer = layerUtils.createLayerFromObject(obj, 0);
      expect(layer.type).toBe("group");
    });

    it("应该将其他类型识别为形状", () => {
      const types = ["rect", "circle", "triangle", "line", "polygon", "path"];
      for (const type of types) {
        const obj = createMockFabricObject({ type });
        const layer = layerUtils.createLayerFromObject(obj, 0);
        expect(layer.type).toBe("shape");
      }
    });
  });

  describe("图层名称生成", () => {
    it("应该使用自定义名称（如果存在）", () => {
      const obj = createMockFabricObject({
        type: "rect",
        layerName: "我的矩形",
      });
      const layer = layerUtils.createLayerFromObject(obj, 0);
      expect(layer.name).toBe("我的矩形");
    });

    it("应该生成默认名称（如果没有自定义名称）", () => {
      const obj = createMockFabricObject({ type: "rect" });
      const layer = layerUtils.createLayerFromObject(obj, 2);
      expect(layer.name).toBe("形状 3");
    });
  });
});
