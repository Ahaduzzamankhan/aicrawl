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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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
        <span className="font-mono text-[11px] text-muted-foreground">
          {label ?? "config"}
        </span>
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

export function McpConfig() {
  const toast = useToast();
  const { data, loading, error } = usePolling(() => api.mcp(), 30000);

  return (
    <div className="space-y-6">
      <PageHeader
        title="MCP configuration"
        description="Connect an AI agent to AICrawl. MCP is a first-class interface: the dashboard is never required for agent workflows."
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
                Serve it with <span className="font-mono">aicrawl start</span> or{" "}
                <span className="font-mono">aicrawl mcp start</span>.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="flex items-center gap-2 rounded-lg border border-border bg-muted/30 px-3 py-2">
                <code className="flex-1 truncate font-mono text-xs">
                  {data.http.endpoint}
                </code>
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
                Authentication is handled by your local network policy. The MCP
                endpoint validates the Host header and is intended for local
                agents; do not expose it publicly without a proxy.
              </p>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Client setup</CardTitle>
              <CardDescription>
                Paste one of these snippets into your MCP client configuration.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Tabs defaultValue="http">
                <TabsList>
                  <TabsTrigger value="http">Streamable HTTP</TabsTrigger>
                  <TabsTrigger value="stdio">stdio</TabsTrigger>
                </TabsList>
                <TabsContent value="http" className="space-y-3">
                  <CodeBlock
                    label="mcp.json"
                    code={JSON.stringify(data.http.client_config, null, 2)}
                  />
                  <p className="text-[11px] text-muted-foreground">
                    Transport: {data.http.transport}. The server supports JSON
                    responses for simple tool calls.
                  </p>
                </TabsContent>
                <TabsContent value="stdio" className="space-y-3">
                  <CodeBlock
                    label="local stdio server"
                    code={JSON.stringify(data.stdio.client_config, null, 2)}
                  />
                  <CodeBlock
                    label="command"
                    code={`${data.stdio.command} ${data.stdio.args.join(" ")}`}
                  />
                  <p className="text-[11px] text-muted-foreground">
                    stdio writes logs to stderr only, keeping stdout as a clean
                    protocol channel.
                  </p>
                </TabsContent>
              </Tabs>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Tools</CardTitle>
              <CardDescription>
                Seven tools share the same engine as the CLI and REST API.
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
                  {data.notice} Prompt-injection attempts are flagged as warnings
                  and page text is wrapped in an explicit data-only block before
                  it reaches an external model.
                </p>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Terminal className="size-4" /> Example agent call
              </CardTitle>
            </CardHeader>
            <CardContent>
              <CodeBlock
                label="extract_website_data"
                code={`{
  "tool": "extract_website_data",
  "arguments": {
    "url": "https://example.com/products",
    "query": "Extract product names and prices",
    "output_format": "json"
  }
}`}
              />
            </CardContent>
          </Card>
        </>
      ) : null}
    </div>
  );
}
