import { useState } from "react";
import { Check, Copy, KeyRound, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { ApiError, api } from "@/lib/api";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, Skeleton } from "@/components/ui/skeleton";
import { copyText, formatDate } from "@/lib/utils";

export function ApiKeys() {
  const toast = useToast();
  const { data, loading, error, refresh } = usePolling(() => api.apiKeys(), 15000);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("dashboard");
  const [busy, setBusy] = useState(false);
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [dashboardKey, setDashboardKey] = useState(
    () => localStorage.getItem("aicrawl.apiKey") ?? "",
  );

  const create = async () => {
    setBusy(true);
    try {
      const response = await api.createApiKey(name.trim() || "dashboard");
      setCreatedKey(response.key);
      setCreating(false);
      toast.success("API key created", "Copy it now; it is shown only once.");
      refresh();
    } catch (cause) {
      toast.error(
        "Could not create key",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    } finally {
      setBusy(false);
    }
  };

  const revoke = async (id: string) => {
    if (!window.confirm("Revoke this API key? Requests using it will fail.")) {
      return;
    }
    try {
      await api.revokeApiKey(id);
      toast.success("Key revoked");
      refresh();
    } catch (cause) {
      toast.error(
        "Could not revoke key",
        cause instanceof ApiError ? cause.message : String(cause),
      );
    }
  };

  const saveDashboardKey = () => {
    if (dashboardKey.trim()) {
      localStorage.setItem("aicrawl.apiKey", dashboardKey.trim());
      toast.success("Dashboard API key saved locally");
    } else {
      localStorage.removeItem("aicrawl.apiKey");
      toast.info("Dashboard API key cleared");
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title="API keys"
        description="Keys authenticate REST API calls when api.require_api_key is enabled. Only hashes are stored; the plaintext key is shown once."
        actions={
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus /> New key
          </Button>
        }
      />

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <KeyRound className="size-4 text-primary" /> Stored keys
          </CardTitle>
          <CardDescription>
            Revoked keys remain visible for audit purposes.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {loading && !data ? (
            <Skeleton className="h-24 w-full" />
          ) : data?.api_keys.length ? (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Prefix</TableHead>
                  <TableHead>Scopes</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead>Last used</TableHead>
                  <TableHead>State</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.api_keys.map((key) => (
                  <TableRow key={key.id}>
                    <TableCell className="text-xs font-medium">
                      {key.name}
                    </TableCell>
                    <TableCell className="font-mono text-[11px]">
                      {key.prefix}
                    </TableCell>
                    <TableCell className="text-[11px] text-muted-foreground">
                      {key.scopes}
                    </TableCell>
                    <TableCell className="text-[11px] text-muted-foreground">
                      {formatDate(key.created_at)}
                    </TableCell>
                    <TableCell className="text-[11px] text-muted-foreground">
                      {formatDate(key.last_used_at)}
                    </TableCell>
                    <TableCell>
                      {key.revoked_at ? (
                        <Badge variant="destructive">revoked</Badge>
                      ) : (
                        <Badge variant="success">active</Badge>
                      )}
                    </TableCell>
                    <TableCell className="text-right">
                      {key.revoked_at ? null : (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => revoke(key.id)}
                        >
                          <Trash2 /> Revoke
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          ) : (
            <EmptyState
              icon={<KeyRound className="size-5" />}
              title="No API keys"
              description="Create a key if you bind the API beyond loopback or enable api.require_api_key."
              action={
                <Button size="sm" onClick={() => setCreating(true)}>
                  <Plus /> Create key
                </Button>
              }
            />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck className="size-4 text-success" /> This dashboard
          </CardTitle>
          <CardDescription>
            If API-key auth is enforced, save a key in this browser. It stays in
            localStorage and is sent as a Bearer token.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-2 sm:flex-row">
          <Input
            type="password"
            placeholder="ac_live_…"
            value={dashboardKey}
            onChange={(event) => setDashboardKey(event.target.value)}
            autoComplete="off"
          />
          <Button variant="outline" onClick={saveDashboardKey}>
            Save locally
          </Button>
        </CardContent>
      </Card>

      <Dialog open={creating} onOpenChange={setCreating}>
        <DialogHeader onClose={() => setCreating(false)}>
          <DialogTitle>Create API key</DialogTitle>
          <DialogDescription>
            Name it after the integration that will use it.
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="key-name">Name</Label>
            <Input
              id="key-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              autoFocus
            />
          </div>
          <Button className="w-full" onClick={create} disabled={busy}>
            Create key
          </Button>
        </DialogBody>
      </Dialog>

      <Dialog
        open={Boolean(createdKey)}
        onOpenChange={(open) => {
          if (!open) {
            setCreatedKey(null);
            setCopied(false);
          }
        }}
      >
        <DialogHeader
          onClose={() => {
            setCreatedKey(null);
            setCopied(false);
          }}
        >
          <DialogTitle>Copy your key now</DialogTitle>
          <DialogDescription>
            It cannot be retrieved again. Store it in a secret manager.
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="space-y-4">
          <div className="flex items-center gap-2 rounded-lg border border-border bg-code px-3 py-2">
            <code className="flex-1 break-all font-mono text-xs">
              {createdKey}
            </code>
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                if (createdKey && (await copyText(createdKey))) {
                  setCopied(true);
                }
              }}
            >
              {copied ? <Check /> : <Copy />}
              {copied ? "Copied" : "Copy"}
            </Button>
          </div>
          <p className="text-[11px] text-muted-foreground">
            Send it as <code>Authorization: Bearer &lt;key&gt;</code> or{" "}
            <code>X-API-Key</code>.
          </p>
        </DialogBody>
      </Dialog>
    </div>
  );
}
