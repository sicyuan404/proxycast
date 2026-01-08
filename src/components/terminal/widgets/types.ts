/**
 * 小部件系统类型定义
 *
 * 定义 WidgetsSidebar 相关的所有类型
 * 包括小部件配置、显示模式、系统信息数据等
 *
 * @module widgets/types
 */

import { LucideIcon } from "lucide-react";

/**
 * 小部件类型枚举
 */
export type WidgetType =
  | "terminal"
  | "files"
  | "web"
  | "sysinfo"
  | "ai"
  | "settings"
  | "tips"
  | "help"
  | "secrets";

/**
 * 小部件显示模式
 * - normal: 显示图标和标签
 * - compact: 仅显示图标
 * - supercompact: 2列网格布局，仅图标
 */
export type WidgetDisplayMode = "normal" | "compact" | "supercompact";

/**
 * 小部件配置
 */
export interface WidgetConfig {
  /** 唯一标识符 */
  id: string;
  /** 显示标签 */
  label: string;
  /** 图标名称（Lucide 图标） */
  icon: string;
  /** 图标颜色 */
  color?: string;
  /** 描述文本（用于 tooltip） */
  description?: string;
  /** 显示顺序 */
  displayOrder: number;
  /** 是否隐藏 */
  hidden?: boolean;
  /** 小部件类型 */
  type: WidgetType;
}

/**
 * 系统信息数据点
 */
export interface SysinfoDataPoint {
  /** 时间戳（毫秒） */
  ts: number;
  /** CPU 使用率（0-100） */
  cpu: number;
  /** 已用内存（GB） */
  "mem:used": number;
  /** 总内存（GB） */
  "mem:total": number;
  /** 各核心 CPU 使用率 */
  [key: `cpu:${number}`]: number;
}

/**
 * 时间序列元数据
 */
export interface TimeSeriesMeta {
  /** 名称 */
  name: string;
  /** Y轴标签 */
  label: string;
  /** Y轴最小值 */
  miny: number;
  /** Y轴最大值（可以是数字或数据字段名） */
  maxy: number | string;
  /** 线条颜色 */
  color: string;
  /** 小数位数 */
  decimalPlaces: number;
}

/**
 * 图表类型
 */
export type PlotType = "CPU" | "Mem" | "CPU + Mem" | "All CPU";

/**
 * 文件条目
 */
export interface FileEntry {
  /** 文件名 */
  name: string;
  /** 完整路径 */
  path: string;
  /** 是否为目录 */
  isDir: boolean;
  /** 文件大小（字节） */
  size: number;
  /** 修改时间（时间戳） */
  modifiedAt: number;
  /** 文件权限（已废弃，使用 modeStr） */
  permissions?: string;
  /** 文件类型/扩展名 */
  fileType?: string;
  /** 是否为隐藏文件 */
  isHidden?: boolean;
  /** 文件权限字符串（如 -rw-r--r--） */
  modeStr?: string;
  /** 文件权限数字（8进制） */
  mode?: number;
  /** MIME 类型 */
  mimeType?: string;
  /** 是否为符号链接 */
  isSymlink?: boolean;
}

/**
 * 设置菜单项
 */
export interface SettingsMenuItem {
  /** 图标 */
  icon: LucideIcon;
  /** 标签 */
  label: string;
  /** 点击处理 */
  onClick: () => void;
}

/**
 * 小部件上下文值
 */
export interface WidgetContextValue {
  /** 小部件配置列表 */
  widgets: WidgetConfig[];
  /** 更新小部件配置 */
  updateWidget: (id: string, config: Partial<WidgetConfig>) => void;
  /** 切换小部件显示/隐藏 */
  toggleWidgetVisibility: (id: string) => void;
  /** 当前激活的小部件 */
  activeWidget: WidgetType | null;
  /** 设置激活的小部件 */
  setActiveWidget: (type: WidgetType | null) => void;
}
