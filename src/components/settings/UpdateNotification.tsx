/**
 * @file UpdateNotification.tsx
 * @description 更新检查设置组件
 *
 * 用于在设置页面中配置自动更新检查行为。
 * 更新提醒弹窗已移至独立窗口 (src/pages/update-notification.tsx)。
 *
 * input: 用户配置操作
 * output: 更新检查设置 UI
 * pos: components/settings 层
 */

import { useState, useEffect } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import { Bug } from "lucide-react";

interface UpdateCheckConfig {
  enabled: boolean;
  check_interval_hours: number;
  show_notification: boolean;
  last_check_timestamp: number;
  skipped_version: string | null;
}

/**
 * 更新检查设置组件
 * 用于在设置页面中配置自动更新检查行为
 */
export function UpdateCheckSettings() {
  const [settings, setSettings] = useState<UpdateCheckConfig>({
    enabled: true,
    check_interval_hours: 24,
    show_notification: true,
    last_check_timestamp: 0,
    skipped_version: null,
  });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const result = await safeInvoke<UpdateCheckConfig>(
        "get_update_check_settings",
      );
      setSettings(result);
    } catch (error) {
      console.error("加载更新检查设置失败:", error);
    } finally {
      setLoading(false);
    }
  };

  const saveSettings = async (newSettings: UpdateCheckConfig) => {
    try {
      await safeInvoke("set_update_check_settings", { settings: newSettings });
      setSettings(newSettings);
    } catch (error) {
      console.error("保存更新检查设置失败:", error);
    }
  };

  const handleToggleEnabled = () => {
    saveSettings({ ...settings, enabled: !settings.enabled });
  };

  const handleToggleNotification = () => {
    saveSettings({
      ...settings,
      show_notification: !settings.show_notification,
    });
  };

  const handleIntervalChange = (hours: number) => {
    saveSettings({ ...settings, check_interval_hours: hours });
  };

  const handleClearSkipped = () => {
    saveSettings({ ...settings, skipped_version: null });
  };

  if (loading) {
    return (
      <div className="p-4 rounded-lg border animate-pulse">
        <div className="h-4 bg-muted rounded w-1/3 mb-2"></div>
        <div className="h-3 bg-muted rounded w-2/3"></div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium">自动更新检查</h3>

      <div className="space-y-3">
        {/* 启用自动检查 */}
        <div className="flex items-center justify-between p-3 rounded-lg border">
          <div>
            <div className="text-sm font-medium">自动检查更新</div>
            <div className="text-xs text-muted-foreground">
              定期检查是否有新版本可用
            </div>
          </div>
          <button
            onClick={handleToggleEnabled}
            className={`relative w-11 h-6 rounded-full transition-colors ${
              settings.enabled ? "bg-blue-600" : "bg-muted"
            }`}
          >
            <span
              className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform ${
                settings.enabled ? "translate-x-5" : ""
              }`}
            />
          </button>
        </div>

        {/* 显示通知 */}
        <div className="flex items-center justify-between p-3 rounded-lg border">
          <div>
            <div className="text-sm font-medium">显示更新通知</div>
            <div className="text-xs text-muted-foreground">
              发现新版本时显示弹窗提醒
            </div>
          </div>
          <button
            onClick={handleToggleNotification}
            disabled={!settings.enabled}
            className={`relative w-11 h-6 rounded-full transition-colors disabled:opacity-50 ${
              settings.show_notification ? "bg-blue-600" : "bg-muted"
            }`}
          >
            <span
              className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform ${
                settings.show_notification ? "translate-x-5" : ""
              }`}
            />
          </button>
        </div>

        {/* 检查间隔 */}
        <div className="p-3 rounded-lg border">
          <div className="text-sm font-medium mb-2">检查间隔</div>
          <div className="flex gap-2">
            {[12, 24, 48, 168].map((hours) => (
              <button
                key={hours}
                onClick={() => handleIntervalChange(hours)}
                disabled={!settings.enabled}
                className={`px-3 py-1.5 rounded-md text-xs transition-colors disabled:opacity-50 ${
                  settings.check_interval_hours === hours
                    ? "bg-blue-600 text-white"
                    : "bg-muted hover:bg-muted/80"
                }`}
              >
                {hours === 168 ? "每周" : `${hours}小时`}
              </button>
            ))}
          </div>
        </div>

        {/* 已跳过的版本 */}
        {settings.skipped_version && (
          <div className="flex items-center justify-between p-3 rounded-lg border bg-muted/30">
            <div>
              <div className="text-sm">已跳过版本</div>
              <div className="text-xs text-muted-foreground font-mono">
                {settings.skipped_version}
              </div>
            </div>
            <button
              onClick={handleClearSkipped}
              className="px-3 py-1 rounded-md text-xs bg-muted hover:bg-muted/80 transition-colors"
            >
              清除
            </button>
          </div>
        )}

        {/* 上次检查时间 */}
        {settings.last_check_timestamp > 0 && (
          <div className="text-xs text-muted-foreground">
            上次检查:{" "}
            {new Date(settings.last_check_timestamp * 1000).toLocaleString()}
          </div>
        )}

        {/* 测试按钮（仅开发环境） */}
        {import.meta.env.DEV && (
          <button
            onClick={async () => {
              try {
                await safeInvoke("test_update_window");
              } catch (error) {
                console.error("测试更新弹窗失败:", error);
              }
            }}
            className="flex items-center justify-center gap-1.5 w-full px-3 py-1.5 rounded-lg border border-dashed border-orange-400 text-orange-600 text-xs hover:bg-orange-50 dark:hover:bg-orange-900/20 transition-colors"
          >
            <Bug className="h-3.5 w-3.5" />
            测试更新弹窗
          </button>
        )}
      </div>
    </div>
  );
}

// 保持向后兼容的导出（已废弃，返回 null）
export function UpdateNotification() {
  return null;
}
