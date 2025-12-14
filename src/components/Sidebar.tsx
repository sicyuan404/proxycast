import {
  LayoutDashboard,
  Settings,
  Plug,
  MessageSquare,
  Key,
  Monitor,
  Globe,
  Boxes,
} from "lucide-react";
import { cn } from "@/lib/utils";

type Page =
  | "dashboard"
  | "credentials"
  | "clients"
  | "api-server"
  | "providers"
  | "settings"
  | "switch"
  | "mcp"
  | "prompts"
  | "skills";

interface SidebarProps {
  currentPage: Page;
  onNavigate: (page: Page) => void;
}

const navItems = [
  { id: "dashboard" as Page, label: "仪表盘", icon: LayoutDashboard },
  { id: "credentials" as Page, label: "凭证管理", icon: Key },
  { id: "clients" as Page, label: "AI Clients", icon: Monitor },
  { id: "api-server" as Page, label: "API Server", icon: Globe },
  { id: "mcp" as Page, label: "MCP", icon: Plug },
  { id: "prompts" as Page, label: "Prompts", icon: MessageSquare },
  { id: "skills" as Page, label: "Skills", icon: Boxes },
  { id: "settings" as Page, label: "设置", icon: Settings },
  // Legacy pages (hidden but accessible)
  // { id: "providers" as Page, label: "Provider (旧)", icon: Server },
  // { id: "switch" as Page, label: "Switch (旧)", icon: ArrowLeftRight },
];

export function Sidebar({ currentPage, onNavigate }: SidebarProps) {
  return (
    <div className="w-56 border-r bg-card p-4">
      <div className="mb-8">
        <h1 className="text-xl font-bold">ProxyCast</h1>
        <p className="text-xs text-muted-foreground">AI API Proxy</p>
      </div>
      <nav className="space-y-1">
        {navItems.map((item) => (
          <button
            key={item.id}
            onClick={() => onNavigate(item.id)}
            className={cn(
              "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors",
              currentPage === item.id
                ? "bg-primary text-primary-foreground"
                : "hover:bg-muted",
            )}
          >
            <item.icon className="h-4 w-4" />
            {item.label}
          </button>
        ))}
      </nav>
    </div>
  );
}
