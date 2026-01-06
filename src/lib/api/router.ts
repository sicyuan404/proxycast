import { invoke } from "@tauri-apps/api/core";

// Provider types
export type ProviderType =
  | "kiro"
  | "gemini"
  | "qwen"
  | "antigravity"
  | "openai"
  | "claude";

// Routing rule
export interface RoutingRule {
  pattern: string;
  target_provider: ProviderType;
  priority: number;
  enabled: boolean;
}

// Exclusion pattern
export interface ExclusionPattern {
  provider: ProviderType;
  pattern: string;
}

// Router configuration
export interface RouterConfig {
  default_provider: ProviderType;
  rules: RoutingRule[];
  exclusions: Record<ProviderType, string[]>;
}

export const routerApi = {
  // Get router configuration
  async getRouterConfig(): Promise<RouterConfig> {
    return invoke("get_router_config");
  },

  // Routing rules
  async addRoutingRule(rule: RoutingRule): Promise<void> {
    return invoke("add_routing_rule", { rule });
  },

  async removeRoutingRule(pattern: string): Promise<void> {
    return invoke("remove_routing_rule", { pattern });
  },

  async updateRoutingRule(pattern: string, rule: RoutingRule): Promise<void> {
    return invoke("update_routing_rule", { pattern, rule });
  },

  async getRoutingRules(): Promise<RoutingRule[]> {
    return invoke("get_routing_rules");
  },

  // Exclusions
  async addExclusion(provider: ProviderType, pattern: string): Promise<void> {
    return invoke("add_exclusion", { provider, pattern });
  },

  async removeExclusion(
    provider: ProviderType,
    pattern: string,
  ): Promise<void> {
    return invoke("remove_exclusion", { provider, pattern });
  },

  async getExclusions(): Promise<Record<ProviderType, string[]>> {
    return invoke("get_exclusions");
  },

  // Default provider
  async setDefaultProvider(provider: ProviderType): Promise<void> {
    return invoke("set_router_default_provider", { provider });
  },

  async clearAllRoutingConfig(): Promise<void> {
    return invoke("clear_all_routing_config");
  },
};
