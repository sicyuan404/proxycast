/**
 * @file TerminalWorkspace.tsx
 * @description 终端工作区组件 - 支持分块布局
 * @module components/terminal/TerminalWorkspace
 *
 * 管理终端页面的分块布局，支持在主终端旁边添加附加面板。
 * 对齐 Waveterm 的 TileLayout 风格，所有面板水平排列。
 * 包含右侧小部件栏（WidgetsSidebar）。
 *
 * ## 功能
 * - 主终端区域（左侧）
 * - 附加面板区域（右侧，水平排列）
 * - 支持 Terminal/Files/Web/Sysinfo 面板类型
 * - Terminal 类型支持多实例
 * - 右侧小部件栏
 */

import { useState, useCallback, useRef } from "react";
import styled from "styled-components";
import { TerminalPanel } from "./TerminalPanel";
import {
  SysinfoView,
  FileBrowserView,
  WebView,
  WidgetsSidebar,
  WidgetProvider,
  WidgetType,
} from "./widgets";
import { TerminalAIPanel } from "./ai";
import {
  ConnectionSelector,
  type ConnectionListEntry,
} from "./ConnectionSelector";
import { ConnectionsEditorModal } from "./ConnectionsEditorModal";
import { Page } from "@/types/page";

// ============================================================================
// 类型定义
// ============================================================================

/** 附加面板类型 */
export type SidePanelType = "terminal" | "files" | "web" | "sysinfo" | "ai";

/** 附加面板配置 */
export interface SidePanel {
  id: string;
  type: SidePanelType;
  title: string;
  /** 终端工作目录（仅 terminal 类型使用） */
  cwd?: string;
  /** 连接配置（仅 terminal 类型使用） */
  connection?: ConnectionListEntry;
}

// ============================================================================
// 样式组件
// ============================================================================

/** 终端工作区外层容器 - 包含内容区和右侧小部件栏 */
const WorkspaceOuterContainer = styled.div`
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: row;
`;

/**
 * 终端工作区内容容器 - 对齐 Waveterm TileLayout
 * 所有面板平等分布，使用 flex: 1 实现均分
 */
const WorkspaceContainer = styled.div`
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: row;
  gap: 3px;
  padding: 3px;
  background: #0a0a0a;
`;

/**
 * 单个面板块 - Block 样式（对齐 Waveterm）
 * 所有面板使用相同的 flex: 1，实现平等分布
 */
const PanelBlock = styled.div<{ $focused?: boolean }>`
  flex: 1 1 0;
  min-width: 150px;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: #1a1a1a;
  border-radius: 6px;
  border: 2px solid ${({ $focused }) => ($focused ? "#58a6ff" : "#2a2a2a")};
  overflow: hidden;
`;

/** 面板头部 - 对齐 Waveterm */
const PanelHeader = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 10px;
  background: #1a1a1a;
  border-bottom: 1px solid #2a2a2a;
  min-height: 28px;
  max-height: 28px;
`;

const PanelTitle = styled.span`
  font-size: 12px;
  font-weight: 500;
  color: #e0e0e0;
  display: flex;
  align-items: center;
  gap: 6px;

  svg {
    width: 14px;
    height: 14px;
    opacity: 0.6;
  }
`;

const PanelCloseButton = styled.button`
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: #808080;
  cursor: pointer;
  transition: all 0.15s ease;

  &:hover {
    background: #333;
    color: #e0e0e0;
  }

  svg {
    width: 12px;
    height: 12px;
  }
`;

const PanelContent = styled.div`
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: #1a1a1a;
`;

// ============================================================================
// 图标组件
// ============================================================================

const TerminalIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="4 17 10 11 4 5" />
    <line x1="12" y1="19" x2="20" y2="19" />
  </svg>
);

const FilesIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
  </svg>
);

const WebIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <line x1="2" y1="12" x2="22" y2="12" />
    <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
  </svg>
);

const SysinfoIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
  </svg>
);

const AIIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 2L2 7l10 5 10-5-10-5z" />
    <path d="M2 17l10 5 10-5" />
    <path d="M2 12l10 5 10-5" />
  </svg>
);

const CloseIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="18" y1="6" x2="6" y2="18" />
    <line x1="6" y1="6" x2="18" y2="18" />
  </svg>
);

// ============================================================================
// 主组件
// ============================================================================

interface TerminalWorkspaceProps {
  /** 页面导航回调 */
  onNavigate: (page: Page) => void;
}

/**
 * 终端工作区组件
 */
export function TerminalWorkspace({ onNavigate }: TerminalWorkspaceProps) {
  // 面板状态管理 - 初始包含主终端
  const [panels, setPanels] = useState<SidePanel[]>([
    { id: "main-terminal", type: "terminal", title: "Terminal" },
  ]);

  // AI 面板状态
  const [showAIPanel, setShowAIPanel] = useState(false);

  // 终端输出引用（用于 AI 上下文）
  const terminalOutputRef = useRef<string | null>(null);

  // 连接编辑器模态窗口状态
  const [isConnectionsEditorOpen, setIsConnectionsEditorOpen] = useState(false);

  // 添加面板 - 所有类型都允许多开
  const addPanel = useCallback(
    (
      type: SidePanelType,
      options?: { cwd?: string; connection?: ConnectionListEntry },
    ) => {
      setPanels((prev) => {
        const titles: Record<SidePanelType, string> = {
          terminal: "Terminal",
          files: "Files",
          web: "Web",
          sysinfo: "Sysinfo",
          ai: "AI",
        };

        // 如果有连接配置，使用连接标签作为标题
        const title = options?.connection
          ? options.connection.label
          : titles[type];

        return [
          ...prev,
          {
            id: `panel-${Date.now()}`,
            type,
            title,
            cwd: options?.cwd,
            connection: options?.connection,
          },
        ];
      });
    },
    [],
  );

  // 移除面板
  const removePanel = useCallback((id: string) => {
    setPanels((prev) => prev.filter((p) => p.id !== id));
  }, []);

  // 更新面板连接
  const updatePanelConnection = useCallback(
    (id: string, connection: ConnectionListEntry) => {
      setPanels((prev) =>
        prev.map((p) => {
          if (p.id === id && p.type === "terminal") {
            return {
              ...p,
              title: connection.label,
              connection,
            };
          }
          return p;
        }),
      );
    },
    [],
  );

  // 渲染面板图标
  const renderPanelIcon = (type: SidePanelType) => {
    switch (type) {
      case "terminal":
        return <TerminalIcon />;
      case "files":
        return <FilesIcon />;
      case "web":
        return <WebIcon />;
      case "sysinfo":
        return <SysinfoIcon />;
      case "ai":
        return <AIIcon />;
      default:
        return null;
    }
  };

  // 在文件浏览器中打开终端的回调
  const handleOpenTerminalFromFiles = useCallback(
    (path: string) => {
      addPanel("terminal", { cwd: path });
    },
    [addPanel],
  );

  // 获取终端输出（用于 AI 上下文）
  const getTerminalOutput = useCallback(() => {
    return terminalOutputRef.current;
  }, []);

  // 渲染面板内容
  const renderPanelContent = (panel: SidePanel) => {
    switch (panel.type) {
      case "terminal":
        return <TerminalPanel panelId={panel.id} cwd={panel.cwd} />;
      case "files":
        return <FileBrowserView onOpenTerminal={handleOpenTerminalFromFiles} />;
      case "web":
        return <WebView />;
      case "sysinfo":
        return <SysinfoView />;
      case "ai":
        return <TerminalAIPanel getTerminalOutput={getTerminalOutput} />;
      default:
        return null;
    }
  };

  /**
   * 处理右侧小部件点击
   * 添加面板或导航到其他页面
   */
  const handleWidgetClick = useCallback(
    (type: WidgetType) => {
      switch (type) {
        case "terminal":
          addPanel("terminal");
          break;
        case "files":
          addPanel("files");
          break;
        case "web":
          addPanel("web");
          break;
        case "sysinfo":
          addPanel("sysinfo");
          break;
        case "ai":
          // 切换 AI 面板显示
          setShowAIPanel((prev) => !prev);
          break;
        case "settings":
          onNavigate("settings");
          break;
        case "tips":
          console.log("提示功能待实现");
          break;
        case "secrets":
          onNavigate("settings");
          break;
        case "help":
          console.log("帮助功能待实现");
          break;
      }
    },
    [addPanel, onNavigate],
  );

  return (
    <WidgetProvider>
      <WorkspaceOuterContainer>
        {/* AI 面板（左侧，参考 Waveterm） */}
        {showAIPanel && (
          <div
            style={{ width: 320, minWidth: 280, maxWidth: 400, flexShrink: 0 }}
          >
            <TerminalAIPanel getTerminalOutput={getTerminalOutput} />
          </div>
        )}

        <WorkspaceContainer>
          {/* 所有面板统一渲染，都可以关闭和多开 */}
          {panels.map((panel) => (
            <PanelBlock key={panel.id}>
              <PanelHeader>
                {panel.type === "terminal" ? (
                  <ConnectionSelector
                    currentConnection={panel.connection?.name}
                    onSelect={(conn) => updatePanelConnection(panel.id, conn)}
                    onEditConnections={() => setIsConnectionsEditorOpen(true)}
                  />
                ) : (
                  <PanelTitle>
                    {renderPanelIcon(panel.type)}
                    {panel.title}
                  </PanelTitle>
                )}
                <PanelCloseButton onClick={() => removePanel(panel.id)}>
                  <CloseIcon />
                </PanelCloseButton>
              </PanelHeader>
              <PanelContent>{renderPanelContent(panel)}</PanelContent>
            </PanelBlock>
          ))}
          {/* 没有面板时显示提示 */}
          {panels.length === 0 && (
            <PanelBlock>
              <PanelContent
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "#666",
                }}
              >
                点击右侧图标添加面板
              </PanelContent>
            </PanelBlock>
          )}
        </WorkspaceContainer>
        <WidgetsSidebar onWidgetClick={handleWidgetClick} />
      </WorkspaceOuterContainer>

      {/* 连接配置编辑器模态窗口 */}
      <ConnectionsEditorModal
        isOpen={isConnectionsEditorOpen}
        onClose={() => setIsConnectionsEditorOpen(false)}
      />
    </WidgetProvider>
  );
}

export default TerminalWorkspace;
