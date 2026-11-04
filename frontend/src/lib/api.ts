/** Typed client for the AICrawl REST API (request schema v1). */

const BASE = import.meta.env.VITE_AICRAWL_API ?? "/api/v1";

export class ApiError extends Error {
  code: string;
  status: number;
  retryable: boolean;
  constructor(message: string, code = "request_failed", status = 0, retryable = false) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
    this.retryable = retryable;
  }
}

export interface Issue {
  code: string;
  field: string;
  message: string;
  required: boolean;
}

export interface ErrorBody {
  code: string;
  message: string;
  retryable: boolean;
}

export interface PageMeta {
  url: string;
  final_url?: string | null;
  status?: number | null;
  items_extracted: number;
  changed: boolean;
  error?: ErrorBody | null;
}

export interface ResponseMeta {
  url?: string | null;
  final_url?: string | null;
  status?: number | null;
  pages_processed: number;
  items_extracted: number;
  job_id?: string | null;
  result_id?: string | null;
  duration_ms: number;
  render: string;
  cached: boolean;
  truncated: boolean;
  projection?: string;
  warnings?: string[];
  pages?: PageMeta[];
}

/** The single response envelope returned by every extraction interface. */
export interface ExtractionResponse<T = unknown> {
  success: boolean;
  data: T;
  meta?: ResponseMeta;
  errors?: Issue[];
  error?: ErrorBody;
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
  mode: string;
  format: string;
  data: unknown;
  warnings: string[];
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

export interface SecuritySummary {
  allow_private_networks: boolean;
  allowed_domains: string[];
  blocked_domains: string[];
  max_redirects: number;
  rate_limit_per_minute: number;
  respect_robots_txt: boolean;
}

export interface Health {
  status: string;
  version: string;
  schema_version: string;
  uptime_ms: number;
  database: { status: string; path: string };
  data_dir: string;
  stats: Stats;
  renderer: { available: boolean; detail?: string };
  security: SecuritySummary;
  capabilities: {
    actions: string[];
    output_formats: string[];
    selectors: string[];
    structured_data: string[];
    schema_validation: boolean;
    rendering: string;
    ai_required: boolean;
  };
  api: { host: string; port: number };
  mcp: { host: string; port: number; endpoint: string };
}

export interface OverviewData {
  stats: Stats;
  recent_jobs: Job[];
  recent_results: ExtractedResult[];
}

export interface SettingsPayload {
  version: string;
  schema_version: string;
  data_dir: string;
  database: string;
  server: { api_host: string; api_port: number; mcp_host: string; mcp_port: number };
  crawler: {
    max_pages: number;
    max_depth: number;
    concurrency: number;
    timeout_ms: number;
    render_timeout_ms: number;
    max_body_bytes: number;
    max_text_chars: number;
    max_crawl_secs: number;
    user_agent: string;
    respect_robots_txt: boolean;
    same_domain: boolean;
  };
  security: SecuritySummary;
  extract: { max_items: number; max_text_chars: number };
  cache: { enabled: boolean; ttl_secs: number; max_entries: number };
  api: { require_api_key: boolean; cors_origins: string[] };
  ai_required: boolean;
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

export interface McpTool {
  name: string;
  description: string;
  arguments: string[];
}

export interface McpConfig {
  http: { endpoint: string; transport: string; client_config: Record<string, unknown> };
  stdio: { command: string; args: string[]; client_config: Record<string, unknown> };
  tools: McpTool[];
  notice: string;
}

function authHeaders(): Record<string, string> {
  const key = localStorage.getItem("aicrawl.apiKey");
  return key ? { Authorization: `Bearer ${key}` } : {};
}

/** Calls a management endpoint and returns `data`, throwing on failure. */
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
  } catch {
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
      Boolean(body?.error?.retryable),
    );
  }
  return body?.data as T;
}

/** Runs an extraction request and returns the full envelope (never throws for
 * extraction failures, so the UI can show `error.code` and field issues). */
async function envelope<T = unknown>(
  path: string,
  payload: unknown,
): Promise<ExtractionResponse<T>> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", ...authHeaders() },
      body: JSON.stringify(payload),
    });
  } catch {
    throw new ApiError(
      `Cannot reach the AICrawl API at ${BASE}. Is the server running?`,
      "network_error",
    );
  }
  const body = (await response.json().catch(() => null)) as ExtractionResponse<T> | null;
  if (!body) {
    throw new ApiError("The API returned an empty response", "invalid_response", response.status);
  }
  return body;
}

export const api = {
  health: () => request<Health>("/health"),
  overview: () => request<OverviewData>("/stats"),
  settings: () => request<SettingsPayload>("/settings"),

  validate: (payload: unknown) => envelope<{ valid: boolean; targets: string[] }>("/validate", payload),
  run: (payload: unknown) => envelope("/extract", payload),
  crawl: (payload: unknown) => envelope("/crawl", payload),
  batch: (payload: unknown) => envelope("/batch", payload),
  map: (payload: unknown) => envelope("/map", payload),
  exportDataset: (jobId: string, format = "json") =>
    envelope<{ job_id: string; format: string; dataset: string }>("/export", {
      job_id: jobId,
      format,
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
    request<{ job_id: string; cancelled: boolean }>(`/jobs/${id}/cancel`, { method: "POST" }),

  results: (params: { limit?: number; offset?: number } = {}) => {
    const query = new URLSearchParams();
    if (params.limit) query.set("limit", String(params.limit));
    if (params.offset) query.set("offset", String(params.offset));
    const suffix = query.toString() ? `?${query}` : "";
    return request<{ results: ExtractedResult[] }>(`/results${suffix}`);
  },

  apiKeys: () => request<{ api_keys: ApiKey[] }>("/api-keys"),
  createApiKey: (name: string) =>
    request<{ api_key: ApiKey; key: string; note: string }>("/api-keys", {
      method: "POST",
      body: JSON.stringify({ name }),
    }),
  revokeApiKey: (id: string) =>
    request<{ revoked: string }>(`/api-keys/${id}`, { method: "DELETE" }),

  mcp: () => request<McpConfig>("/mcp"),
};
