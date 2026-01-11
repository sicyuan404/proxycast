import { safeInvoke } from "@/lib/dev-bridge";

export interface McpServer {
  id: string;
  name: string;
  server_config: {
    command: string;
    args?: string[];
    env?: Record<string, string>;
  };
  description?: string;
  enabled_proxycast: boolean;
  enabled_claude: boolean;
  enabled_codex: boolean;
  enabled_gemini: boolean;
  created_at?: number;
}

export const mcpApi = {
  getServers: (): Promise<McpServer[]> => safeInvoke("get_mcp_servers"),

  addServer: (server: McpServer): Promise<void> =>
    safeInvoke("add_mcp_server", { server }),

  updateServer: (server: McpServer): Promise<void> =>
    safeInvoke("update_mcp_server", { server }),

  deleteServer: (id: string): Promise<void> =>
    safeInvoke("delete_mcp_server", { id }),

  toggleServer: (
    id: string,
    appType: string,
    enabled: boolean,
  ): Promise<void> => safeInvoke("toggle_mcp_server", { id, appType, enabled }),

  /** 从外部应用导入 MCP 配置 */
  importFromApp: (appType: string): Promise<number> =>
    safeInvoke("import_mcp_from_app", { appType }),

  /** 同步所有 MCP 配置到实际配置文件 */
  syncAllToLive: (): Promise<void> => safeInvoke("sync_all_mcp_to_live"),
};
