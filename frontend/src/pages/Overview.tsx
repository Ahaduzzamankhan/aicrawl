import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import {
  ArrowRight,
  Database,
  Globe,
  ListChecks,
  Plug,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Zap,
} from "lucide-react";
import { api } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { PageHeader } from "@/components/PageHeader";
import { StatCard } from "@/components/StatCard";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, Skeleton } from "@/components/ui/skeleton";
import { formatDate, formatDuration, truncate } from "@/lib/utils";

export function Overview() {
  const { data, error, loading, refresh } = usePolling(() => api.overview(), 5000);
  const health = usePolling(() => api.health(), 15000);

  if (loading && !data) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-16 w-full" />
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 4 }).map((_, index) => (
            <Skeleton key={index} className="h-28" />
          ))}
        </div>
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  const stats = data?.stats;

  return (
    <div className="space-y-6">
      <PageHeader
        title="Overview"
        description="One engine for the CLI, REST API, MCP server and this dashboard. Crawl permitted sites; extract structured JSON for humans or agents."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => refresh()}>
              <RefreshCw />
              Refresh
            </Button>
            <Button size="sm" asChild>
              <Link to="/extract" className="inline-flex items-center gap-1.5">
                <Sparkles className="size-4" /> New extraction
              </Link>
            </Button>
          </>
        }
      />

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard
          label="Total jobs"
          value={stats?.jobs_total ?? 0}
          hint={`${stats?.jobs_succeeded ?? 0} succeeded · ${stats?.jobs_failed ?? 0} failed`}
          icon={<ListChecks className="size-4" />}
        />
        <StatCard
          label="Active jobs"
          value={stats?.jobs_running ?? 0}
          hint="queued or running crawls"
          icon={<Zap className="size-4" />}
          tone={stats?.jobs_running ? "warning" : "default"}
        />
        <StatCard
          label="Extracted results"
          value={stats?.results_total ?? 0}
          hint={`${stats?.pages_total ?? 0} pages stored`}
          icon={<Database className="size-4" />}
          tone="success"
        />
        <StatCard
          label="Websites seen"
          value={stats?.websites_total ?? 0}
          hint="cached in SQLite"
          icon={<Globe className="size-4" />}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader className="flex-row items-center justify-between">
            <div>
              <CardTitle>Recent jobs</CardTitle>
              <CardDescription>
                Every CLI, dashboard and MCP request is stored as a job.
              </CardDescription>
            </div>
            <Button variant="ghost" size="sm" asChild>
              <Link to="/jobs" className="inline-flex items-center gap-1">
                All jobs <ArrowRight className="size-3.5" />
              </Link>
            </Button>
          </CardHeader>
          <CardContent>
            {data?.recent_jobs.length ? (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Status</TableHead>
                    <TableHead>Kind</TableHead>
                    <TableHead>URL</TableHead>
                    <TableHead>Pages</TableHead>
                    <TableHead>Created</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.recent_jobs.map((job) => (
                    <TableRow key={job.id}>
                      <TableCell>
                        <StatusBadge status={job.status} />
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {job.kind}
                      </TableCell>
                      <TableCell className="max-w-[22rem]">
                        <span className="block truncate font-mono text-xs">
                          {job.url}
                        </span>
                        {job.query ? (
                          <span className="block truncate text-[11px] text-muted-foreground">
                            {job.query}
                          </span>
                        ) : null}
                      </TableCell>
                      <TableCell className="text-xs tabular-nums">
                        {job.pages_crawled}
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {formatDate(job.created_at)}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            ) : (
              <EmptyState
                icon={<Sparkles className="size-5" />}
                title="No jobs yet"
                description="Run your first extraction from the dashboard, the CLI or an MCP client."
                action={
                  <Button size="sm" asChild>
                    <Link to="/extract">Start an extraction</Link>
                  </Button>
                }
              />
            )}
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <ShieldCheck className="size-4 text-primary" /> Deterministic engine
              </CardTitle>
              <CardDescription>
                CSS and XPath rules, JSON-LD and schema validation. No model, no API key.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-2 text-xs">
              <Row label="Request schema">
                <Badge variant="outline">v{health.data?.schema_version ?? "1"}</Badge>
              </Row>
              <Row label="Selectors">
                <span className="font-mono">CSS · XPath</span>
              </Row>
              <Row label="Renderer">
                {health.data?.renderer.available ? (
                  <Badge variant="success">Playwright</Badge>
                ) : (
                  <Badge variant="outline">HTTP only</Badge>
                )}
              </Row>
              <Row label="AI required">
                <Badge variant="success">never</Badge>
              </Row>
              <div className="pt-2">
                <Button variant="outline" size="sm" className="w-full" asChild>
                  <Link to="/extract" className="inline-flex items-center gap-1">
                    Open request workbench <ArrowRight className="size-3.5" />
                  </Link>
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <ShieldCheck className="size-4 text-success" />
                Safety posture
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-xs">
              <Row label="SSRF protection">
                {health.data?.security.allow_private_networks ? (
                  <Badge variant="warning">private networks allowed</Badge>
                ) : (
                  <Badge variant="success">private IPs blocked</Badge>
                )}
              </Row>
              <Row label="robots.txt">
                {health.data?.security.respect_robots_txt ? (
                  <Badge variant="success">respected</Badge>
                ) : (
                  <Badge variant="warning">disabled</Badge>
                )}
              </Row>
              <Row label="Rate limit">
                <span className="font-mono">
                  {health.data?.security.rate_limit_per_minute ?? "—"}/min per host
                </span>
              </Row>
              <Row label="Max redirects">
                <span className="font-mono">
                  {health.data?.security.max_redirects ?? "—"}
                </span>
              </Row>
              <div className="pt-2">
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full"
                  asChild
                >
                  <Link to="/settings" className="inline-flex items-center gap-1">
                    <ShieldCheck className="size-3.5" /> Review settings
                  </Link>
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>

      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <div>
            <CardTitle>Recent results</CardTitle>
            <CardDescription>
              Validated JSON returned to a human or an AI agent.
            </CardDescription>
          </div>
          <Button variant="ghost" size="sm" asChild>
            <Link to="/results" className="inline-flex items-center gap-1">
              Explore <ArrowRight className="size-3.5" />
            </Link>
          </Button>
        </CardHeader>
        <CardContent>
          {data?.recent_results.length ? (
            <div className="divide-y divide-border/70">
              {data.recent_results.map((result) => (
                <div
                  key={result.id}
                  className="flex flex-col gap-2 py-3 sm:flex-row sm:items-center sm:justify-between"
                >
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">{result.mode}</Badge>
                      <span className="truncate font-mono text-xs">
                        {result.url}
                      </span>
                    </div>
                    <p className="mt-1 truncate text-[11px] text-muted-foreground">
                      {result.query ?? "no query"} · {formatDate(result.created_at)}
                    </p>
                  </div>
                  <div className="flex shrink-0 items-center gap-3 text-[11px] text-muted-foreground">
                    <span>{formatDuration(result.duration_ms)}</span>
                    {result.validated ? (
                      <Badge variant="success">schema</Badge>
                    ) : (
                      <span>{truncate(result.id, 14)}</span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <EmptyState
              title="No results yet"
              description="Results appear here as soon as an extraction completes."
            />
          )}
        </CardContent>
      </Card>

      <Card className="border-primary/20 bg-primary/[0.03]">
        <CardContent className="flex flex-col gap-3 pt-5 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-start gap-3">
            <Plug className="mt-0.5 size-4 text-primary" />
            <div>
              <p className="text-sm font-medium">MCP-native by design</p>
              <p className="text-xs text-muted-foreground">
                Agents call the same engine at{" "}
                <span className="font-mono">
                  {health.data?.mcp.endpoint ?? "http://127.0.0.1:8787/mcp"}
                </span>
                . The dashboard is optional.
              </p>
            </div>
          </div>
          <Button variant="outline" size="sm" asChild>
            <Link to="/mcp" className="inline-flex items-center gap-1">
              MCP setup <ArrowRight className="size-3.5" />
            </Link>
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      {children}
    </div>
  );
}
