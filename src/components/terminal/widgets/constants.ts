/**
 * 小部件系统常量配置
 *
 * 定义默认小部件配置、颜色、图表元数据等
 *
 * @module widgets/constants
 */

import {
  WidgetConfig,
  TimeSeriesMeta,
  PlotType,
  SysinfoDataPoint,
} from "./types";

/**
 * 默认小部件配置
 */
export const DEFAULT_WIDGETS: WidgetConfig[] = [
  {
    id: "ai",
    label: "AI",
    icon: "Sparkles",
    color: "var(--warning-color, #F59E0B)",
    description: "Terminal AI 助手",
    displayOrder: 0,
    type: "ai",
  },
  {
    id: "terminal",
    label: "终端",
    icon: "Terminal",
    color: "var(--accent-color)",
    description: "打开终端",
    displayOrder: 1,
    type: "terminal",
  },
  {
    id: "files",
    label: "文件",
    icon: "FolderOpen",
    color: "var(--sysinfo-mem-color, #FFC107)",
    description: "浏览文件",
    displayOrder: 2,
    type: "files",
  },
  {
    id: "web",
    label: "浏览器",
    icon: "Globe",
    color: "var(--info-color, #2196F3)",
    description: "内嵌浏览器",
    displayOrder: 3,
    type: "web",
  },
  {
    id: "sysinfo",
    label: "系统",
    icon: "Activity",
    color: "var(--sysinfo-cpu-color, #58C142)",
    description: "系统监控",
    displayOrder: 4,
    type: "sysinfo",
  },
];

/**
 * 设置菜单项配置
 */
export const SETTINGS_MENU_ITEMS = [
  {
    id: "settings",
    label: "设置",
    icon: "Settings",
    type: "settings" as const,
  },
  { id: "tips", label: "提示", icon: "Lightbulb", type: "tips" as const },
  { id: "secrets", label: "密钥", icon: "Lock", type: "secrets" as const },
  { id: "help", label: "帮助", icon: "HelpCircle", type: "help" as const },
];

/**
 * 小部件栏宽度（像素）
 */
export const WIDGETS_SIDEBAR_WIDTH = 48;

/**
 * 模式切换缓冲区（像素）
 * 防止在边界值附近频繁切换
 */
export const MODE_SWITCH_GRACE_PERIOD = 10;

/**
 * 每个小部件的最小高度（像素）
 * 用于计算 supercompact 模式切换阈值
 */
export const MIN_HEIGHT_PER_WIDGET = 32;

/**
 * 系统信息数据点数量
 * 120 点 = 2 分钟历史（每秒 1 点）
 */
export const DEFAULT_NUM_POINTS = 120;

/**
 * 系统信息更新间隔（毫秒）
 */
export const SYSINFO_UPDATE_INTERVAL = 1000;

/**
 * 默认 CPU 图表元数据
 */
export function defaultCpuMeta(name: string): TimeSeriesMeta {
  return {
    name,
    label: "%",
    miny: 0,
    maxy: 100,
    color: "var(--sysinfo-cpu-color, #58C142)",
    decimalPlaces: 0,
  };
}

/**
 * 默认内存图表元数据
 */
export function defaultMemMeta(name: string, maxY: string): TimeSeriesMeta {
  return {
    name,
    label: "GB",
    miny: 0,
    maxy: maxY,
    color: "var(--sysinfo-mem-color, #FFC107)",
    decimalPlaces: 1,
  };
}

/**
 * 默认图表元数据映射
 */
export const DEFAULT_PLOT_META: Record<string, TimeSeriesMeta> = {
  cpu: defaultCpuMeta("CPU %"),
  "mem:total": defaultMemMeta("内存总量", "mem:total"),
  "mem:used": defaultMemMeta("已用内存", "mem:total"),
  "mem:free": defaultMemMeta("可用内存", "mem:total"),
  "mem:available": defaultMemMeta("可用内存", "mem:total"),
};

// 添加 CPU 核心元数据（最多 32 核）
for (let i = 0; i < 32; i++) {
  DEFAULT_PLOT_META[`cpu:${i}`] = defaultCpuMeta(`核心 ${i}`);
}

/**
 * 图表类型到指标的映射函数
 */
export const PLOT_TYPE_METRICS: Record<
  PlotType,
  (data: SysinfoDataPoint) => string[]
> = {
  CPU: () => ["cpu"],
  Mem: () => ["mem:used"],
  "CPU + Mem": () => ["cpu", "mem:used"],
  "All CPU": (data) => {
    if (!data) return ["cpu"];
    return Object.keys(data)
      .filter((key) => key.startsWith("cpu:"))
      .sort((a, b) => {
        const valA = parseInt(a.replace("cpu:", ""));
        const valB = parseInt(b.replace("cpu:", ""));
        return valA - valB;
      });
  },
};

/**
 * 图表颜色列表
 */
export const PLOT_COLORS = [
  "#58C142",
  "#FFC107",
  "#FF5722",
  "#2196F3",
  "#9C27B0",
  "#00BCD4",
  "#FFEB3B",
  "#795548",
];

/**
 * localStorage 键名
 */
export const STORAGE_KEYS = {
  WIDGET_CONFIG: "proxycast-widget-config",
  ACTIVE_WIDGET: "proxycast-active-widget",
};
