import { useMemo, useState, type ReactNode } from "react";
import { Check, Copy, Download } from "lucide-react";
import { Button } from "@/components/ui/button";
import { copyText, downloadJson } from "@/lib/utils";

const TOKEN =
  /("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)/g;

function highlight(json: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let lastIndex = 0;
  let index = 0;
  for (const match of json.matchAll(TOKEN)) {
    const start = match.index ?? 0;
    if (start > lastIndex) {
      nodes.push(json.slice(lastIndex, start));
    }
    const [full, stringToken, colon, literal, number] = match;
    if (stringToken) {
      nodes.push(
        <span
          key={`k${index++}`}
          className={colon ? "text-sky-400" : "text-emerald-400"}
        >
          {stringToken}
        </span>,
      );
      if (colon) nodes.push(colon);
    } else if (literal) {
      nodes.push(
        <span key={`k${index++}`} className="text-fuchsia-400">
          {literal}
        </span>,
      );
    } else if (number) {
      nodes.push(
        <span key={`k${index++}`} className="text-amber-400">
          {number}
        </span>,
      );
    } else {
      nodes.push(full);
    }
    lastIndex = start + full.length;
  }
  if (lastIndex < json.length) nodes.push(json.slice(lastIndex));
  return nodes;
}

interface JsonViewerProps {
  value: unknown;
  filename?: string;
  maxHeight?: string;
  toolbar?: ReactNode;
}

export function JsonViewer({
  value,
  filename = "aicrawl-result.json",
  maxHeight = "28rem",
  toolbar,
}: JsonViewerProps) {
  const [copied, setCopied] = useState(false);
  const text = useMemo(() => JSON.stringify(value ?? null, null, 2), [value]);
  const rendered = useMemo(() => highlight(text), [text]);

  const copy = async () => {
    if (await copyText(text)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    }
  };

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-code">
      <div className="flex items-center justify-between gap-2 border-b border-border/70 px-3 py-2">
        <span className="font-mono text-[11px] text-muted-foreground">
          JSON · {new Blob([text]).size} bytes
        </span>
        <div className="flex items-center gap-1">
          {toolbar}
          <Button variant="ghost" size="sm" onClick={copy}>
            {copied ? <Check /> : <Copy />}
            {copied ? "Copied" : "Copy"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => downloadJson(filename, value)}
          >
            <Download />
            Export
          </Button>
        </div>
      </div>
      <pre
        className="scrollbar-thin overflow-auto p-4 text-[12px] leading-relaxed text-foreground/90"
        style={{ maxHeight }}
      >
        <code>{rendered}</code>
      </pre>
    </div>
  );
}
