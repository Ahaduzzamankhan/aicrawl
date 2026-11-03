import { useEffect, useState } from "react";
import {
  BrainCircuit,
  CheckCircle2,
  KeyRound,
  Loader2,
  Pencil,
  Plug,
  Plus,
  Star,
  Trash2,
  Zap,
} from "lucide-react";
import {
  ApiError,
  api,
  type Provider,
  type ProviderPayload,
} from "@/lib/api";
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
import { EmptyState, Skeleton } from "@/components/ui/skeleton";
import { formatDate } from "@/lib/utils";

const EMPTY_FORM: ProviderPayload = {
  name: "",
  kind: "openai_compatible",
  base_url: "https://api.openai.com/v1",
  model: "gpt-4o-mini",
  api_key: "",
  is_default: false,
  max_tokens: 1024,
  temperature: 0,
};

export function Providers() {
  const toast = useToast();
  const { data, loading, error, refresh } = usePolling(
    () => api.providers(),
    12000,
  );
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<Provider | null>(null);
  const [form, setForm] = useState<ProviderPayload>(EMPTY_FORM);
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);

  useEffect(() => {
    if (!dialogOpen) return;
    if (editing) {
      setForm({
        name: editing.name,
        kind: editing.kind,
        base_url: editing.base_url,
        model: editing.model,
        api_key: "",
        is_default: editing.is_default,
        max_tokens: editing.max_tokens,
        temperature: editing.temperature,
      });
    } else {
      setForm({ ...EMPTY_FORM, is_default: (data?.providers.length ?? 0) === 0 });
    }
  }, [dialogOpen, editing, data?.providers.length]);

  const submit = async () => {
    if (!form.name.trim() || !form.model.trim() || !form.base_url.trim()) {
      toast.error("Missing fields", "Name, base URL and model are required.");
      return;
    }
    if (form.kind !== "ollama" && !editing && !form.api_key?.trim()) {
      toast.error(
        "API key required",
        "Hosted providers need a key. Keys are encrypted before storage.",
      );
      return;
    }
    setBusy(true);
    try {
      if (editing) {
        await api.updateProvider(editing.id, form);
        toast.success("Provider updated", form.name);
      } else {
        await api.createProvider(form);
        toast.success("Provider added", form.name);
      }
      setDialogOpen(false);
      setEditing(null);
      refresh();
    } catch (cause) {
      toast.error(
        "Could not save provider",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    } finally {
      setBusy(false);
    }
  };

  const test = async (provider: Provider) => {
    setTesting(provider.id);
    try {
      const result = await api.testProvider(provider.id);
      toast.success(
        `Provider replied in ${result.latency_ms}ms`,
        JSON.stringify(result.reply).slice(0, 120),
      );
    } catch (cause) {
      toast.error(
        "Provider test failed",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    } finally {
      setTesting(null);
    }
  };

  const makeDefault = async (provider: Provider) => {
    try {
      await api.setDefaultProvider(provider.id);
      toast.success("Default provider updated", provider.name);
      refresh();
    } catch (cause) {
      toast.error(
        "Could not update default",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    }
  };

  const remove = async (provider: Provider) => {
    if (!window.confirm(`Delete provider "${provider.name}"?`)) return;
    try {
      await api.deleteProvider(provider.id);
      toast.success("Provider deleted");
      refresh();
    } catch (cause) {
      toast.error(
        "Could not delete provider",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    }
  };

  const providers = data?.providers ?? [];

  return (
    <div className="space-y-6">
      <PageHeader
        title="AI providers"
        description="AICrawl ships no model and no credentials. Add an external OpenAI-compatible endpoint or a local Ollama server for semantic extraction."
        actions={
          <Button
            size="sm"
            onClick={() => {
              setEditing(null);
              setDialogOpen(true);
            }}
          >
            <Plus /> Add provider
          </Button>
        }
      />

      <Card
        className={
          data?.ai.configured
            ? "border-success/25 bg-success/[0.04]"
            : "border-warning/25 bg-warning/[0.04]"
        }
      >
        <CardContent className="flex items-start gap-3 pt-5">
          <BrainCircuit
            className={
              data?.ai.configured
                ? "mt-0.5 size-4 text-success"
                : "mt-0.5 size-4 text-warning"
            }
          />
          <div>
            <p className="text-sm font-medium">
              {data?.ai.configured
                ? "Semantic extraction enabled"
                : "Deterministic extraction only"}
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              {data?.ai.detail ??
                "Add a provider to use natural-language extraction with a model."}
            </p>
            {data?.ai.configured ? (
              <div className="mt-2 flex flex-wrap gap-1.5">
                <Badge variant="outline">{data.ai.kind}</Badge>
                <Badge variant="outline">{data.ai.model}</Badge>
                <Badge variant="outline">source: {data.ai.source}</Badge>
              </div>
            ) : null}
          </div>
        </CardContent>
      </Card>

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      {loading && !data ? (
        <Skeleton className="h-40 w-full" />
      ) : providers.length ? (
        <div className="grid gap-3 lg:grid-cols-2">
          {providers.map((provider) => (
            <Card key={provider.id} className="p-4">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <p className="truncate text-sm font-semibold">
                      {provider.name}
                    </p>
                    {provider.is_default ? (
                      <Badge variant="success">
                        <Star className="size-3" /> default
                      </Badge>
                    ) : null}
                  </div>
                  <p className="mt-0.5 truncate font-mono text-[11px] text-muted-foreground">
                    {provider.base_url}
                  </p>
                </div>
                <Badge variant="outline">{provider.kind}</Badge>
              </div>

              <div className="mt-3 grid grid-cols-2 gap-2 text-[11px]">
                <Info label="model" value={provider.model} mono />
                <Info
                  label="max tokens"
                  value={String(provider.max_tokens)}
                />
                <Info
                  label="api key"
                  value={provider.has_api_key ? "stored (encrypted)" : "none"}
                />
                <Info
                  label="updated"
                  value={formatDate(provider.updated_at)}
                />
              </div>

              <div className="mt-3 flex flex-wrap items-center gap-1">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => test(provider)}
                  disabled={testing === provider.id}
                >
                  {testing === provider.id ? (
                    <Loader2 className="animate-spin" />
                  ) : (
                    <Zap />
                  )}
                  Test
                </Button>
                {!provider.is_default ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => makeDefault(provider)}
                  >
                    <CheckCircle2 /> Make default
                  </Button>
                ) : null}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setEditing(provider);
                    setDialogOpen(true);
                  }}
                >
                  <Pencil /> Edit
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="ml-auto text-destructive hover:text-destructive"
                  onClick={() => remove(provider)}
                >
                  <Trash2 /> Delete
                </Button>
              </div>
            </Card>
          ))}
        </div>
      ) : (
        <EmptyState
          icon={<Plug className="size-5" />}
          title="No AI providers configured"
          description="Extraction already works deterministically. Add a provider to answer semantic queries and honour custom schemas reliably."
          action={
            <Button size="sm" onClick={() => setDialogOpen(true)}>
              <Plus /> Add your first provider
            </Button>
          }
        />
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <KeyRound className="size-4 text-primary" /> Key handling
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-2 text-xs text-muted-foreground">
          <p>
            Provider keys are encrypted with ChaCha20-Poly1305 using a key stored
            in the data directory (or <code>AICRAWL_SECRET_KEY</code>) and are
            never written to the config file or logs.
          </p>
          <p>
            Ollama does not require a key. For hosted APIs, use a dedicated key
            with the smallest possible scope.
          </p>
        </CardContent>
      </Card>

      <Dialog
        open={dialogOpen}
        onOpenChange={(open) => {
          setDialogOpen(open);
          if (!open) setEditing(null);
        }}
      >
        <DialogHeader onClose={() => setDialogOpen(false)}>
          <DialogTitle>
            {editing ? "Edit provider" : "Add AI provider"}
          </DialogTitle>
          <DialogDescription>
            Any endpoint that implements the OpenAI chat completions API works,
            as does Ollama.
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="p-name">Name</Label>
              <Input
                id="p-name"
                value={form.name}
                onChange={(event) =>
                  setForm((current) => ({ ...current, name: event.target.value }))
                }
                placeholder="OpenAI production"
                autoFocus
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="p-kind">Kind</Label>
              <Select
                id="p-kind"
                value={form.kind}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    kind: event.target.value,
                    base_url:
                      event.target.value === "ollama"
                        ? "http://127.0.0.1:11434"
                        : current.base_url,
                    model:
                      event.target.value === "ollama"
                        ? "llama3.1"
                        : current.model,
                  }))
                }
              >
                <option value="openai_compatible">OpenAI-compatible</option>
                <option value="openai">OpenAI</option>
                <option value="ollama">Ollama (local)</option>
              </Select>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="p-base">Base URL</Label>
            <Input
              id="p-base"
              value={form.base_url}
              onChange={(event) =>
                setForm((current) => ({
                  ...current,
                  base_url: event.target.value,
                }))
              }
              spellCheck={false}
              placeholder="https://api.openai.com/v1"
            />
            <p className="text-[10px] text-muted-foreground">
              <code>/chat/completions</code> is appended automatically unless the
              URL already includes it.
            </p>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="p-model">Model</Label>
              <Input
                id="p-model"
                value={form.model}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    model: event.target.value,
                  }))
                }
                spellCheck={false}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="p-key">
                API key {form.kind === "ollama" ? "(not required)" : ""}
              </Label>
              <Input
                id="p-key"
                type="password"
                autoComplete="off"
                value={form.api_key ?? ""}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    api_key: event.target.value,
                  }))
                }
                placeholder={
                  editing?.has_api_key ? "leave blank to keep existing" : "sk-…"
                }
                disabled={form.kind === "ollama"}
              />
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="p-tokens">Max output tokens</Label>
              <Input
                id="p-tokens"
                type="number"
                min={64}
                max={8192}
                value={form.max_tokens ?? 1024}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    max_tokens: Number(event.target.value),
                  }))
                }
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="p-temp">Temperature</Label>
              <Input
                id="p-temp"
                type="number"
                min={0}
                max={2}
                step={0.1}
                value={form.temperature ?? 0}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    temperature: Number(event.target.value),
                  }))
                }
              />
            </div>
          </div>

          <div className="flex items-center justify-between rounded-md border border-border px-3 py-2">
            <div>
              <p className="text-xs font-medium">Use as default</p>
              <p className="text-[10px] text-muted-foreground">
                Only one provider is used per extraction
              </p>
            </div>
            <Switch
              checked={Boolean(form.is_default)}
              onCheckedChange={(checked) =>
                setForm((current) => ({ ...current, is_default: checked }))
              }
              label="Use as default"
            />
          </div>

          <Button className="w-full" onClick={submit} disabled={busy}>
            {busy ? <Loader2 className="animate-spin" /> : <Plug />}
            {editing ? "Save changes" : "Add provider"}
          </Button>
        </DialogBody>
      </Dialog>
    </div>
  );
}

function Info({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="rounded-md border border-border/70 px-2 py-1.5">
      <p className="text-[9px] uppercase tracking-wide text-muted-foreground">
        {label}
      </p>
      <p className={mono ? "truncate font-mono text-[10px]" : "truncate"}>
        {value}
      </p>
    </div>
  );
}
