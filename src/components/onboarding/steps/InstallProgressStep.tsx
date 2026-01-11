/**
 * 初次安装引导 - 安装进度
 */

import { useState, useEffect, useCallback, useRef } from "react";
import styled from "styled-components";
import { safeInvoke } from "@/lib/dev-bridge";
import { safeListen } from "@/lib/dev-bridge";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { Check, X, Loader2 } from "lucide-react";
import { Progress } from "@/components/ui/progress";
import { onboardingPlugins } from "../constants";

const Container = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px 24px;
`;

const Title = styled.h2`
  font-size: 24px;
  font-weight: 600;
  color: hsl(var(--foreground));
  margin-bottom: 8px;
  text-align: center;
`;

const Subtitle = styled.p`
  font-size: 14px;
  color: hsl(var(--muted-foreground));
  margin-bottom: 32px;
  text-align: center;
`;

const PluginList = styled.div`
  display: flex;
  flex-direction: column;
  gap: 16px;
  width: 100%;
  max-width: 500px;
`;

const PluginRow = styled.div`
  display: flex;
  align-items: center;
  gap: 16px;
`;

const IconWrapper = styled.div<{ $status: string }>`
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: ${({ $status }) => {
    switch ($status) {
      case "complete":
        return "hsl(142.1 76.2% 36.3%)";
      case "failed":
        return "hsl(0 84.2% 60.2%)";
      default:
        return "hsl(var(--muted))";
    }
  }};
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all 0.3s;

  svg {
    width: 20px;
    height: 20px;
    color: ${({ $status }) =>
      $status === "complete" || $status === "failed"
        ? "white"
        : "hsl(var(--foreground))"};
  }
`;

const PluginInfo = styled.div`
  flex: 1;
  min-width: 0;
`;

const PluginName = styled.div`
  font-size: 14px;
  font-weight: 600;
  color: hsl(var(--foreground));
  margin-bottom: 4px;
`;

const PluginStatus = styled.div<{ $status: string }>`
  font-size: 12px;
  color: ${({ $status }) => {
    switch ($status) {
      case "complete":
        return "hsl(142.1 76.2% 36.3%)";
      case "failed":
        return "hsl(0 84.2% 60.2%)";
      default:
        return "hsl(var(--muted-foreground))";
    }
  }};
`;

const ProgressWrapper = styled.div`
  width: 100px;
  flex-shrink: 0;
`;

const OverallProgress = styled.div`
  width: 100%;
  max-width: 500px;
  margin-top: 32px;
  padding-top: 24px;
  border-top: 1px solid hsl(var(--border));
`;

const OverallLabel = styled.div`
  display: flex;
  justify-content: space-between;
  margin-bottom: 8px;
  font-size: 12px;
  color: hsl(var(--muted-foreground));
`;

/**
 * 插件安装状态
 */
export interface PluginInstallState {
  pluginId: string;
  status: "pending" | "downloading" | "installing" | "complete" | "failed";
  progress: number;
  message: string;
  error?: string;
}

/**
 * 安装进度事件
 */
interface InstallProgress {
  stage: string;
  percent: number;
  message: string;
}

/**
 * 安装结果
 */
interface InstallResult {
  success: boolean;
  plugin?: {
    id: string;
    name: string;
  };
  error?: string;
}

interface InstallProgressStepProps {
  selectedPlugins: string[];
  onComplete: (results: PluginInstallState[]) => void;
}

export function InstallProgressStep({
  selectedPlugins,
  onComplete,
}: InstallProgressStepProps) {
  const [installStates, setInstallStates] = useState<PluginInstallState[]>([]);
  const [isInstalling, setIsInstalling] = useState(false);
  const hasStarted = useRef(false);

  // 初始化安装状态
  useEffect(() => {
    if (selectedPlugins.length === 0) {
      onComplete([]);
      return;
    }

    setInstallStates(
      selectedPlugins.map((id) => ({
        pluginId: id,
        status: "pending",
        progress: 0,
        message: "等待安装...",
      })),
    );
  }, [selectedPlugins, onComplete]);

  // 顺序安装插件
  const installPlugins = useCallback(async () => {
    if (isInstalling || selectedPlugins.length === 0) return;
    setIsInstalling(true);

    let unlisten: UnlistenFn | null = null;

    for (let i = 0; i < selectedPlugins.length; i++) {
      const pluginId = selectedPlugins[i];
      const plugin = onboardingPlugins.find((p) => p.id === pluginId);

      if (!plugin) continue;

      try {
        // 监听当前插件的进度
        unlisten = await safeListen<InstallProgress>(
          "plugin-install-progress",
          (event) => {
            setInstallStates((prev) =>
              prev.map((state) =>
                state.pluginId === pluginId
                  ? {
                      ...state,
                      status:
                        event.payload.stage === "complete"
                          ? "complete"
                          : event.payload.stage === "failed"
                            ? "failed"
                            : "downloading",
                      progress: event.payload.percent,
                      message: event.payload.message,
                    }
                  : state,
              ),
            );
          },
        );

        // 更新状态为下载中
        setInstallStates((prev) =>
          prev.map((state) =>
            state.pluginId === pluginId
              ? { ...state, status: "downloading", message: "准备下载..." }
              : state,
          ),
        );

        // 调用安装 API
        const result = await safeInvoke<InstallResult>(
          "install_plugin_from_url",
          {
            url: plugin.downloadUrl,
          },
        );

        // 取消监听
        if (unlisten) {
          unlisten();
          unlisten = null;
        }

        // 更新结果状态
        setInstallStates((prev) =>
          prev.map((state) =>
            state.pluginId === pluginId
              ? {
                  ...state,
                  status: result.success ? "complete" : "failed",
                  progress: 100,
                  message: result.success
                    ? "安装成功"
                    : result.error || "安装失败",
                  error: result.error,
                }
              : state,
          ),
        );
      } catch (e) {
        // 取消监听
        if (unlisten) {
          unlisten();
          unlisten = null;
        }

        setInstallStates((prev) =>
          prev.map((state) =>
            state.pluginId === pluginId
              ? {
                  ...state,
                  status: "failed",
                  progress: 100,
                  message: "安装出错",
                  error: e instanceof Error ? e.message : String(e),
                }
              : state,
          ),
        );
      }
    }

    setIsInstalling(false);
  }, [selectedPlugins, isInstalling]);

  // 开始安装
  useEffect(() => {
    if (
      installStates.length > 0 &&
      !isInstalling &&
      !hasStarted.current &&
      installStates.every((s) => s.status === "pending")
    ) {
      hasStarted.current = true;
      installPlugins();
    }
  }, [installStates, isInstalling, installPlugins]);

  // 检查是否全部完成
  useEffect(() => {
    if (
      installStates.length > 0 &&
      installStates.every(
        (s) => s.status === "complete" || s.status === "failed",
      )
    ) {
      // 延迟一点调用 onComplete，让用户看到最终状态
      const timer = setTimeout(() => {
        onComplete(installStates);
      }, 1000);
      return () => clearTimeout(timer);
    }
  }, [installStates, onComplete]);

  // 计算总体进度
  const completedCount = installStates.filter(
    (s) => s.status === "complete" || s.status === "failed",
  ).length;
  const overallProgress =
    selectedPlugins.length > 0
      ? Math.round((completedCount / selectedPlugins.length) * 100)
      : 0;

  const getStatusIcon = (state: PluginInstallState) => {
    const plugin = onboardingPlugins.find((p) => p.id === state.pluginId);
    const Icon = plugin?.icon;

    switch (state.status) {
      case "complete":
        return <Check />;
      case "failed":
        return <X />;
      case "downloading":
      case "installing":
        return <Loader2 className="animate-spin" />;
      default:
        return Icon ? <Icon /> : null;
    }
  };

  return (
    <Container>
      <Title>正在安装插件</Title>
      <Subtitle>请稍候，正在为您安装选中的插件...</Subtitle>

      <PluginList>
        {installStates.map((state) => {
          const plugin = onboardingPlugins.find((p) => p.id === state.pluginId);

          return (
            <PluginRow key={state.pluginId}>
              <IconWrapper $status={state.status}>
                {getStatusIcon(state)}
              </IconWrapper>
              <PluginInfo>
                <PluginName>{plugin?.name || state.pluginId}</PluginName>
                <PluginStatus $status={state.status}>
                  {state.message}
                </PluginStatus>
              </PluginInfo>
              <ProgressWrapper>
                <Progress value={state.progress} />
              </ProgressWrapper>
            </PluginRow>
          );
        })}
      </PluginList>

      <OverallProgress>
        <OverallLabel>
          <span>总体进度</span>
          <span>
            {completedCount} / {selectedPlugins.length}
          </span>
        </OverallLabel>
        <Progress value={overallProgress} />
      </OverallProgress>
    </Container>
  );
}
