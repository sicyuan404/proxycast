/**
 * @file 插件 UI 组件库
 * @description 导出给插件使用的公共组件和工具
 * @module lib/plugin-components
 *
 * 插件可以通过这个模块使用主应用的 UI 组件，
 * 保持一致的视觉风格和交互体验。
 */

// ============================================================================
// 基础 UI 组件
// ============================================================================

// 按钮
export { Button } from "@/components/ui/button";

// 卡片
export {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";

// 标签页
export { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

// 徽章
export { Badge } from "@/components/ui/badge";

// 输入框
export { Input } from "@/components/ui/input";

// 文本域
export { Textarea } from "@/components/ui/textarea";

// 开关
export { Switch } from "@/components/ui/switch";

// 选择器
export {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// 对话框
export {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

// 下拉菜单
export {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

// 工具提示
export {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

// ============================================================================
// 自定义组件
// ============================================================================

// 模态框
export { Modal } from "@/components/Modal";

// ============================================================================
// OAuth 凭证相关组件
// ============================================================================

// Kiro 凭证表单（自包含版本，适合插件使用）
export { KiroFormStandalone } from "@/components/provider-pool/credential-forms/KiroFormStandalone";

// Kiro 凭证表单（需要外部状态管理）
export { KiroForm } from "@/components/provider-pool/credential-forms/KiroForm";

// Antigravity 凭证表单（自包含版本，适合插件使用）
export { AntigravityFormStandalone } from "@/components/provider-pool/credential-forms/AntigravityFormStandalone";

// Gemini 凭证表单（自包含版本，适合插件使用）
export { GeminiFormStandalone } from "@/components/provider-pool/credential-forms/GeminiFormStandalone";

// Claude 凭证表单（自包含版本，适合插件使用）
export { ClaudeFormStandalone } from "@/components/provider-pool/credential-forms/ClaudeFormStandalone";

// 浏览器模式选择器
export {
  BrowserModeSelector,
  type BrowserMode,
} from "@/components/provider-pool/credential-forms/BrowserModeSelector";

// 文件导入表单
export { FileImportForm } from "@/components/provider-pool/credential-forms/FileImportForm";

// Playwright 安装指南
export { PlaywrightInstallGuide } from "@/components/provider-pool/credential-forms/PlaywrightInstallGuide";

// Playwright 错误显示
export { PlaywrightErrorDisplay } from "@/components/provider-pool/credential-forms/PlaywrightErrorDisplay";

// 编辑凭证模态框
export { EditCredentialModal } from "@/components/provider-pool/EditCredentialModal";

// ============================================================================
// 工具函数
// ============================================================================

// 样式工具
export { cn } from "@/lib/utils";

// Toast 通知
export { toast } from "sonner";

// ============================================================================
// 图标（从 lucide-react 重新导出常用图标）
// ============================================================================

export {
  // 操作
  Plus,
  Minus,
  Check,
  X,
  Edit,
  Trash2,
  Copy,
  Download,
  Upload,
  RefreshCw,
  Search,
  Settings,
  Settings2,
  RotateCcw,
  // 状态
  Loader2,
  AlertCircle,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Info,
  Heart,
  HeartOff,
  // 导航
  ChevronDown,
  ChevronUp,
  ChevronLeft,
  ChevronRight,
  ArrowLeft,
  ArrowRight,
  ExternalLink,
  // 凭证相关
  Key,
  KeyRound,
  Lock,
  Unlock,
  Shield,
  ShieldCheck,
  Fingerprint,
  // 文件
  File,
  FileText,
  Folder,
  FolderOpen,
  // 用户
  User,
  Users,
  // 其他
  Star,
  Clock,
  Calendar,
  Activity,
  Zap,
  Power,
  PowerOff,
  Globe,
  LogIn,
  LogOut,
  Timer,
  BarChart3,
  MonitorDown,
  Terminal,
  Building,
  Cloud,
  Server,
  Mail,
  Sparkles,
  Cookie,
  FileJson,
  Code,
  Bot,
} from "lucide-react";

// ============================================================================
// 类型定义
// ============================================================================

export type {
  ProxyCastPluginSDK as PluginSDK,
  CredentialInfo,
} from "@/lib/plugin-sdk/types";

// ============================================================================
// Provider Pool API（用于凭证管理）
// ============================================================================

export { providerPoolApi } from "@/lib/api/providerPool";
export {
  getKiroCredentialFingerprint,
  switchKiroToLocal,
  kiroCredentialApi,
} from "@/lib/api/providerPool";
export type {
  PoolProviderType,
  CredentialDisplay,
  ProviderCredential,
  KiroFingerprintInfo,
  SwitchToLocalResult,
  CredentialSource,
  UpdateCredentialRequest,
} from "@/lib/api/providerPool";

// Usage API
export { usageApi } from "@/lib/api/usage";
export type { UsageInfo } from "@/lib/api/usage";
