import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Clock,
  FileJson,
  Loader2,
  Play,
  Sparkles,
  Wand2,
} from "lucide-react";
import { ApiError, api, type ExtractResponse } from "@/lib/api";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/PageHeader";
import { JsonViewer } from "@/components/JsonViewer";
import { StatusBadge, ModeBadge } from "@/components/StatusBadge";
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
import { Textarea } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn, formatDuration } from "@/lib/utils";

const EXAMPLES = [
  {
    label: "Products",
    url: "https://example.com/products",
    query: "Extract product names, prices and descriptions",
    schema: `{
  "type": "object",
  "properties": {
    "products": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string" },
          "price": { "type": "string" },
          "description": { "type": "string" }
        },
        "required": ["name"]
      }
    }
  },
  "required": ["products"]
}`,
  },
  {
    label: "Article",
    url: "https://example.com/blog/post",
    query: "Extract the article title, author, publish date and a summary",
    schema: "",
  },
  {
    label: "Pricing",
    url: "https://example.com/pricing",
    query: "Extract plan names, monthly prices and included features",
    schema: "",
  },
];

type Phase = "idle" | "running" | "done" | "failed";

export function NewExtraction() {
  const toast = useToast();
  const [url, setUrl] = useState("https://example.com");
  const [query, setQuery] = useState(
    "Extract product names, prices and descriptions",
  );
  const [schemaText, setSchemaText] = useState("");
  const [format, setFormat] = useState("json");
  const [useAi, setUseAi] = useState(true);
  const [render, setRender] = useState(false);
  const [useCache, setUseCache] = useState(true);
  const [phase, setPhase] = useState<Phase>("idle");
  const [elapsed, setElapsed] = useState(0);
  const [result, setResult] = useState<ExtractResponse | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const startedAt = useRef<number | null>(null);

  useEffect(() => {
    if (phase !== "running") return;
    const timer = window.setInterval(() => {
      if (startedAt.current) {
        setElapsed(Date.now() - startedAt.current);
      }
    }, 100);
    return () => window.clearInterval(timer);
  }, [phase]);

  const schemaError = useMemo(() => {
    if (!schemaText.trim()) return null;
    try {
      const parsed = JSON.parse(schemaText);
      if (typeof parsed !== "object" || Array.isArray(parsed)) {
        return "Schema must be a JSON object";
      }
      return null;
    } catch (error) {
      return error instanceof Error ? error.message : "Invalid JSON";
    }
  }, [schemaText]);

  const run = async () => {
    if (!url.trim()) {
      toast.error("URL required", "Enter the page you want to extract from.");
      return;
    }
    if (schemaError) {
      toast.error("Schema is not valid JSON", schemaError);
      return;
    }
    setPhase("running");
    setFailure(null);
    setResult(null);
    setElapsed(0);
    startedAt.current = Date.now();

    try {
      const response = await api.extract({
        url: url.trim(),
        query: query.trim() || "Extract the main content of this page",
        schema: schemaText.trim() ? JSON.parse(schemaText) : undefined,
        output_format: format,
        use_ai: useAi,
        render,
        use_cache: useCache,
      });
      setResult(response);
      setPhase("done");
      toast.success(
        "Extraction complete",
        `${response.outcome.mode} · ${formatDuration(response.outcome.duration_ms)}`,
      );
    } catch (error) {
      const message =
        error instanceof ApiError ? error.message : String(error);
      setFailure(message);
      setPhase("failed");
      toast.error("Extraction failed", message);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title="New extraction"
        description="Describe what you need in natural language. The Rust engine fetches, parses and validates; an external AI provider is used only when configured."
        actions={
          <Badge variant="outline" className="font-mono">
            POST /api/v1/extract
          </Badge>
        }
      />

      <div className="grid gap-4 xl:grid-cols-5">
        <div className="space-y-4 xl:col-span-3">
          <Card>
            <CardHeader className="flex-row items-center justify-between">
              <div>
                <CardTitle>Request</CardTitle>
                <CardDescription>
                  Website URL and the query to answer.
                </CardDescription>
              </div>
              <Wand2 className="size-4 text-primary" />
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-1.5">
                <Label htmlFor="url">Website URL</Label>
                <Input
                  id="url"
                  placeholder="https://example.com/products"
                  value={url}
                  onChange={(event) => setUrl(event.target.value)}
                  spellCheck={false}
                  inputMode="url"
                />
              </div>

              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <Label htmlFor="query">Extraction query</Label>
                  <span className="text-[10px] text-muted-foreground">
                    natural language
                  </span>
                </div>
                <Textarea
                  id="query"
                  rows={3}
                  placeholder="Extract product names, prices and descriptions"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                />
                <div className="flex flex-wrap gap-1.5 pt-1">
                  <span className="text-[10px] text-muted-foreground">
                    Examples:
                  </span>
                  {EXAMPLES.map((example) => (
                    <button
                      key={example.label}
                      onClick={() => {
                        setUrl(example.url);
                        setQuery(example.query);
                        setSchemaText(example.schema);
                      }}
                      className="rounded-full border border-border px-2 py-0.5 text-[10px] text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
                    >
                      {example.label}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid gap-3 sm:grid-cols-3">
                <div className="space-y-1.5">
                  <Label htmlFor="format">Output format</Label>
                  <Select
                    id="format"
                    value={format}
                    onChange={(event) => setFormat(event.target.value)}
                  >
                    <option value="json">JSON</option>
                    <option value="markdown">Markdown</option>
                    <option value="text">Text</option>
                  </Select>
                </div>
                <Toggle
                  label="Use external AI"
                  hint="Semantic extraction"
                  checked={useAi}
                  onChange={setUseAi}
                />
                <Toggle
                  label="Render JavaScript"
                  hint="Playwright (optional)"
                  checked={render}
                  onChange={setRender}
                />
              </div>

              <div className="flex items-center justify-between rounded-md border border-border px-3 py-2">
                <div>
                  <p className="text-xs font-medium">Cache</p>
                  <p className="text-[10px] text-muted-foreground">
                    Reuse recent responses from SQLite (TTL configured server-side)
                  </p>
                </div>
                <Switch
                  checked={useCache}
                  onCheckedChange={setUseCache}
                  label="Use page cache"
                />
              </div>

              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <Label htmlFor="schema">JSON Schema (optional)</Label>
                  {schemaError ? (
                    <span className="text-[10px] text-destructive">
                      {schemaError}
                    </span>
                  ) : (
                    <span className="text-[10px] text-muted-foreground">
                      validated server-side
                    </span>
                  )}
                </div>
                <Textarea
                  id="schema"
                  rows={8}
                  placeholder={'{\n  "type": "object",\n  "properties": {}\n}'}
                  value={schemaText}
                  onChange={(event) => setSchemaText(event.target.value)}
                  className={cn(
                    "font-mono text-[11px]",
                    schemaError && "border-destructive/50",
                  )}
                  spellCheck={false}
                />
              </div>

              <Button
                className="w-full"
                size="lg"
                onClick={run}
                disabled={phase === "running" || Boolean(schemaError)}
              >
                {phase === "running" ? (
                  <>
                    <Loader2 className="animate-spin" /> Extracting…
                  </>
                ) : (
                  <>
                    <Play /> Start extraction
                  </>
                )}
              </Button>
            </CardContent>
          </Card>
        </div>

        <div className="space-y-4 xl:col-span-2">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Clock className="size-4 text-primary" /> Live job status
              </CardTitle>
              <CardDescription>
                The same job record is visible to the CLI and MCP clients.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <Timeline phase={phase} elapsed={elapsed} />
              {result ? (
                <div className="space-y-2 rounded-lg border border-border bg-muted/30 p-3 text-xs">
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">job</span>
                    <StatusBadge status={result.job.status} />
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">mode</span>
                    <ModeBadge mode={result.outcome.mode} />
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">job id</span>
                    <span className="font-mono text-[10px]">
                      {result.job.id}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">fetch</span>
                    <span>{formatDuration(result.outcome.fetch_ms)}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">total</span>
                    <span>{formatDuration(result.outcome.duration_ms)}</span>
                  </div>
                </div>
              ) : null}

              {result?.outcome.warnings.length ? (
                <div className="space-y-1.5 rounded-lg border border-warning/30 bg-warning/10 p-3">
                  {result.outcome.warnings.map((warning) => (
                    <p
                      key={warning}
                      className="flex items-start gap-2 text-[11px] text-warning"
                    >
                      <AlertTriangle className="mt-0.5 size-3 shrink-0" />
                      {warning}
                    </p>
                  ))}
                </div>
              ) : null}

              {failure ? (
                <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-[11px] text-destructive">
                  {failure}
                </div>
              ) : null}

              {!result && !failure ? (
                <p className="text-[11px] leading-relaxed text-muted-foreground">
                  Requests are validated against the SSRF policy, robots.txt and
                  crawl limits before anything is fetched.
                </p>
              ) : null}
            </CardContent>
          </Card>

          {result ? (
            <Card>
              <CardHeader>
                <CardTitle>Page</CardTitle>
              </CardHeader>
              <CardContent className="space-y-1.5 text-xs">
                <div className="flex justify-between gap-3">
                  <span className="text-muted-foreground">title</span>
                  <span className="truncate text-right">
                    {result.outcome.page.title ?? "—"}
                  </span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-muted-foreground">status</span>
                  <span className="font-mono">
                    {result.outcome.page.status}
                  </span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-muted-foreground">words</span>
                  <span>{result.outcome.page.word_count}</span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-muted-foreground">cache</span>
                  <span>{result.outcome.page.from_cache ? "hit" : "miss"}</span>
                </div>
                {result.result.provider ? (
                  <div className="flex justify-between gap-3">
                    <span className="text-muted-foreground">provider</span>
                    <span className="truncate text-right font-mono text-[10px]">
                      {result.result.provider}
                    </span>
                  </div>
                ) : null}
              </CardContent>
            </Card>
          ) : null}
        </div>
      </div>

      {result ? (
        <Card>
          <CardHeader className="flex-row items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2">
                <FileJson className="size-4 text-primary" /> Output
              </CardTitle>
              <CardDescription>
                {result.result.validated
                  ? "Payload validated against your JSON Schema."
                  : "No schema requested; payload returned as extracted."}
              </CardDescription>
            </div>
            {result.result.validated ? (
              <Badge variant="success">
                <CheckCircle2 className="size-3" /> validated
              </Badge>
            ) : null}
          </CardHeader>
          <CardContent>
            <Tabs defaultValue="data">
              <TabsList>
                <TabsTrigger value="data">Structured data</TabsTrigger>
                <TabsTrigger value="rendered">Rendered output</TabsTrigger>
                <TabsTrigger value="raw">Full response</TabsTrigger>
              </TabsList>
              <TabsContent value="data">
                <JsonViewer value={result.outcome.data} />
              </TabsContent>
              <TabsContent value="rendered">
                <pre className="scrollbar-thin max-h-[28rem] overflow-auto rounded-lg border border-border bg-code p-4 text-[12px] leading-relaxed whitespace-pre-wrap">
                  {result.outcome.output}
                </pre>
              </TabsContent>
              <TabsContent value="raw">
                <JsonViewer value={result} filename="aicrawl-extraction.json" />
              </TabsContent>
            </Tabs>
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between rounded-md border border-border px-3 py-2">
      <div>
        <p className="text-xs font-medium">{label}</p>
        <p className="text-[10px] text-muted-foreground">{hint}</p>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} label={label} />
    </div>
  );
}

function Timeline({ phase, elapsed }: { phase: Phase; elapsed: number }) {
  const steps = [
    { key: "fetch", label: "Fetch with SSRF + robots checks" },
    { key: "parse", label: "Parse HTML / readable content" },
    { key: "extract", label: "Extract and validate JSON" },
  ];
  return (
    <div className="space-y-2">
      {steps.map((step, index) => {
        const active =
          phase === "running" &&
          ((index === 0 && elapsed < 700) ||
            (index === 1 && elapsed >= 700 && elapsed < 1400) ||
            (index === 2 && elapsed >= 1400));
        const done =
          phase === "done" ||
          (phase === "running" && !active && elapsed > (index + 1) * 700);
        const failed = phase === "failed" && index === steps.length - 1;
        return (
          <div key={step.key} className="flex items-center gap-2 text-xs">
            {failed ? (
              <AlertTriangle className="size-3.5 text-destructive" />
            ) : done ? (
              <CheckCircle2 className="size-3.5 text-success" />
            ) : active ? (
              <Loader2 className="size-3.5 animate-spin text-primary" />
            ) : (
              <span className="size-3.5 rounded-full border border-border" />
            )}
            <span
              className={cn(
                "text-muted-foreground",
                (active || done) && "text-foreground",
                failed && "text-destructive",
              )}
            >
              {step.label}
            </span>
          </div>
        );
      })}
      <p className="pt-1 text-[10px] text-muted-foreground">
        {phase === "running"
          ? `${(elapsed / 1000).toFixed(1)}s elapsed`
          : phase === "done"
            ? "Completed"
            : phase === "failed"
              ? "Failed"
              : "Ready"}
      </p>
    </div>
  );
}
