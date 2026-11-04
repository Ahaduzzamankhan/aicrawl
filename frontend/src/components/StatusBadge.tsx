import { Badge } from "@/components/ui/badge";
import type { Job } from "@/lib/api";

export function StatusBadge({ status }: { status: Job["status"] }) {
  switch (status) {
    case "succeeded":
      return <Badge variant="success">succeeded</Badge>;
    case "running":
      return <Badge variant="warning">running</Badge>;
    case "queued":
      return <Badge variant="secondary">queued</Badge>;
    case "failed":
      return <Badge variant="destructive">failed</Badge>;
    case "cancelled":
      return <Badge variant="outline">cancelled</Badge>;
    default:
      return <Badge variant="muted">{status}</Badge>;
  }
}

