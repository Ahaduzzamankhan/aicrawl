import type { ReactNode } from "react";
import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export function StatCard({
  label,
  value,
  hint,
  icon,
  tone = "default",
}: {
  label: string;
  value: ReactNode;
  hint?: string;
  icon?: ReactNode;
  tone?: "default" | "success" | "warning" | "danger";
}) {
  return (
    <Card className="p-4">
      <div className="flex items-center justify-between">
        <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
          {label}
        </p>
        {icon ? (
          <span
            className={cn(
              "text-muted-foreground",
              tone === "success" && "text-success",
              tone === "warning" && "text-warning",
              tone === "danger" && "text-destructive",
            )}
          >
            {icon}
          </span>
        ) : null}
      </div>
      <p className="mt-2 text-2xl font-semibold tabular-nums tracking-tight">
        {value}
      </p>
      {hint ? (
        <p className="mt-1 text-[11px] text-muted-foreground">{hint}</p>
      ) : null}
    </Card>
  );
}
