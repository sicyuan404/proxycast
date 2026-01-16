/**
 * @file DeveloperSettings.tsx
 * @description 开发者设置页面 - 组件视图调试等开发工具
 */
import { Code2, Eye } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { useComponentDebug } from "@/contexts/ComponentDebugContext";

export function DeveloperSettings() {
  const { enabled, setEnabled } = useComponentDebug();

  return (
    <div className="space-y-6 max-w-2xl">
      {/* 标题 */}
      <div className="flex items-center gap-2 mb-4">
        <Code2 className="w-5 h-5 text-primary" />
        <h3 className="text-lg font-semibold">开发者工具</h3>
      </div>

      {/* 组件视图调试 */}
      <div className="border rounded-lg p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-blue-500/10 rounded-lg">
              <Eye className="w-5 h-5 text-blue-500" />
            </div>
            <div>
              <h4 className="font-medium">组件视图调试</h4>
              <p className="text-sm text-muted-foreground">
                显示组件轮廓，Alt+点击查看组件信息
              </p>
            </div>
          </div>
          <Switch checked={enabled} onCheckedChange={setEnabled} />
        </div>

        {enabled && (
          <div className="mt-4 p-3 bg-muted/50 rounded-md text-sm text-muted-foreground">
            <p className="font-medium text-foreground mb-2">使用说明:</p>
            <ul className="list-disc list-inside space-y-1">
              <li>启用后自动显示所有组件的边框轮廓</li>
              <li>按住 Alt 键 + 鼠标悬浮高亮显示当前组件</li>
              <li>Alt + 点击组件可查看名称和文件路径</li>
              <li>文件路径仅在开发模式 (npm run tauri dev) 下可用</li>
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
