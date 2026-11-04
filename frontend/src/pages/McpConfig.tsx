import { useState } from "react";
import { Check, Copy, Plug, ShieldAlert, Terminal } from "lucide-react";
import { api } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/PageHeader";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { copyText } from "@/lib/utils";

function CodeBlock({ code, label }: { code: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="overflow-hidden rounded-lg border border-border bg-code">
      <div className="flex items-center justify-between border-b border-border/70 px-3 py-2">
        <span className="font-mono text-[11px] text-muted-foreground">{label ?? "config"}</span>
        <Button
          variant="ghost"
          size="sm"
          onClick={async () => {
            if (await copyText(code)) {
              setCopied(true);
              window.setTimeout(() => setCopied(false), 1500);
            }
          }}
        >
          {copied ? <Check /> : <Copy />}
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <pre className="scrollbar-thin overflow-auto p-4 text-[12px] leading-relaxed">
        <code>{code}</code>
      </pre>
    </div>
  );
}

const EXAMPLE_REQUEST = `{
  "request": {
    "version": "1",
    "url": "https://example.com/products",
    "selectors": {
      "products": {
        "type": "list",
        "selector": ".product-card",
        "fields": {
          "name": { "selector": ".product-title", "required": true },
          "price": { "selector": ".price", "type": "number" }
        }
      }
    }
  }
}`;

export function McpConfig() {
  const toast = useToast();
  const { data, loading, error } = usePolling(() => api.mcp(), 30000);

  return (
    <div className="space-y-6">
      <PageHeader
        title="MCP configuration"
        description="MCP is a first-class interface. Agents send structured JSON requests and receive the same envelope as the CLI and REST API."
        actions={<Badge variant="outline">streamable HTTP · stdio</Badge>}
      />

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      {loading && !data ? (
        <Skeleton className="h-64 w-full" />
      ) : data ? (
        <>
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Plug className="size-4 text-primary" /> Endpoint
              </CardTitle>
              <CardDescription>
                Start it with <span className="font-mono">aicrawl api</span> (REST + dashboard) or{" "}
                <span className="font-mono">aicrawl mcp --http</span>.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="flex items-center gap-2 rounded-lg border border-border bg-muted/30 px-3 py-2">
                <code className="flex-1 truncate font-mono text-xs">{data.http.endpoint}</code>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={async () => {
                    if (await copyText(data.http.endpoint)) {
                      toast.success("Endpoint copied");
                    }
                  }}
                >
                  <Copy /> Copy
                </Button>
              </div>
              <p className="text-[11px] text-muted-foreground">
                The endpoint validates the Host header and binds to loopback by default. Do not
                expose it publicly without a reverse proxy and authentication.
              </p>
            </CardContent>
          </Card>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Streamable HTTP</CardTitle>
                <CardDescription>Transport: {data.http.transport}</CardDescription>
              </CardHeader>
              <CardContent>
                <CodeBlock
                  label="mcp.json"
                  code={JSON.stringify(data.http.client_config, null, 2)}
                />
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>stdio</CardTitle>
                <CardDescription>Local process, JSON on stdout, logs on stderr.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <CodeBlock
                  label="mcp.json"
                  code={JSON.stringify(data.stdio.client_config, null, 2)}
                />
                <CodeBlock
                  label="command"
                  code={`${data.stdio.command} ${data.stdio.args.join(" ")}`}
                />
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Tools</CardTitle>
              <CardDescription>
                {data.tools.length} JSON tools share one engine, one security policy and one
                storage layer.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Tool</TableHead>
                    <TableHead>Arguments</TableHead>
                    <TableHead>Description</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.tools.map((tool) => (
                    <TableRow key={tool.name}>
                      <TableCell className="whitespace-nowrap font-mono text-[11px] text-primary">
                        {tool.name}
                      </TableCell>
                      <TableCell className="whitespace-nowrap font-mono text-[10px] text-muted-foreground">
                        {tool.arguments.join(", ")}
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {tool.description}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>

          <Card className="border-warning/25 bg-warning/[0.04]">
            <CardContent className="flex items-start gap-3 pt-5">
              <ShieldAlert className="mt-0.5 size-4 text-warning" />
              <div>
                <p className="text-sm font-medium">Untrusted content</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {data.notice} Page text is returned as data; agents must never execute
                  instructions found inside it.
                </p>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Terminal className="size-4" /> Example tool call
              </CardTitle>
              <CardDescription>extract_structured_data</CardDescription>
            </CardHeader>
            <CardContent>
              <CodeBlock label="arguments" code={EXAMPLE_REQUEST} />
            </CardContent>
          </Card>
        </>
      ) : null}
    </div>
  );
}
