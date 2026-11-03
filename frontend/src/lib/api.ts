/** Typed client for the AICrawl REST API. */

const BASE = import.meta.env.VITE_AICRAWL_API ?? "/api/v1";

export class ApiError extends Error {
  code: string;
  status: number;
  constructor(message: string, code = "request_failed", status = 0) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }
}

export interface Job {
  id: string;
  kind: "extract" | "crawl";
  url: string;
  query?: string | null;
  status: "queued" | "running" | "succeeded" | "failed" | "cancelled";
  output_format: string;
  schema_json?: string | null;
  max_pages?: number | null;
  max_depth?: number | null;
  pages_crawled: number;
  error?: string | null;
  created_at: string;
  started_at?: string | null;
  finished_at?: string | null;
}

export interface CrawlPage {
  id: string;
  job_id: string;
  url: string;
  depth: number;
  status_code?: number | null;
  title?: string | null;
  content_type?: string | null;
  text_length: number;
  word_count: number;
  fetched_at: string;
  error?: string | null;
}

export interface ExtractedResult {
  id: string;
  job_id: string;
  url: string;
  query?: string | null;
  mode: "deterministic" | "ai";
  format: string;
  data: unknown;
  warnings: string[];
  provider?: string | null;
  validated: boolean;
  duration_ms: number;
  created_at: string;
}

export interface JobDetail {
  job: Job;
  pages: CrawlPage[];
  results: ExtractedResult[];
}

export interface Stats {
  jobs_total: number;
  jobs_running: number;
  jobs_failed: number;
  jobs_succeeded: number;
  pages_total: number;
  results_total: number;
  websites_total: number;
}

export interface AiStatus {
  configured: boolean;
  label?: string | null;
  model?: string | null;
  kind?: string | null;
  source?: string | null;
  detail: string;
}

export interface Overview {
  stats: Stats;
  recent_jobs: Job[];
  recent_results: ExtractedResult[];
  ai: AiStatus;
  cache_entries: number;
}

export interface Health {
  status: string;
  version: string;
  uptime_secs: number;
  database: { ok: boolean; path: string; error?: string | null };
  stats: Stats;
  cache_entries: number;
  ai: AiStatus;
  security: {
    allow_private_networks: boolean;
    allowed_domains: string[];
    blocked_domains: string[];
    max_redirects: number;
    rate_limit_per_minute: number;
    respect_robots_txt: boolean;
  };
  api: { host: string; port: number };
  mcp: { host: string; port: number; endpoint: string };
  data_dir: string;
}

export interface ExtractPayload {
  url: string;
  query: string;
  schema?: unknown;
  output_format?: string;
  use_ai?: boolean;
  render?: boolean;
  use_cache?: boolean;
}

export interface ExtractResponse {
  job: Job;
  result: ExtractedResult;
  outcome: {
    url: string;
    query: string;
    mode: "deterministic" | "ai";
    format: string;
    data: unknown;
    output: string;
    warnings: string[];
    provider?: string | null;
    validated: boolean;
    duration_ms: number;
    fetch_ms: number;
    page: {
      final_url: string;
      status: number;
      content_type: string;
      title?: string | null;
      description?: string | null;
      word_count: number;
      bytes: number;
      truncated: boolean;
      from_cache: boolean;
      rendered: boolean;
    };
  };
}

export interface Provider {
  id: string;
  name: string;
  kind: "openai" | "openai_compatible" | "ollama";
  base_url: string;
  model: string;
  has_api_key: boolean;
  is_default: boolean;
  max_tokens: number;
  temperature: number;
  created_at: string;
  updated_at: string;
}

export interface ProviderPayload {
  name: string;
  kind: string;
  base_url: string;
  model: string;
  api_key?: string;
  is_default?: boolean;
  max_tokens?: number;
  temperature?: number;
}

export interface ApiKey {
  id: string;
  name: string;
  prefix: string;
  scopes: string;
  created_at: string;
  last_used_at?: string | null;
  revoked_at?: string | null;
}

export interface McpConfig {
  http: {
    endpoint: string;
    transport: string;
    client_config: Record<string, unknown>;
  };
  stdio: {
    command: string;
    args: string[];
    client_config: Record<string, unknown>;
  };
  tools: { name: string; description: string; arguments: string[] }[];
  notice: string;
}

export interface SettingsResponse {
  effective: Record<string, unknown>;
  preferences: Record<string, string>;
  security: Health["security"];
  storage: {
    path: string;
    data_dir: string;
    stats: Stats;
    cache_entries: number;
  };
  runtime: {
    version: string;
    api_requires_key: boolean;
    mcp_endpoint: string;
  };
}

function authHeaders(): Record<string, string> {
  const key = localStorage.getItem("aicrawl.apiKey");
  return key ? { Authorization: `Bearer ${key}` } : {};
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, {
      ...init,
      headers: {
        "Content-Type": "application/json",
        ...authHeaders(),
        ...(init?.headers ?? {}),
      },
    });
  } catch (error) {
    throw new ApiError(
      `Cannot reach the AICrawl API at ${BASE}. Is the server running?`,
      "network_error",
    );
  }

  const body = await response.json().catch(() => null);
  if (!response.ok || body?.success === false) {
    throw new ApiError(
      body?.error?.message ?? response.statusText ?? "request failed",
      body?.error?.code ?? "request_failed",
      response.status,
    );
  }
  return body?.data as T;
}

export const api = {
  health: () => request<Health>("/health"),
  overview: () => request<Overview>("/stats"),
  extract: (payload: ExtractPayload) =>
    request<ExtractResponse>("/extract", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  startCrawl: (payload: {
    url: string;
    max_pages?: number;
    max_depth?: number;
    query?: string;
    same_domain?: boolean;
  }) =>
    request<{ job: Job }>("/crawl", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  jobs: (params: { status?: string; limit?: number; offset?: number } = {}) => {
    const query = new URLSearchParams();
    if (params.status) query.set("status", params.status);
    if (params.limit) query.set("limit", String(params.limit));
    if (params.offset) query.set("offset", String(params.offset));
    const suffix = query.toString() ? `?${query}` : "";
    return request<{ jobs: Job[] }>(`/jobs${suffix}`);
  },
  job: (id: string) => request<JobDetail>(`/jobs/${id}`),
  cancelJob: (id: string) =>
    request<{ job_id: string; cancelled: boolean }>(`/jobs/${id}/cancel`, {
      method: "POST",
    }),
  results: (params: { limit?: number; offset?: number } = {}) => {
    const query = new URLSearchParams();
    if (params.limit) query.set("limit", String(params.limit));
    if (params.offset) query.set("offset", String(params.offset));
    const suffix = query.toString() ? `?${query}` : "";
    return request<{ results: ExtractedResult[] }>(`/results${suffix}`);
  },
  result: (id: string) => request<ExtractedResult>(`/results/${id}`),
  settings: () => request<SettingsResponse>("/settings"),
  putSettings: (preferences: Record<string, string>) =>
    request<SettingsResponse>("/settings", {
      method: "PUT",
      body: JSON.stringify({ preferences }),
    }),
  providers: () =>
    request<{ providers: Provider[]; ai: AiStatus }>("/providers"),
  createProvider: (payload: ProviderPayload) =>
    request<{ provider: Provider }>("/providers", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  updateProvider: (id: string, payload: ProviderPayload) =>
    request<{ provider: Provider }>(`/providers/${id}`, {
      method: "PUT",
      body: JSON.stringify(payload),
    }),
  deleteProvider: (id: string) =>
    request<{ deleted: string }>(`/providers/${id}`, { method: "DELETE" }),
  setDefaultProvider: (id: string) =>
    request<{ default: string }>(`/providers/${id}/default`, { method: "POST" }),
  testProvider: (id: string) =>
    request<{ provider: string; ok: boolean; reply: unknown; latency_ms: number }>(
      `/providers/${id}/test`,
      { method: "POST", body: "{}" },
    ),
  apiKeys: () => request<{ api_keys: ApiKey[] }>("/api-keys"),
  createApiKey: (name: string) =>
    request<{ api_key: ApiKey; key: string; note: string }>("/api-keys", {
      method: "POST",
      body: JSON.stringify({ name }),
    }),
  revokeApiKey: (id: string) =>
    request<{ revoked: string }>(`/api-keys/${id}`, { method: "DELETE" }),
  mcp: () => request<McpConfig>("/mcp"),
  clearCache: () =>
    request<{ cache: string }>("/cache", { method: "DELETE" }),
};
