import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  Download,
  History,
  Loader2,
  Play,
  Save,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import {
  ApiError,
  api,
  type ExtractionResponse,
  type Issue,
} from "@/lib/api";
import { PageHeader } from "@/components/PageHeader";
import { JsonViewer } from "@/components/JsonViewer";
import { useToast } from "@/components/ui/toast";
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
import { copyText, formatDuration } from "@/lib/utils";

const TEMPLATE = `{
  "version": "1",
  "url": "https://example.com/products",
  "action": "extract",
  "render": "auto",
  "selectors": {
    "products": {
      "type": "list",
      "selector": ".product-card",
      "fields": {
        "name": { "selector": ".product-title", "required": true },
        "price": { "selector": ".price", "type": "number" },
        "link": { "selector": "a", "type": "url" }
      }
    }
  },
  "pagination": { "enabled": false, "next_selector": "a.next", "max_pages": 5 },
  "deduplicate": { "enabled": true, "keys": ["name"] },
  "output": { "format": "json", "pretty": true }
}`;

const PRESETS: { label: string; value: string }[] = [
  {
    label: "Document summary",
    value: `{
  "version": "1",
  "url": "https://example.com",
  "action": "extract"
}`,
  },
  {
    label: "Crawl a site",
    value: `{
  "version": "1",
  "url": "https://example.com",
  "action": "crawl",
  "crawl": { "max_pages": 10, "max_depth": 2, "same_domain": true, "sitemap": true },
  "selectors": {
    "title": { "selector": "h1" },
    "description": { "type": "meta", "name": "description" }
  }
}`,
  },
  {
    label: "Batch URLs",
    value: `{
  "version": "1",
  "action": "batch",
  "urls": ["https://example.com", "https://example.com/about"],
  "selectors": { "title": { "selector": "h1", "required": true } }
}`,
  },
  {
    label: "Map a domain",
    value: `{
  "version": "1",
  "url": "https://example.com",
  "action": "map",
  "crawl": { "max_pages": 50, "max_depth": 2, "sitemap": true }
}`,
  },
  {
    label: "JSON-LD + Open Graph",
    value: `{
  "version": "1",
  "url": "https://example.com",
  "selectors": {
    "ld": { "type": "json_ld" },
    "og_title": { "type": "meta", "name": "og:title" },
    "headings": { "type": "list", "selector": "h2", "fields": { "text": { "selector": ":scope" } } }
  }
}`,
  },
];

interface HistoryEntry {
  at: string;
  request: string;
  success: boolean;
  jobId?: string | null;
  items?: number;
}

const HISTORY_KEY = "aicrawl.history";
const TEMPLATES_KEY = "aicrawl.templates";

export function NewExtraction() {
  const toast = useToast();
  const [text, setText] = useState(TEMPLATE);
  const [url, setUrl] = useState("https://example.com/products");
  const [action, setAction] = useState("extract");
  const [response, setResponse] = useState<ExtractionResponse | null>(null);
  const [busy, setBusy] = useState<"validate" | "run" | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [templates, setTemplates] = useState<Record<string, string>>({});
  const [templateName, setTemplateName] = useState("");

  useEffect(() => {
    try {
      setHistory(JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]"));
      setTemplates(JSON.parse(localStorage.getItem(TEMPLATES_KEY) ?? "{}"));
    } catch {
      /* ignore malformed local storage */
    }
  }, []);

  const parsed = useMemo(() => {
    try {
      const value = JSON.parse(text) as Record<string, unknown>;
      setParseError(null);
      return value;
    } catch (error) {
      setParseError(error instanceof Error ? error.message : String(error));
      return null;
    }
  }, [text]);

  /** Keeps the URL/action inputs in sync with the editor without clobbering JSON. */
  const applyControls = () => {
    if (!parsed) return null;
    const next = { ...parsed };
    if (url.trim()) next.url = url.trim();
    if (action) next.action = action;
    return next;
  };

  const run = async (mode: "validate" | "run") => {
    const payload = applyControls();
    if (!payload) {
      toast.error("Request is not valid JSON", parseError ?? undefined);
      return;
    }
    setBusy(mode);
    try {
      const result =
        mode === "validate"
          ? await api.validate(payload)
          : payload.action === "crawl"
            ? await api.crawl(payload)
            : payload.action === "batch"
              ? await api.batch(payload)
              : payload.action === "map"
                ? await api.map(payload)
                : await api.run(payload);
      setResponse(result);
      if (mode === "run") {
        const entry: HistoryEntry = {
          at: new Date().toISOString(),
          request: JSON.stringify(payload, null, 2),
          success: result.success,
          jobId: result.meta?.job_id,
          items: result.meta?.items_extracted,
        };
        const next = [entry, ...history].slice(0, 12);
        setHistory(next);
        localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
      }
      if (result.success) {
        toast.success(
          mode === "validate" ? "Request is valid" : "Extraction complete",
          result.meta?.job_id ?? undefined,
        );
      } else {
        toast.error(result.error?.code ?? "Request failed", result.error?.message);
      }
    } catch (error) {
      const cause = error instanceof ApiError ? error : new ApiError(String(error));
      toast.error(cause.code, cause.message);
    } finally {
      setBusy(null);
    }
  };

  const download = () => {
    if (!response) return;
    const blob = new Blob([JSON.stringify(response, null, 2)], { type: "application/json" });
    const link = document.createElement("a");
    link.href = URL.createObjectURL(blob);
    link.download = `aicrawl-${response.meta?.job_id ?? "result"}.json`;
    link.click();
    URL.revokeObjectURL(link.href);
  };

  const saveTemplate = () => {
    const name = templateName.trim();
    if (!name) {
      toast.error("Name the template first");
      return;
    }
    const next = { ...templates, [name]: text };
    setTemplates(next);
    localStorage.setItem(TEMPLATES_KEY, JSON.stringify(next));
    setTemplateName("");
    toast.success("Template saved", name);
  };

  const removeTemplate = (name: string) => {
    const next = { ...templates };
    delete next[name];
    setTemplates(next);
    localStorage.setItem(TEMPLATES_KEY, JSON.stringify(next));
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title="Request workbench"
        description="AICrawl is JSON-first. Describe the page and the fields you want with CSS or XPath rules; the same request works in the CLI, REST API and MCP tools."
        actions={<Badge variant="outline">schema v1 · no AI required</Badge>}
      />

      <div className="grid gap-4 xl:grid-cols-2">
        <Card className="flex flex-col">
          <CardHeader className="gap-3">
            <div className="flex items-center justify-between gap-2">
              <div>
                <CardTitle>Request JSON</CardTitle>
                <CardDescription>Validate locally, then run against the engine.</CardDescription>
              </div>
              <Select
                value=""
                onChange={(event) => {
                  const preset = PRESETS.find((item) => item.label === event.target.value);
                  if (preset) setText(preset.value);
                }}
                aria-label="Load a preset"
                className="w-44"
              >
                <option value="">Load preset…</option>
                {PRESETS.map((preset) => (
                  <option key={preset.label} value={preset.label}>
                    {preset.label}
                  </option>
                ))}
                {Object.keys(templates).map((name) => (
                  <option key={`saved-${name}`} value={`saved:${name}`}>
                    {`saved: ${name}`}
                  </option>
                ))}
              </Select>
            </div>

            <div className="grid gap-3 sm:grid-cols-[1fr_11rem]">
              <div className="space-y-1.5">
                <Label htmlFor="request-url">Target URL</Label>
                <Input
                  id="request-url"
                  value={url}
                  onChange={(event) => setUrl(event.target.value)}
                  spellCheck={false}
                  className="font-mono text-xs"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="request-action">Action</Label>
                <Select
                  id="request-action"
                  value={action}
                  onChange={(event) => setAction(event.target.value)}
                >
                  <option value="extract">extract</option>
                  <option value="crawl">crawl</option>
                  <option value="batch">batch</option>
                  <option value="map">map</option>
                </Select>
              </div>
            </div>
          </CardHeader>

          <CardContent className="flex flex-1 flex-col gap-3">
            <textarea
              value={text}
              onChange={(event) => setText(event.target.value)}
              spellCheck={false}
              aria-label="Request JSON"
              className="scrollbar-thin min-h-[22rem] flex-1 resize-y rounded-lg border border-border bg-code p-3 font-mono text-[12px] leading-relaxed text-foreground outline-none focus:ring-1 focus:ring-ring"
            />

            {parseError ? (
              <p className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-[11px] text-destructive">
                <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
                {parseError}
              </p>
            ) : null}

            <div className="flex flex-wrap items-center gap-2">
              <Button size="sm" onClick={() => run("run")} disabled={busy !== null || !parsed}>
                {busy === "run" ? <Loader2 className="animate-spin" /> : <Play />}
                Run request
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => run("validate")}
                disabled={busy !== null || !parsed}
              >
                {busy === "validate" ? <Loader2 className="animate-spin" /> : <ShieldCheck />}
                Validate
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={async () => {
                  if (await copyText(text)) toast.success("Request copied");
                }}
              >
                <Copy /> Copy
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setText(TEMPLATE);
                  setResponse(null);
                }}
              >
                <Trash2 /> Reset
              </Button>
            </div>

            <div className="flex flex-wrap items-end gap-2 border-t border-border/60 pt-3">
              <div className="flex-1 space-y-1.5">
                <Label htmlFor="template-name">Save as template</Label>
                <Input
                  id="template-name"
                  placeholder="e.g. product cards"
                  value={templateName}
                  onChange={(event) => setTemplateName(event.target.value)}
                />
              </div>
              <Button size="sm" variant="outline" onClick={saveTemplate}>
                <Save /> Save
              </Button>
            </div>

            {Object.keys(templates).length ? (
              <div className="flex flex-wrap gap-1.5">
                {Object.keys(templates).map((name) => (
                  <span
                    key={name}
                    className="inline-flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[11px]"
                  >
                    <button
                      className="hover:text-primary"
                      onClick={() => setText(templates[name])}
                    >
                      {name}
                    </button>
                    <button
                      className="text-muted-foreground hover:text-destructive"
                      onClick={() => removeTemplate(name)}
                      aria-label={`Delete template ${name}`}
                    >
                      ×
                    </button>
                  </span>
                ))}
              </div>
            ) : null}
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card>
            <CardHeader className="flex-row items-start justify-between gap-2">
              <div>
                <CardTitle>Response</CardTitle>
                <CardDescription>The shared envelope returned by CLI, API and MCP.</CardDescription>
              </div>
              {response ? (
                <Badge variant={response.success ? "success" : "destructive"}>
                  {response.success ? "success" : response.error?.code ?? "error"}
                </Badge>
              ) : null}
            </CardHeader>
            <CardContent className="space-y-3">
              {response ? (
                <>
                  <MetaSummary response={response} />
                  {response.error ? (
                    <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
                      <p className="font-medium">{response.error.code}</p>
                      <p className="mt-1">{response.error.message}</p>
                      {response.error.retryable ? (
                        <p className="mt-1 text-[11px]">Retryable: yes</p>
                      ) : null}
                    </div>
                  ) : null}
                  <Issues issues={response.errors ?? []} />
                  {response.meta?.warnings?.length ? (
                    <div className="space-y-1 rounded-md border border-warning/30 bg-warning/10 p-3 text-[11px] text-warning">
                      {response.meta.warnings.map((warning) => (
                        <p key={warning}>{warning}</p>
                      ))}
                    </div>
                  ) : null}
                  <JsonViewer
                    value={response.data ?? null}
                    filename={`aicrawl-${response.meta?.job_id ?? "data"}.json`}
                    maxHeight="22rem"
                  />
                  <div className="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={async () => {
                        if (await copyText(JSON.stringify(response, null, 2))) {
                          toast.success("Response copied");
                        }
                      }}
                    >
                      <Copy /> Copy JSON
                    </Button>
                    <Button size="sm" variant="outline" onClick={download}>
                      <Download /> Download
                    </Button>
                  </div>
                </>
              ) : (
                <div className="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border px-4 py-10 text-center">
                  <CheckCircle2 className="size-5 text-muted-foreground" />
                  <p className="text-sm font-medium">No response yet</p>
                  <p className="max-w-sm text-xs text-muted-foreground">
                    Validation needs no network access. Running a request fetches the page,
                    applies the rules and stores the job in SQLite.
                  </p>
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <History className="size-4" /> Request history
              </CardTitle>
              <CardDescription>Recent runs are kept in this browser only.</CardDescription>
            </CardHeader>
            <CardContent>
              {history.length ? (
                <div className="space-y-1.5">
                  {history.map((entry) => (
                    <button
                      key={`${entry.at}-${entry.jobId ?? "local"}`}
                      onClick={() => setText(entry.request)}
                      className="flex w-full items-center justify-between gap-2 rounded-md border border-border/70 px-3 py-2 text-left text-[11px] transition hover:bg-muted"
                    >
                      <span className="truncate font-mono">
                        {entry.jobId ?? entry.request.slice(0, 40)}
                      </span>
                      <span className="flex shrink-0 items-center gap-2 text-muted-foreground">
                        {typeof entry.items === "number" ? <span>{entry.items} items</span> : null}
                        <span>{new Date(entry.at).toLocaleTimeString()}</span>
                        <Badge variant={entry.success ? "success" : "destructive"}>
                          {entry.success ? "ok" : "failed"}
                        </Badge>
                      </span>
                    </button>
                  ))}
                </div>
              ) : (
                <p className="text-xs text-muted-foreground">Nothing yet.</p>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}

function MetaSummary({ response }: { response: ExtractionResponse }) {
  const meta = response.meta;
  if (!meta) {
    return (
      <p className="text-[11px] text-muted-foreground">
        Validation responses do not include page metadata.
      </p>
    );
  }
  return (
    <div className="flex flex-wrap gap-1.5 text-[11px]">
      <Badge variant="outline">{meta.pages_processed} page(s)</Badge>
      <Badge variant="outline">{meta.items_extracted} item(s)</Badge>
      <Badge variant="outline">{formatDuration(meta.duration_ms)}</Badge>
      <Badge variant="outline">{meta.render}</Badge>
      {meta.projection ? <Badge variant="outline">{meta.projection}</Badge> : null}
      {meta.cached ? <Badge variant="outline">cache</Badge> : null}
      {meta.truncated ? <Badge variant="warning">truncated</Badge> : null}
      {meta.job_id ? (
        <Badge variant="outline">
          <span className="font-mono">{meta.job_id}</span>
        </Badge>
      ) : null}
    </div>
  );
}

function Issues({ issues }: { issues: Issue[] }) {
  if (!issues.length) return null;
  return (
    <div className="overflow-hidden rounded-md border border-warning/30">
      <table className="w-full text-left text-[11px]">
        <thead className="bg-warning/10 text-warning">
          <tr>
            <th className="px-2 py-1 font-medium">Field</th>
            <th className="px-2 py-1 font-medium">Code</th>
            <th className="px-2 py-1 font-medium">Required</th>
          </tr>
        </thead>
        <tbody>
          {issues.slice(0, 12).map((issue, index) => (
            <tr key={`${issue.field}-${index}`} className="border-t border-border/70">
              <td className="px-2 py-1 font-mono">{issue.field}</td>
              <td className="px-2 py-1 text-muted-foreground">{issue.code}</td>
              <td className="px-2 py-1">{issue.required ? "yes" : "no"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
