/**
 * 文件导入表单组件
 */

import { FolderOpen } from "lucide-react";

interface FileImportFormProps {
  credsFilePath: string;
  setCredsFilePath: (path: string) => void;
  onSelectFile: () => void;
  placeholder?: string;
  hint?: string;
  projectId?: string;
  setProjectId?: (id: string) => void;
  showProjectId?: boolean;
}

export function FileImportForm({
  credsFilePath,
  setCredsFilePath,
  onSelectFile,
  placeholder = "选择凭证文件...",
  hint,
  projectId,
  setProjectId,
  showProjectId = false,
}: FileImportFormProps) {
  return (
    <>
      <div>
        <label className="mb-1 block text-sm font-medium">
          凭证文件路径 <span className="text-red-500">*</span>
        </label>
        <div className="flex gap-2">
          <input
            type="text"
            value={credsFilePath}
            onChange={(e) => setCredsFilePath(e.target.value)}
            placeholder={placeholder}
            className="flex-1 rounded-lg border bg-background px-3 py-2 text-sm"
          />
          <button
            type="button"
            onClick={onSelectFile}
            className="flex items-center gap-1 rounded-lg border px-3 py-2 text-sm hover:bg-muted"
          >
            <FolderOpen className="h-4 w-4" />
            浏览
          </button>
        </div>
        {hint && <p className="mt-1 text-xs text-muted-foreground">{hint}</p>}
      </div>

      {showProjectId && setProjectId && (
        <div>
          <label className="mb-1 block text-sm font-medium">
            Project ID (可选)
          </label>
          <input
            type="text"
            value={projectId || ""}
            onChange={(e) => setProjectId(e.target.value)}
            placeholder="Google Cloud Project ID..."
            className="w-full rounded-lg border bg-background px-3 py-2 text-sm"
          />
        </div>
      )}
    </>
  );
}
