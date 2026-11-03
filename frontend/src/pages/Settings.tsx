import { useEffect, useState, type ReactNode } from "react";
import {
  Database,
  Loader2,
  Moon,
  Save,
  Server,
  ShieldCheck,
  Sun,
  Trash2,
} from "lucide-react";
import { ApiError, api } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { useTheme } from "@/hooks/useTheme";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/PageHeader";
import { JsonViewer } from "@/components/JsonViewer";
import { StatusBadge } from "@/components/StatusBadge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

export function Settings() {
  const toast = useToast();
  const { theme, setTheme } = useTheme();
  const { data, loading, error, refresh } = usePolling(
    () => api.settings(),
    20000,
  );
  const [maxPages, setMaxPages] = useState("10");
  const [format, setFormat] = useState("json");
  const [saving, setSaving] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [hydrated, setHydrated] = useState(false);

  useEffect(() => {
    if (!data || hydrated) return;
    setMaxPages(data.preferences.default_max_pages ?? "10");
    setFormat(data.preferences.default_output_format ?? "json");
    setHydrated(true);
  }, [data, hydrated]);

  const save = async () => {
    setSaving(true);
    try {
      await api.putSettings({
        theme,
        default_max_pages: String(Math.max(1, Number(maxPages) || 10)),
        default_output_format: format,
      });
      toast.success("Preferences saved");
      refresh();
    } catch (cause) {
      toast.error(
        "Could not save preferences",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    } finally {
      setSaving(false);
    }
  };

  const clearCache = async () => {
    setClearing(true);
    try {
      await api.clearCache();
      toast.success("Page cache cleared");
      refresh();
    } catch (cause) {
      toast.error(
        "Could not clear cache",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    } finally {
      setClearing(false);
    }
  };

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
        description="Runtime configuration, security posture and storage. Server-side options are edited in config.toml or with the CLI."
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
            <CardTitle>Dashboard preferences</CardTitle>
            <CardDescription>
              Stored locally and in SQLite; they do not change crawl limits.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label>Theme</Label>
              <div className="flex gap-2">
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
              </div>
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              <div className="space-y-1.5">
                <Label htmlFor="default-pages">Default max pages</Label>
                <Input
                  id="default-pages"
                  type="number"
                  min={1}
                  max={500}
                  value={maxPages}
                  onChange={(event) => setMaxPages(event.target.value)}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="default-format">Default output</Label>
                <Select
                  id="default-format"
                  value={format}
                  onChange={(event) => setFormat(event.target.value)}
                >
                  <option value="json">JSON</option>
                  <option value="markdown">Markdown</option>
                  <option value="text">Text</option>
                </Select>
              </div>
            </div>

            <Button onClick={save} disabled={saving} className="w-full">
              {saving ? <Loader2 className="animate-spin" /> : <Save />}
              Save preferences
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck className="size-4 text-success" /> Security posture
            </CardTitle>
            <CardDescription>
              Enforced by the shared engine for CLI, API and MCP.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-2 text-xs">
            <SettingRow
              label="Private networks"
              value={
                data?.security.allow_private_networks ? (
                  <Badge variant="warning">allowed</Badge>
                ) : (
                  <Badge variant="success">blocked (SSRF safe)</Badge>
                )
              }
            />
            <SettingRow
              label="robots.txt"
              value={
                data?.security.respect_robots_txt ? (
                  <Badge variant="success">respected</Badge>
                ) : (
                  <Badge variant="warning">disabled</Badge>
                )
              }
            />
            <SettingRow
              label="Per-host rate limit"
              value={
                <span className="font-mono">
                  {data?.security.rate_limit_per_minute ?? "—"}/min
                </span>
              }
            />
            <SettingRow
              label="Max redirects"
              value={
                <span className="font-mono">
                  {data?.security.max_redirects ?? "—"}
                </span>
              }
            />
            <SettingRow
              label="Allowed domains"
              value={
                <span className="font-mono text-[10px]">
                  {data?.security.allowed_domains.length
                    ? data.security.allowed_domains.join(", ")
                    : "any (policy permitting)"}
                </span>
              }
            />
            <SettingRow
              label="Blocked domains"
              value={
                <span className="font-mono text-[10px]">
                  {data?.security.blocked_domains.length
                    ? data.security.blocked_domains.join(", ")
                    : "none"}
                </span>
              }
            />
            <p className="pt-2 text-[10px] leading-relaxed text-muted-foreground">
              Change these in <code>config.toml</code> under{" "}
              <code>[security]</code>; restart the server afterwards.
            </p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              <Database className="size-4 text-primary" /> Storage & cache
            </CardTitle>
            <CardDescription>
              SQLite is the only database AICrawl uses.
            </CardDescription>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={clearCache}
            disabled={clearing}
          >
            {clearing ? <Loader2 className="animate-spin" /> : <Trash2 />}
            Clear cache
          </Button>
        </CardHeader>
        <CardContent className="space-y-2 text-xs">
          <SettingRow
            label="Database"
            value={
              <span className="font-mono text-[10px]">
                {data?.storage.path ?? "—"}
              </span>
            }
          />
          <SettingRow
            label="Data directory"
            value={
              <span className="font-mono text-[10px]">
                {data?.storage.data_dir ?? "—"}
              </span>
            }
          />
          <SettingRow
            label="Cached pages"
            value={String(data?.storage.cache_entries ?? 0)}
          />
          <SettingRow
            label="Stored results"
            value={String(data?.storage.stats.results_total ?? 0)}
          />
          <SettingRow
            label="Stored pages"
            value={String(data?.storage.stats.pages_total ?? 0)}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server className="size-4" /> Runtime
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-2 text-xs sm:grid-cols-2">
          <SettingRow
            label="Version"
            value={<span className="font-mono">{data?.runtime.version}</span>}
          />
          <SettingRow
            label="API key auth"
            value={
              data?.runtime.api_requires_key ? (
                <Badge variant="warning">required</Badge>
              ) : (
                <Badge variant="outline">loopback / open</Badge>
              )
            }
          />
          <SettingRow
            label="MCP endpoint"
            value={
              <span className="font-mono text-[10px]">
                {data?.runtime.mcp_endpoint}
              </span>
            }
          />
          <SettingRow
            label="Result modes"
            value={<StatusBadge status="succeeded" />}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Effective configuration</CardTitle>
          <CardDescription>
            Read-only view (secrets are never included).
          </CardDescription>
        </CardHeader>
        <CardContent>
          <JsonViewer
            value={data?.effective}
            filename="aicrawl-config.json"
            maxHeight="24rem"
          />
        </CardContent>
      </Card>
    </div>
  );
}

function SettingRow({
  label,
  value,
}: {
  label: string;
  value: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-border/50 py-1.5 last:border-0">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-right">{value}</span>
    </div>
  );
}
