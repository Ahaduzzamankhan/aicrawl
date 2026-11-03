import { useCallback, useEffect, useRef, useState } from "react";

interface PollingState<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  refresh: () => void;
}

/**
 * Polls an async loader while the document is visible.
 * A 4s default keeps the dashboard responsive without hammering the engine.
 */
export function usePolling<T>(
  loader: () => Promise<T>,
  intervalMs = 4000,
  enabled = true,
): PollingState<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const loaderRef = useRef(loader);
  loaderRef.current = loader;

  const run = useCallback(async () => {
    try {
      const next = await loaderRef.current();
      setData(next);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let timer: number | undefined;

    const tick = async () => {
      if (cancelled) return;
      if (document.visibilityState === "visible") {
        await run();
      }
      if (!cancelled) {
        timer = window.setTimeout(tick, intervalMs);
      }
    };

    void tick();
    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [enabled, intervalMs, run]);

  return { data, error, loading, refresh: run };
}
