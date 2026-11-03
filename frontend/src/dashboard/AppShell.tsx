import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import {
  Activity,
  BrainCircuit,
  Database,
  Gauge,
  KeyRound,
  ListChecks,
  Menu,
  Moon,
  Plug,
  Settings,
  Sparkles,
  Sun,
  X,
} from "lucide-react";
import { api } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { useTheme } from "@/hooks/useTheme";
import { cn } from "@/lib/utils";

const NAV_GROUPS = [
  {
    label: "Workspace",
    items: [
      { to: "/", label: "Overview", icon: Gauge, end: true },
      { to: "/extract", label: "New Extraction", icon: Sparkles },
      { to: "/jobs", label: "Crawl Jobs", icon: ListChecks },
      { to: "/results", label: "Results Explorer", icon: Database },
    ],
  },
  {
    label: "Integrations",
    items: [
      { to: "/mcp", label: "MCP Configuration", icon: Plug },
      { to: "/api-keys", label: "API Keys", icon: KeyRound },
      { to: "/providers", label: "AI Providers", icon: BrainCircuit },
    ],
  },
  {
    label: "System",
    items: [{ to: "/settings", label: "Settings", icon: Settings }],
  },
];

export function AppShell() {
  const { theme, toggle } = useTheme();
  const [mobileOpen, setMobileOpen] = useState(false);
  const location = useLocation();
  const health = usePolling(() => api.health(), 15000);

  useEffect(() => {
    setMobileOpen(false);
  }, [location.pathname]);

  const online = Boolean(health.data) && health.data?.database.ok !== false;

  return (
    <div className="min-h-dvh bg-background">
      <aside className="fixed inset-y-0 left-0 z-40 hidden w-60 flex-col border-r border-border bg-sidebar lg:flex">
        <SidebarContent online={online} version={health.data?.version} />
      </aside>

      {mobileOpen ? (
        <div className="fixed inset-0 z-50 lg:hidden">
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setMobileOpen(false)}
          />
          <aside className="relative flex h-full w-64 flex-col border-r border-border bg-sidebar">
            <button
              onClick={() => setMobileOpen(false)}
              className="absolute right-3 top-3 rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
              aria-label="Close navigation"
            >
              <X className="size-4" />
            </button>
            <SidebarContent online={online} version={health.data?.version} />
          </aside>
        </div>
      ) : null}

      <div className="lg:pl-60">
        <header className="sticky top-0 z-30 flex h-14 items-center gap-3 border-b border-border bg-background/85 px-4 backdrop-blur sm:px-6">
          <button
            onClick={() => setMobileOpen(true)}
            className="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground lg:hidden"
            aria-label="Open navigation"
          >
            <Menu className="size-5" />
          </button>

          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Activity className="size-3.5" />
            <span className="font-mono">
              {health.data?.api.host}:{health.data?.api.port}
            </span>
          </div>

          <div className="ml-auto flex items-center gap-2">
            <div
              className={cn(
                "flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium",
                online
                  ? "border-success/30 bg-success/10 text-success"
                  : "border-destructive/30 bg-destructive/10 text-destructive",
              )}
              title={health.error ?? undefined}
            >
              <span
                className={cn(
                  "size-1.5 rounded-full",
                  online ? "bg-success" : "bg-destructive",
                )}
              />
              {online ? "engine online" : health.loading ? "connecting" : "offline"}
            </div>
            <button
              onClick={toggle}
              className="rounded-md p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
              aria-label="Toggle theme"
            >
              {theme === "dark" ? (
                <Sun className="size-4" />
              ) : (
                <Moon className="size-4" />
              )}
            </button>
          </div>
        </header>

        <main className="mx-auto w-full max-w-7xl p-4 sm:p-6 lg:p-8">
          {health.error ? (
            <div className="mb-4 rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
              {health.error}
            </div>
          ) : null}
          <Outlet />
        </main>

        <footer className="mx-auto flex w-full max-w-7xl flex-col gap-1 px-4 pb-8 text-[11px] text-muted-foreground sm:flex-row sm:items-center sm:justify-between sm:px-6 lg:px-8">
          <span>
            AICrawl v{health.data?.version ?? "0.0.1-experimental.1"} · Rust-powered crawling ·
            SQLite storage
          </span>
          <span>
            No CAPTCHA bypass, no paywall circumvention, robots.txt respected.
          </span>
        </footer>
      </div>
    </div>
  );
}

function SidebarContent({
  online,
  version,
}: {
  online: boolean;
  version?: string;
}) {
  return (
    <>
      <div className="flex h-14 items-center gap-2 border-b border-border px-4">
        <span className="grid size-7 place-items-center rounded-md bg-primary/15 text-primary">
          ✦
        </span>
        <div className="leading-tight">
          <p className="text-sm font-semibold tracking-tight">AICrawl</p>
          <p className="text-[10px] text-muted-foreground">
            extraction engine v{version ?? "0.0.1-experimental.1"}
          </p>
        </div>
      </div>

      <nav className="scrollbar-thin flex-1 space-y-5 overflow-y-auto px-3 py-4">
        {NAV_GROUPS.map((group) => (
          <div key={group.label}>
            <p className="px-2 pb-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground/70">
              {group.label}
            </p>
            <div className="space-y-0.5">
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.end}
                  className={({ isActive }) =>
                    cn(
                      "flex items-center gap-2.5 rounded-md px-2 py-1.5 text-[13px] transition-colors",
                      isActive
                        ? "bg-accent text-accent-foreground"
                        : "text-muted-foreground hover:bg-muted hover:text-foreground",
                    )
                  }
                >
                  <item.icon className="size-4" />
                  {item.label}
                </NavLink>
              ))}
            </div>
          </div>
        ))}
      </nav>

      <div className="border-t border-border p-3">
        <div className="rounded-lg border border-border bg-card px-3 py-2.5">
          <div className="flex items-center gap-2 text-[11px]">
            <span
              className={cn(
                "size-1.5 rounded-full",
                online ? "bg-success" : "bg-destructive",
              )}
            />
            <span className="font-medium">
              {online ? "SQLite connected" : "engine offline"}
            </span>
          </div>
          <p className="mt-1 text-[10px] leading-relaxed text-muted-foreground">
            MCP · REST · CLI share one engine
          </p>
        </div>
      </div>
    </>
  );
}
