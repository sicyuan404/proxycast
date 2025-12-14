import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { Dashboard } from "./components/Dashboard";
import { Providers } from "./components/Providers";
import { SettingsPage } from "./components/settings";
import { SwitchPage } from "./components/switch";
import { ClientsPage } from "./components/clients";
import { McpPage } from "./components/mcp";
import { PromptsPage } from "./components/prompts";
import { CredentialsPage } from "./components/credentials/CredentialsPage";
import { ApiServerPage } from "./components/api-server/ApiServerPage";
import { SkillsPage } from "./components/skills";

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

function App() {
  const [currentPage, setCurrentPage] = useState<Page>("dashboard");

  const renderPage = () => {
    switch (currentPage) {
      case "dashboard":
        return <Dashboard />;
      case "credentials":
        return <CredentialsPage />;
      case "clients":
        return <ClientsPage />;
      case "api-server":
        return <ApiServerPage />;
      case "providers":
        return <Providers />;
      case "settings":
        return <SettingsPage />;
      case "switch":
        return <SwitchPage />;
      case "mcp":
        return <McpPage />;
      case "prompts":
        return <PromptsPage />;
      case "skills":
        return <SkillsPage />;
      default:
        return <Dashboard />;
    }
  };

  return (
    <div className="flex h-screen bg-background">
      <Sidebar currentPage={currentPage} onNavigate={setCurrentPage} />
      <main className="flex-1 overflow-auto p-6">{renderPage()}</main>
    </div>
  );
}

export default App;
