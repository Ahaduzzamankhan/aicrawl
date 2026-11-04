import { useEffect, useState } from "react";
import {
  Ban,
  ExternalLink,
  ListChecks,
  Loader2,
  Plus,
  RefreshCw,
  Search,
} from "lucide-react";
import {
  ApiError,
  api,
  type CrawlPage,
  type ExtractedResult,
  type Job,
  type JobDetail,
} from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/PageHeader";
import { StatusBadge } from "@/components/StatusBadge";
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
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, Skeleton } from "@/components/ui/skeleton";
import { formatDate, truncate } from "@/lib/utils";

const FILTERS = ["", "running", "queued", "succeeded", "failed", "cancelled"];

export function Jobs() {
  const toast = useToast();
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<JobDetail | null>(null);
  const [creating, setCreating] = useState(false);
  const { data, error, loading, refresh } = usePolling(
    () => api.jobs({ status: filter || undefined, limit: 100 }),
    3000,
  );

  useEffect(() => {
    if (!selected) {
      setDetail(null);
      return;
    }
    let active = true;
    const load = async () => {
      try {
        const next = await api.job(selected);
        if (active) setDetail(next);
      } catch {
        /* surfaced by the dialog */
      }
    };
    void load();
    const timer = window.setInterval(load, 2000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [selected]);

  const cancel = async (id: string) => {
    try {
      await api.cancelJob(id);
      toast.info("Cancellation requested", id);
      refresh();
    } catch (cause) {
      toast.error(
        "Cannot cancel job",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    }
  };

  const jobs = data?.jobs ?? [];

  return (
    <div className="space-y-6">
      <PageHeader
        title="Crawl jobs"
        description="Both extractions and multi-page crawls are jobs. Running crawls can be cancelled and results are stored in SQLite."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => refresh()}>
              <RefreshCw /> Refresh
            </Button>
            <Button size="sm" onClick={() => setCreating(true)}>
              <Plus /> New crawl
            </Button>
          </>
        }
      />

      <div className="flex flex-wrap items-center gap-2">
        {FILTERS.map((value) => (
          <button
            key={value || "all"}
            onClick={() => setFilter(value)}
            className={
              filter === value
                ? "rounded-full border border-primary/40 bg-primary/10 px-3 py-1 text-xs font-medium text-primary"
                : "rounded-full border border-border px-3 py-1 text-xs text-muted-foreground transition hover:text-foreground"
            }
          >
            {value || "all"}
          </button>
        ))}
        <span className="ml-auto text-[11px] text-muted-foreground">
          auto-refresh every 3s
        </span>
      </div>

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <Card>
        <CardContent className="pt-5">
          {loading && !data ? (
            <div className="space-y-2">
              {Array.from({ length: 5 }).map((_, index) => (
                <Skeleton key={index} className="h-12" />
              ))}
            </div>
          ) : jobs.length ? (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Status</TableHead>
                  <TableHead>Kind</TableHead>
                  <TableHead>URL / query</TableHead>
                  <TableHead>Pages</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {jobs.map((job) => (
                  <TableRow key={job.id}>
                    <TableCell>
                      <StatusBadge status={job.status} />
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {job.kind}
                    </TableCell>
                    <TableCell className="max-w-[26rem]">
                      <p className="truncate font-mono text-xs">{job.url}</p>
                      {job.query ? (
                        <p className="truncate text-[11px] text-muted-foreground">
                          {job.query}
                        </p>
                      ) : null}
                      <p className="truncate text-[10px] text-muted-foreground/70">
                        {job.id}
                      </p>
                    </TableCell>
                    <TableCell className="text-xs tabular-nums">
                      {job.pages_crawled}
                      {job.max_pages ? (
                        <span className="text-muted-foreground">
                          /{job.max_pages}
                        </span>
                      ) : null}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs text-muted-foreground">
                      {formatDate(job.created_at)}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setSelected(job.id)}
                        >
                          View
                        </Button>
                        {job.status === "running" || job.status === "queued" ? (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => cancel(job.id)}
                          >
                            <Ban /> Cancel
                          </Button>
                        ) : null}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          ) : (
            <EmptyState
              icon={<ListChecks className="size-5" />}
              title="No jobs for this filter"
              description="Start an extraction or a crawl to populate the queue."
              action={
                <Button size="sm" onClick={() => setCreating(true)}>
                  <Plus /> New crawl
                </Button>
              }
            />
          )}
        </CardContent>
      </Card>

      <JobDialog
        jobId={selected}
        detail={detail}
        onClose={() => setSelected(null)}
        onCancel={cancel}
      />
      <NewCrawlDialog open={creating} onOpenChange={setCreating} onCreated={refresh} />
    </div>
  );
}

function JobDialog({
  jobId,
  detail,
  onClose,
  onCancel,
}: {
  jobId: string | null;
  detail: JobDetail | null;
  onClose: () => void;
  onCancel: (id: string) => void;
}) {
  return (
    <Dialog open={Boolean(jobId)} onOpenChange={(open) => !open && onClose()} className="max-w-4xl">
      <DialogHeader onClose={onClose}>
        <DialogTitle>Job detail</DialogTitle>
        <DialogDescription>
          <span className="font-mono">{jobId}</span>
        </DialogDescription>
      </DialogHeader>
      <DialogBody className="space-y-5">
        {detail ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <StatusBadge status={detail.job.status} />
              <Badge variant="outline">{detail.job.kind}</Badge>
              <span className="font-mono text-xs text-muted-foreground">
                {detail.job.url}
              </span>
              {detail.job.status === "running" ||
              detail.job.status === "queued" ? (
                <Button
                  size="sm"
                  variant="destructive"
                  className="ml-auto"
                  onClick={() => onCancel(detail.job.id)}
                >
                  <Ban /> Cancel job
                </Button>
              ) : null}
            </div>

            <div className="grid gap-2 text-xs sm:grid-cols-2">
              <Field label="Query" value={detail.job.query ?? "—"} />
              <Field label="Created" value={formatDate(detail.job.created_at)} />
              <Field
                label="Started"
                value={formatDate(detail.job.started_at)}
              />
              <Field
                label="Finished"
                value={formatDate(detail.job.finished_at)}
              />
              <Field label="Pages" value={String(detail.job.pages_crawled)} />
              <Field
                label="Limits"
                value={`${detail.job.max_pages ?? "—"} pages · depth ${detail.job.max_depth ?? "—"}`}
              />
            </div>

            {detail.job.error ? (
              <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
                {detail.job.error}
              </div>
            ) : null}

            {detail.results.length ? (
              <section className="space-y-2">
                <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Results ({detail.results.length})
                </h3>
                {detail.results.map((result) => (
                  <ResultBlock key={result.id} result={result} />
                ))}
              </section>
            ) : null}

            <section className="space-y-2">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Pages ({detail.pages.length})
              </h3>
              <PagesTable pages={detail.pages} />
            </section>
          </>
        ) : (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="size-4 animate-spin" /> Loading job…
          </div>
        )}
      </DialogBody>
    </Dialog>
  );
}

function ResultBlock({ result }: { result: ExtractedResult }) {
  return (
    <details className="rounded-lg border border-border bg-muted/20 p-3">
      <summary className="cursor-pointer list-none">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="outline">{result.mode}</Badge>
          <span className="font-mono text-[11px]">{result.url}</span>
          {result.validated ? <Badge variant="success">schema</Badge> : null}
          <span className="ml-auto text-[10px] text-muted-foreground">
            {result.id}
          </span>
        </div>
      </summary>
      <div className="mt-3">
        <JsonViewer value={result.data} filename={`${result.id}.json`} maxHeight="18rem" />
      </div>
    </details>
  );
}

function PagesTable({ pages }: { pages: CrawlPage[] }) {
  if (!pages.length) {
    return (
      <p className="text-xs text-muted-foreground">No pages recorded yet.</p>
    );
  }
  return (
    <div className="rounded-lg border border-border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Depth</TableHead>
            <TableHead>Title</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Words</TableHead>
            <TableHead>URL</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {pages.map((page) => (
            <TableRow key={page.id}>
              <TableCell className="text-xs">{page.depth}</TableCell>
              <TableCell className="max-w-[14rem] truncate text-xs">
                {page.title ?? "(untitled)"}
              </TableCell>
              <TableCell className="text-xs">
                {page.error ? (
                  <Badge variant="destructive">error</Badge>
                ) : (
                  <span className="font-mono">{page.status_code ?? "—"}</span>
                )}
              </TableCell>
              <TableCell className="text-xs tabular-nums">
                {page.word_count}
              </TableCell>
              <TableCell className="max-w-[18rem] truncate font-mono text-[11px] text-muted-foreground">
                {page.url}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border/70 px-3 py-2">
      <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
        {label}
      </p>
      <p className="truncate text-xs">{value}</p>
    </div>
  );
}

function NewCrawlDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}) {
  const toast = useToast();
  const [url, setUrl] = useState("https://example.com");
  const [maxPages, setMaxPages] = useState(10);
  const [maxDepth, setMaxDepth] = useState(2);
  const [sameDomain, setSameDomain] = useState(true);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      const response = await api.crawl({
        version: "1",
        url: url.trim(),
        action: "crawl",
        crawl: {
          max_pages: maxPages,
          max_depth: maxDepth,
          same_domain: sameDomain,
          sitemap: true,
        },
      });
      if (!response.success) {
        toast.error(
          response.error?.code ?? "crawl failed",
          response.error?.message ?? "The crawl was rejected",
        );
        return;
      }
      toast.success("Crawl complete", response.meta?.job_id ?? undefined);
      onOpenChange(false);
      onCreated();
    } catch (error) {
      toast.error(
        "Could not start crawl",
        error instanceof ApiError ? error.message : String(error),
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogHeader onClose={() => onOpenChange(false)}>
        <DialogTitle>New crawl</DialogTitle>
        <DialogDescription>
          Bounded breadth-first crawl. robots.txt and the SSRF policy always apply.
        </DialogDescription>
      </DialogHeader>
      <DialogBody className="space-y-4">
        <div className="space-y-1.5">
          <Label htmlFor="crawl-url">Starting URL</Label>
          <Input
            id="crawl-url"
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            spellCheck={false}
          />
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="max-pages">Max pages</Label>
            <Input
              id="max-pages"
              type="number"
              min={1}
              max={500}
              value={maxPages}
              onChange={(event) =>
                setMaxPages(Math.max(1, Number(event.target.value)))
              }
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="max-depth">Max depth</Label>
            <Input
              id="max-depth"
              type="number"
              min={0}
              max={5}
              value={maxDepth}
              onChange={(event) =>
                setMaxDepth(Math.min(5, Math.max(0, Number(event.target.value))))
              }
            />
          </div>
        </div>
        <div className="flex items-center justify-between rounded-md border border-border px-3 py-2">
          <div>
            <p className="text-xs font-medium">Stay on this domain</p>
            <p className="text-[10px] text-muted-foreground">
              Ignore links to other hosts
            </p>
          </div>
          <Switch
            checked={sameDomain}
            onCheckedChange={setSameDomain}
            label="Stay on this domain"
          />
        </div>
        <Button className="w-full" onClick={submit} disabled={busy}>
          {busy ? <Loader2 className="animate-spin" /> : <Search />}
          Start crawl
        </Button>
      </DialogBody>
    </Dialog>
  );
}
