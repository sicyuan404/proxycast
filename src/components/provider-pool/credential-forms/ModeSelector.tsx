/**
 * 登录/文件导入模式选择器
 */

import { LogIn, FolderOpen } from "lucide-react";

interface ModeSelectorProps {
  mode: "login" | "file";
  setMode: (mode: "login" | "file") => void;
  loginLabel?: string;
  fileLabel?: string;
}

export function ModeSelector({
  mode,
  setMode,
  loginLabel = "登录",
  fileLabel = "导入文件",
}: ModeSelectorProps) {
  return (
    <div className="flex gap-2 mb-4">
      <button
        type="button"
        onClick={() => setMode("login")}
        className={`flex-1 rounded-lg border px-3 py-2 text-sm ${
          mode === "login"
            ? "border-primary bg-primary/10 text-primary"
            : "hover:bg-muted"
        }`}
      >
        <LogIn className="inline h-4 w-4 mr-1" />
        {loginLabel}
      </button>
      <button
        type="button"
        onClick={() => setMode("file")}
        className={`flex-1 rounded-lg border px-3 py-2 text-sm ${
          mode === "file"
            ? "border-primary bg-primary/10 text-primary"
            : "hover:bg-muted"
        }`}
      >
        <FolderOpen className="inline h-4 w-4 mr-1" />
        {fileLabel}
      </button>
    </div>
  );
}
