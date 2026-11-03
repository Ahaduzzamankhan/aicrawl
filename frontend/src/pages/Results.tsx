import { useMemo, useState } from "react";
import {
  Database,
  FileJson,
  RefreshCw,
  Search,
} from "lucide-react";
import { api, type ExtractedResult } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { PageHeader } from "@/components/PageHeader";
import { StatusBadge, ModeBadge } from "@/components/StatusBadge";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, Skeleton } from "@/components/ui/skeleton";
import { formatDate, formatDuration } from "@/lib/utils";

export function Results() {
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<ExtractedResult | null>(null);
  const { data, error, loading, refresh } = usePolling(
    () => api.results({ limit: 100 }),
    8000,
  );

  const results = useMemo(() => {
    const items = data?.results ?? [];
    if (!search.trim()) return items;
    const needle = search.trim().toLowerCase();
    return items.filter(
      (result) =>
        result.url.toLowerCase().includes(needle) ||
        (result.query ?? "").toLowerCase().includes(needle) ||
        result.mode.includes(needle) ||
        result.id.includes(needle),
    );
  }, [data, search]);

  return (
    <div className="space-y-6">
      <PageHeader
        title="Results explorer"
        description="Every validated JSON payload produced by the CLI, dashboard or an MCP agent, stored in SQLite."
        actions={
          <Button variant="outline" size="sm" onClick={() => refresh()}>
            <RefreshCw /> Refresh
          </Button>
        }
      />

      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <div className="relative w-full sm:max-w-sm">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="Filter by URL, query, mode or id…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            className="pl-9"
          />
        </div>
        <span className="text-[11px] text-muted-foreground sm:ml-auto">
          {results.length} result{results.length === 1 ? "" : "s"}
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
              {Array.from({ length: 4 }).map((_, index) => (
                <Skeleton key={index} className="h-12" />
              ))}
            </div>
          ) : results.length ? (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Mode</TableHead>
                  <TableHead>URL</TableHead>
                  <TableHead>Query</TableHead>
                  <TableHead>Schema</TableHead>
                  <TableHead>Duration</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {results.map((result) => (
                  <TableRow
                    key={result.id}
                    className="cursor-pointer"
                    onClick={() => setSelected(result)}
                  >
                    <TableCell>
                      <ModeBadge mode={result.mode} />
                    </TableCell>
                    <TableCell className="max-w-[18rem] truncate font-mono text-xs">
                      {result.url}
                    </TableCell>
                    <TableCell className="max-w-[16rem] truncate text-xs text-muted-foreground">
                      {result.query ?? "—"}
                    </TableCell>
                    <TableCell>
                      {result.validated ? (
                        <Badge variant="success">validated</Badge>
                      ) : (
                        <span className="text-[11px] text-muted-foreground">
                          —
                        </span>
                      )}
                    </TableCell>
                    <TableCell className="text-xs tabular-nums">
                      {formatDuration(result.duration_ms)}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs text-muted-foreground">
                      {formatDate(result.created_at)}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button variant="ghost" size="sm">
                        <FileJson /> Open
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          ) : (
            <EmptyState
              icon={<Database className="size-5" />}
              title={search ? "No matching results" : "No results yet"}
              description={
                search
                  ? "Try a different filter."
                  : "Run an extraction and its JSON payload will appear here."
              }
            />
          )}
        </CardContent>
      </Card>

      <Dialog
        open={Boolean(selected)}
        onOpenChange={(open) => !open && setSelected(null)}
        className="max-w-4xl"
      >
        <DialogHeader onClose={() => setSelected(null)}>
          <DialogTitle>Extraction result</DialogTitle>
          <DialogDescription>
            <span className="font-mono">{selected?.id}</span>
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="space-y-4">
          {selected ? (
            <>
              <div className="flex flex-wrap items-center gap-2 text-xs">
                <ModeBadge mode={selected.mode} />
                {selected.validated ? (
                  <Badge variant="success">schema validated</Badge>
                ) : null}
                {selected.provider ? (
                  <Badge variant="outline">{selected.provider}</Badge>
                ) : null}
                <span className="ml-auto text-muted-foreground">
                  {formatDuration(selected.duration_ms)} ·{" "}
                  {formatDate(selected.created_at)}
                </span>
              </div>
              <div className="space-y-1 text-xs">
                <p className="truncate">
                  <span className="text-muted-foreground">url · </span>
                  <span className="font-mono">{selected.url}</span>
                </p>
                <p>
                  <span className="text-muted-foreground">query · </span>
                  {selected.query ?? "—"}
                </p>
                <p>
                  <span className="text-muted-foreground">job · </span>
                  <span className="font-mono">{selected.job_id}</span>
                </p>
              </div>
              {selected.warnings.length ? (
                <div className="space-y-1 rounded-md border border-warning/30 bg-warning/10 p-3 text-[11px] text-warning">
                  {selected.warnings.map((warning) => (
                    <p key={warning}>{warning}</p>
                  ))}
                </div>
              ) : null}
              <JsonViewer
                value={selected.data}
                filename={`aicrawl-${selected.id}.json`}
                maxHeight="34rem"
              />
            </>
          ) : null}
        </DialogBody>
      </Dialog>
    </div>
  );
}
