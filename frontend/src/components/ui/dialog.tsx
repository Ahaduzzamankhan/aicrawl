import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
  className?: string;
}

export function Dialog({ open, onOpenChange, children, className }: DialogProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onOpenChange(false);
    };
    document.addEventListener("keydown", onKey);
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = previous;
    };
  }, [open, onOpenChange]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={() => onOpenChange(false)}
        aria-hidden
      />
      <div
        role="dialog"
        aria-modal="true"
        className={cn(
          "relative flex max-h-[88vh] w-full max-w-3xl flex-col overflow-hidden rounded-xl border border-border bg-card shadow-2xl animate-in",
          className,
        )}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}

export function DialogHeader({
  children,
  onClose,
}: {
  children: ReactNode;
  onClose?: () => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-border p-5">
      <div className="min-w-0">{children}</div>
      {onClose ? (
        <button
          onClick={onClose}
          className="rounded-md p-1 text-muted-foreground transition hover:bg-muted hover:text-foreground"
          aria-label="Close dialog"
        >
          <X className="size-4" />
        </button>
      ) : null}
    </div>
  );
}

export function DialogTitle({ children }: { children: ReactNode }) {
  return <h2 className="text-base font-semibold tracking-tight">{children}</h2>;
}

export function DialogDescription({ children }: { children: ReactNode }) {
  return <p className="mt-1 text-xs text-muted-foreground">{children}</p>;
}

export function DialogBody({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("scrollbar-thin flex-1 overflow-y-auto p-5", className)}>
      {children}
    </div>
  );
}
