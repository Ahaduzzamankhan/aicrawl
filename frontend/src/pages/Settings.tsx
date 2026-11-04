import type { ReactNode } from "react";
import {
  Cpu,
  Database,
  Moon,
  Server,
  ShieldCheck,
  Sun,
  Wrench,
} from "lucide-react";
import { api } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { useTheme } from "@/hooks/useTheme";
import { PageHeader } from "@/components/PageHeader";
import { JsonViewer } from "@/components/JsonViewer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

export function Settings() {
  const { theme, setTheme } = useTheme();
  const { data, loading, error, refresh } = usePolling(() => api.settings(), 20000);

  if (loading && !data) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-20 w-full" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Settings"
        description="Runtime configuration, security posture and storage. Server options come from config.toml; AICrawl never needs an AI provider."
        actions={
          <Button variant="outline" size="sm" onClick={() => refresh()}>
            Reload
          </Button>
        }
      />

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Appearance</CardTitle>
            <CardDescription>Stored in this browser only.</CardDescription>
          </CardHeader>
          <CardContent className="flex gap-2">
            <button
              onClick={() => setTheme("dark")}
              className={cn(
                "flex flex-1 items-center justify-center gap-2 rounded-md border px-3 py-2 text-xs transition",
                theme === "dark"
                  ? "border-primary/40 bg-primary/10 text-primary"
                  : "border-border text-muted-foreground hover:text-foreground",
              )}
            >
              <Moon className="size-3.5" /> Dark
            </button>
            <button
              onClick={() => setTheme("light")}
              className={cn(
                "flex flex-1 items-center justify-center gap-2 rounded-md border px-3 py-2 text-xs transition",
                theme === "light"
                  ? "border-primary/40 bg-primary/10 text-primary"
                  : "border-border text-muted-foreground hover:text-foreground",
              )}
            >
              <Sun className="size-3.5" /> Light
            </button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck className="size-4 text-success" /> Security posture
            </CardTitle>
            <CardDescription>Enforced by the shared engine for CLI, API and MCP.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-2 text-xs">
            <Row
              label="Private networks"
              value={
                data?.security.allow_private_networks ? (
                  <Badge variant="warning">allowed</Badge>
                ) : (
                  <Badge variant="success">blocked (SSRF safe)</Badge>
                )
              }
            />
            <Row
              label="robots.txt"
              value={
                data?.security.respect_robots_txt ? (
                  <Badge variant="success">respected</Badge>
                ) : (
                  <Badge variant="warning">disabled</Badge>
                )
              }
            />
            <Row
              label="Per-host rate limit"
              value={
                <span className="font-mono">{data?.security.rate_limit_per_minute ?? "—"}/min</span>
              }
            />
            <Row
              label="Max redirects"
              value={<span className="font-mono">{data?.security.max_redirects ?? "—"}</span>}
            />
            <Row
              label="Allowed domains"
              value={
                <span className="font-mono text-[10px]">
                  {data?.security.allowed_domains.length
                    ? data.security.allowed_domains.join(", ")
                    : "any (policy permitting)"}
                </span>
              }
            />
            <Row
              label="Blocked domains"
              value={
                <span className="font-mono text-[10px]">
                  {data?.security.blocked_domains.length
                    ? data.security.blocked_domains.join(", ")
                    : "none"}
                </span>
              }
            />
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Cpu className="size-4 text-primary" /> Extraction
            </CardTitle>
            <CardDescription>Limits applied to every request.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-2 text-xs">
            <Row
              label="Max items per list"
              value={<span className="font-mono">{data?.extract.max_items ?? "—"}</span>}
            />
            <Row
              label="Max text chars"
              value={<span className="font-mono">{data?.extract.max_text_chars ?? "—"}</span>}
            />
            <Row
              label="Max pages / depth"
              value={
                <span className="font-mono">
                  {data?.crawler.max_pages ?? "—"} / {data?.crawler.max_depth ?? "—"}
                </span>
              }
            />
            <Row
              label="Concurrency"
              value={<span className="font-mono">{data?.crawler.concurrency ?? "—"}</span>}
            />
            <Row
              label="Crawl budget"
              value={<span className="font-mono">{data?.crawler.max_crawl_secs ?? "—"}s</span>}
            />
            <Row
              label="Render timeout"
              value={<span className="font-mono">{data?.crawler.render_timeout_ms ?? "—"}ms</span>}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Database className="size-4 text-primary" /> Storage
            </CardTitle>
            <CardDescription>SQLite is the only database AICrawl uses.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-2 text-xs">
            <Row
              label="Database"
              value={<span className="break-all font-mono text-[10px]">{data?.database}</span>}
            />
            <Row
              label="Data directory"
              value={<span className="break-all font-mono text-[10px]">{data?.data_dir}</span>}
            />
            <Row
              label="Cache"
              value={
                <Badge variant={data?.cache.enabled ? "success" : "outline"}>
                  {data?.cache.enabled ? "enabled" : "off"}
                </Badge>
              }
            />
            <Row
              label="Cache TTL"
              value={<span className="font-mono">{data?.cache.ttl_secs ?? "—"}s</span>}
            />
            <Row
              label="Max body"
              value={
                <span className="font-mono">
                  {Math.round((data?.crawler.max_body_bytes ?? 0) / 1024)} KB
                </span>
              }
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Server className="size-4" /> Runtime
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-xs">
            <Row
              label="Version"
              value={<span className="font-mono">{data?.version}</span>}
            />
            <Row
              label="Request schema"
              value={<Badge variant="outline">v{data?.schema_version}</Badge>}
            />
            <Row
              label="API key auth"
              value={
                data?.api.require_api_key ? (
                  <Badge variant="warning">required</Badge>
                ) : (
                  <Badge variant="outline">loopback / open</Badge>
                )
              }
            />
            <Row
              label="API"
              value={
                <span className="font-mono text-[10px]">
                  {data?.server.api_host}:{data?.server.api_port}
                </span>
              }
            />
            <Row
              label="MCP"
              value={
                <span className="font-mono text-[10px]">
                  {data?.server.mcp_host}:{data?.server.mcp_port}
                </span>
              }
            />
            <Row label="AI provider" value={<Badge variant="success">not required</Badge>} />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Wrench className="size-4" /> Edit server configuration
          </CardTitle>
          <CardDescription>
            Use <span className="font-mono">aicrawl config show</span> or edit{" "}
            <span className="font-mono">config.toml</span>, then restart the server.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <JsonViewer value={data} filename="aicrawl-settings.json" maxHeight="26rem" />
        </CardContent>
      </Card>
    </div>
  );
}

function Row({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-border/50 py-1.5 last:border-0">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-right">{value}</span>
    </div>
  );
}
