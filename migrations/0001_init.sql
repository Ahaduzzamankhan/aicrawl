-- AICrawl initial schema (SQLite only).
-- Timestamps are stored as RFC 3339 strings (UTC).

CREATE TABLE IF NOT EXISTS crawl_jobs (
    id              TEXT PRIMARY KEY,
    kind            TEXT NOT NULL CHECK (kind IN ('extract', 'crawl')),
    url             TEXT NOT NULL,
    query           TEXT,
    status          TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    output_format   TEXT NOT NULL DEFAULT 'json',
    schema_json     TEXT,
    max_pages       INTEGER,
    max_depth       INTEGER,
    pages_crawled   INTEGER NOT NULL DEFAULT 0,
    error           TEXT,
    meta_json       TEXT,
    created_at      TEXT NOT NULL,
    started_at      TEXT,
    finished_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_crawl_jobs_status_created ON crawl_jobs (status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_crawl_jobs_created ON crawl_jobs (created_at DESC);

CREATE TABLE IF NOT EXISTS crawl_pages (
    id              TEXT PRIMARY KEY,
    job_id          TEXT NOT NULL REFERENCES crawl_jobs (id) ON DELETE CASCADE,
    url             TEXT NOT NULL,
    depth           INTEGER NOT NULL DEFAULT 0,
    status_code     INTEGER,
    title           TEXT,
    content_type    TEXT,
    text_length     INTEGER NOT NULL DEFAULT 0,
    content_hash    TEXT,
    word_count      INTEGER NOT NULL DEFAULT 0,
    fetched_at      TEXT NOT NULL,
    error           TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_crawl_pages_job_url ON crawl_pages (job_id, url);
CREATE INDEX IF NOT EXISTS idx_crawl_pages_job ON crawl_pages (job_id);

CREATE TABLE IF NOT EXISTS extracted_results (
    id              TEXT PRIMARY KEY,
    job_id          TEXT NOT NULL REFERENCES crawl_jobs (id) ON DELETE CASCADE,
    url             TEXT NOT NULL,
    query           TEXT,
    mode            TEXT NOT NULL CHECK (mode IN ('deterministic', 'ai')),
    format          TEXT NOT NULL DEFAULT 'json',
    data_json       TEXT NOT NULL,
    warnings_json   TEXT NOT NULL DEFAULT '[]',
    provider        TEXT,
    validated       INTEGER NOT NULL DEFAULT 0,
    duration_ms     INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_extracted_results_job ON extracted_results (job_id);
CREATE INDEX IF NOT EXISTS idx_extracted_results_created ON extracted_results (created_at DESC);

CREATE TABLE IF NOT EXISTS websites (
    domain          TEXT PRIMARY KEY,
    scheme          TEXT,
    first_seen_at   TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    last_status     INTEGER,
    crawl_count     INTEGER NOT NULL DEFAULT 0,
    robots_allowed  INTEGER,
    notes           TEXT
);

CREATE TABLE IF NOT EXISTS settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS api_keys (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    key_hash        TEXT NOT NULL UNIQUE,
    prefix          TEXT NOT NULL,
    scopes          TEXT NOT NULL DEFAULT 'read,write',
    created_at      TEXT NOT NULL,
    last_used_at    TEXT,
    revoked_at      TEXT
);

CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys (key_hash);

CREATE TABLE IF NOT EXISTS ai_providers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('openai', 'openai_compatible', 'ollama')),
    base_url        TEXT NOT NULL,
    model           TEXT NOT NULL,
    api_key_enc     TEXT,
    is_default      INTEGER NOT NULL DEFAULT 0,
    max_tokens      INTEGER NOT NULL DEFAULT 1024,
    temperature     REAL NOT NULL DEFAULT 0.0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_providers_default ON ai_providers (is_default) WHERE is_default = 1;

CREATE TABLE IF NOT EXISTS page_cache (
    url             TEXT PRIMARY KEY,
    final_url       TEXT NOT NULL,
    status_code     INTEGER NOT NULL,
    content_type    TEXT NOT NULL,
    body            TEXT NOT NULL,
    content_hash    TEXT NOT NULL,
    body_bytes      INTEGER NOT NULL,
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_page_cache_expires ON page_cache (expires_at);

CREATE TABLE IF NOT EXISTS robots_cache (
    origin          TEXT PRIMARY KEY,
    body            TEXT NOT NULL,
    status_code     INTEGER NOT NULL,
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT NOT NULL
);
