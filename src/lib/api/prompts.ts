import { safeInvoke } from "@/lib/dev-bridge";

export interface Prompt {
  id: string;
  app_type: string;
  name: string;
  content: string;
  description?: string;
  enabled: boolean;
  createdAt?: number;
  updatedAt?: number;
}

export type AppType = "claude" | "codex" | "gemini";

export const promptsApi = {
  /** Get all prompts as a map (id -> Prompt) */
  getPrompts: (app: AppType): Promise<Record<string, Prompt>> =>
    safeInvoke("get_prompts", { app }),

  /** Upsert a prompt (insert or update) */
  upsertPrompt: (app: AppType, id: string, prompt: Prompt): Promise<void> =>
    safeInvoke("upsert_prompt", { app, id, prompt }),

  /** Add a new prompt */
  addPrompt: (prompt: Prompt): Promise<void> =>
    safeInvoke("add_prompt", { prompt }),

  /** Update an existing prompt */
  updatePrompt: (prompt: Prompt): Promise<void> =>
    safeInvoke("update_prompt", { prompt }),

  /** Delete a prompt */
  deletePrompt: (app: AppType, id: string): Promise<void> =>
    safeInvoke("delete_prompt", { app, id }),

  /** Enable a prompt and sync to live file */
  enablePrompt: (app: AppType, id: string): Promise<void> =>
    safeInvoke("enable_prompt", { app, id }),

  /** Import prompt from live file */
  importFromFile: (app: AppType): Promise<string> =>
    safeInvoke("import_prompt_from_file", { app }),

  /** Get current live prompt file content */
  getCurrentFileContent: (app: AppType): Promise<string | null> =>
    safeInvoke("get_current_prompt_file_content", { app }),

  /** Auto-import from live file if no prompts exist */
  autoImport: (app: AppType): Promise<number> =>
    safeInvoke("auto_import_prompt", { app }),

  // Legacy API for compatibility
  switchPrompt: (appType: AppType, id: string): Promise<void> =>
    safeInvoke("switch_prompt", { appType, id }),
};
