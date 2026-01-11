/**
 * @file Tauri 音乐命令 API
 * @description 封装 Tauri 后端音乐相关命令
 * @module components/content-creator/canvas/music/api/tauri
 */

import { safeInvoke } from "@/lib/dev-bridge";

/**
 * MIDI 分析结果
 */
export interface MidiAnalysisResult {
  /** 调式信息 */
  mode: string;
  /** BPM (每分钟节拍数) */
  bpm: number;
  /** 拍号 */
  time_signature: string;
  /** 音轨信息 */
  tracks: TrackInfo[];
  /** 旋律特征 */
  melody_features: MelodyFeatures;
}

/**
 * 音轨信息
 */
export interface TrackInfo {
  /** 音轨索引 */
  index: number;
  /** 音轨名称 */
  name: string;
  /** 乐器名称 */
  instrument: string;
  /** 音符数量 */
  note_count: number;
  /** 是否为人声音轨 */
  is_vocal: boolean;
}

/**
 * 旋律特征
 */
export interface MelodyFeatures {
  /** 音域范围 (半音数) */
  range: number;
  /** 平均音高 */
  avg_pitch: number;
  /** 音程跳跃频率 */
  interval_jumps: number;
  /** 节奏复杂度 */
  rhythm_complexity: number;
}

/**
 * Python 环境信息
 */
export interface PythonEnvInfo {
  /** 是否已安装 Python */
  python_installed: boolean;
  /** Python 版本 */
  python_version: string | null;
  /** 缺失的依赖包 */
  missing_packages: string[];
}

/**
 * 检查 Python 环境
 */
export async function checkPythonEnv(): Promise<PythonEnvInfo> {
  return safeInvoke<PythonEnvInfo>("check_python_env");
}

/**
 * 分析 MIDI 文件
 * @param midiPath MIDI 文件路径
 */
export async function analyzeMidi(
  midiPath: string,
): Promise<MidiAnalysisResult> {
  return safeInvoke<MidiAnalysisResult>("analyze_midi", { midiPath });
}

/**
 * 将 MP3 转换为 MIDI
 * @param mp3Path MP3 文件路径
 * @param outputPath 输出 MIDI 文件路径
 */
export async function convertMp3ToMidi(
  mp3Path: string,
  outputPath: string,
): Promise<string> {
  return safeInvoke<string>("convert_mp3_to_midi", { mp3Path, outputPath });
}

/**
 * 加载音乐资源文件
 * @param resourceName 资源文件名 (相对于 resources/music/ 目录)
 */
export async function loadMusicResource(resourceName: string): Promise<string> {
  return safeInvoke<string>("load_music_resource", { resourceName });
}

/**
 * 安装 Python 依赖
 */
export async function installPythonDependencies(): Promise<string> {
  return safeInvoke<string>("install_python_dependencies");
}

/**
 * 加载押韵数据库
 */
export async function loadRhymePatterns(): Promise<any> {
  const content = await loadMusicResource("rhyme-patterns.json");
  return JSON.parse(content);
}

/**
 * 加载和弦进行库
 */
export async function loadChordProgressions(): Promise<any> {
  const content = await loadMusicResource("chord-progressions.json");
  return JSON.parse(content);
}

/**
 * 加载五声音阶规则
 */
export async function loadPentatonicRules(): Promise<any> {
  const content = await loadMusicResource("pentatonic-rules.json");
  return JSON.parse(content);
}

/**
 * 加载国风旋律模式
 */
export async function loadGuofengPatterns(): Promise<any> {
  const content = await loadMusicResource("guofeng-patterns.json");
  return JSON.parse(content);
}

/**
 * 加载 MIDI 解析规则
 */
export async function loadMidiParserRules(): Promise<any> {
  const content = await loadMusicResource("midi-parser-rules.json");
  return JSON.parse(content);
}
