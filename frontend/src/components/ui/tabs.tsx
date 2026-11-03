import {
  createContext,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { cn } from "@/lib/utils";

interface TabsContextValue {
  value: string;
  setValue: (value: string) => void;
}

const TabsContext = createContext<TabsContextValue | null>(null);

interface TabsProps {
  defaultValue: string;
  value?: string;
  onValueChange?: (value: string) => void;
  children: ReactNode;
  className?: string;
}

export function Tabs({
  defaultValue,
  value,
  onValueChange,
  children,
  className,
}: TabsProps) {
  const [internal, setInternal] = useState(defaultValue);
  const current = value ?? internal;
  const context = useMemo<TabsContextValue>(
    () => ({
      value: current,
      setValue: (next) => {
        setInternal(next);
        onValueChange?.(next);
      },
    }),
    [current, onValueChange],
  );

  return (
    <TabsContext.Provider value={context}>
      <div className={cn("flex flex-col gap-4", className)}>{children}</div>
    </TabsContext.Provider>
  );
}

export function TabsList({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      role="tablist"
      className={cn(
        "inline-flex w-fit items-center gap-1 rounded-lg bg-muted p-1",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function TabsTrigger({
  value,
  children,
  className,
}: {
  value: string;
  children: ReactNode;
  className?: string;
}) {
  const context = useContext(TabsContext);
  if (!context) throw new Error("TabsTrigger must be used inside Tabs");
  const active = context.value === value;
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={() => context.setValue(value)}
      className={cn(
        "rounded-md px-3 py-1 text-xs font-medium transition-colors",
        active
          ? "bg-background text-foreground shadow-sm"
          : "text-muted-foreground hover:text-foreground",
        className,
      )}
    >
      {children}
    </button>
  );
}

export function TabsContent({
  value,
  children,
  className,
}: {
  value: string;
  children: ReactNode;
  className?: string;
}) {
  const context = useContext(TabsContext);
  if (!context) throw new Error("TabsContent must be used inside Tabs");
  if (context.value !== value) return null;
  return (
    <div role="tabpanel" className={cn("animate-in", className)}>
      {children}
    </div>
  );
}
