import { safeInvoke } from "@/lib/dev-bridge";

interface AutoFixResult {
  issues_found: string[];
  fixes_applied: string[];
  warnings: string[];
}

export const useAutoFix = () => {
  const runAutoFix = async (): Promise<AutoFixResult> => {
    return await safeInvoke("auto_fix_configuration");
  };

  return {
    runAutoFix,
  };
};
