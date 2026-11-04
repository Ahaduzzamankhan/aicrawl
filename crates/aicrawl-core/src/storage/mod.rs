//! SQLite persistence. SQLite is the only database used by AICrawl.
//!
//! Connections are pooled, all statements are parameterised (prepared by SQLx)
//! and multi-row operations run inside transactions. Timestamps are stored as
//! RFC 3339 strings so the database remains portable and inspectable.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::error::{AicrawlError, Result};
use crate::util::{is_expired, new_id, now_rfc3339, rfc3339_offset};

/// Job lifecycle values stored in `crawl_jobs.status`.
pub mod job_status {
    pub const QUEUED: &str = "queued";
    pub const RUNNING: &str = "running";
    pub const SUCCEEDED: &str = "succeeded";
    pub const FAILED: &str = "failed";
    pub const CANCELLED: &str = "cancelled";

    /// Statuses that still accept work.
    pub fn is_active(status: &str) -> bool {
        status == QUEUED || status == RUNNING
    }
}

/// Kind of work recorded in `crawl_jobs.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// A single-page extraction.
    Extract,
    /// A multi-page crawl.
    Crawl,
}

impl JobKind {
    /// Database representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Crawl => "crawl",
        }
    }
}

/// A persisted job row.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CrawlJob {
    pub id: String,
    pub kind: String,
    pub url: String,
    pub query: Option<String>,
    pub status: String,
    pub output_format: String,
    pub schema_json: Option<String>,
    pub max_pages: Option<i64>,
    pub max_depth: Option<i64>,
    pub pages_crawled: i64,
    pub error: Option<String>,
    pub meta_json: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl CrawlJob {
    /// Whether the job is still queued or running.
    pub fn is_active(&self) -> bool {
        job_status::is_active(&self.status)
    }

    /// Parsed schema, when the job requested one.
    pub fn schema(&self) -> Option<serde_json::Value> {
        self.schema_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
    }
}

/// A fetched page recorded under a crawl job.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CrawlPage {
    pub id: String,
    pub job_id: String,
    pub url: String,
    pub depth: i64,
    pub status_code: Option<i64>,
    pub title: Option<String>,
    pub content_type: Option<String>,
    pub text_length: i64,
    pub content_hash: Option<String>,
    pub word_count: i64,
    pub fetched_at: String,
    pub error: Option<String>,
}

/// An extraction result.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExtractedResult {
    pub id: String,
    pub job_id: String,
    pub url: String,
    pub query: Option<String>,
    pub mode: String,
    pub format: String,
    pub data_json: String,
    pub warnings_json: String,
    pub provider: Option<String>,
    pub validated: i64,
    pub duration_ms: i64,
    pub created_at: String,
}

impl ExtractedResult {
    /// Parsed extraction payload.
    pub fn data(&self) -> serde_json::Value {
        serde_json::from_str(&self.data_json).unwrap_or(serde_json::Value::Null)
    }

    /// Parsed warnings list.
    pub fn warnings(&self) -> Vec<String> {
        serde_json::from_str(&self.warnings_json).unwrap_or_default()
    }

    /// Whether the payload passed schema validation.
    pub fn is_validated(&self) -> bool {
        self.validated != 0
    }
}

/// Website-level metadata accumulated across crawls.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebsiteMeta {
    pub domain: String,
    pub scheme: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub last_status: Option<i64>,
    pub crawl_count: i64,
    pub robots_allowed: Option<i64>,
    pub notes: Option<String>,
}

/// Stored API key metadata. The plaintext key is never persisted.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApiKeyRecord {
    pub id: String,
    pub name: String,
    pub key_hash: String,
    pub prefix: String,
    pub scopes: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

impl ApiKeyRecord {
    /// Removes the hash before the record leaves the process.
    pub fn redacted(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "prefix": self.prefix,
            "scopes": self.scopes,
            "created_at": self.created_at,
            "last_used_at": self.last_used_at,
            "revoked_at": self.revoked_at,
        })
    }
}

/// AI provider configuration. `api_key_enc` holds ciphertext only.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiProviderRecord {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    pub api_key_enc: Option<String>,
    pub is_default: i64,
    pub max_tokens: i64,
    pub temperature: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl AiProviderRecord {
    /// Whether a provider key is stored.
    pub fn has_api_key(&self) -> bool {
        self.api_key_enc.is_some()
    }

    /// JSON representation with the ciphertext replaced by a boolean.
    pub fn redacted(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "kind": self.kind,
            "base_url": self.base_url,
            "model": self.model,
            "has_api_key": self.has_api_key(),
            "is_default": self.is_default != 0,
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

/// Cached page body and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CachedPage {
    pub url: String,
    pub final_url: String,
    pub status_code: i64,
    pub content_type: String,
    pub body: String,
    pub content_hash: String,
    pub body_bytes: i64,
    pub fetched_at: String,
    pub expires_at: String,
}

/// Cached robots.txt document.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RobotsCacheRow {
    pub origin: String,
    pub body: String,
    pub status_code: i64,
    pub fetched_at: String,
    pub expires_at: String,
}

/// Aggregate counters used by the dashboard overview.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreStats {
    pub jobs_total: i64,
    pub jobs_running: i64,
    pub jobs_failed: i64,
    pub jobs_succeeded: i64,
    pub pages_total: i64,
    pub results_total: i64,
    pub websites_total: i64,
}

/// Input for creating a job.
#[derive(Debug, Clone)]
pub struct NewJob {
    pub kind: JobKind,
    pub url: String,
    pub query: Option<String>,
    pub output_format: String,
    pub schema_json: Option<String>,
    pub max_pages: Option<u32>,
    pub max_depth: Option<u32>,
    pub meta_json: Option<String>,
}

/// Input for recording a fetched page.
#[derive(Debug, Clone)]
pub struct NewPage {
    pub job_id: String,
    pub url: String,
    pub depth: u32,
    pub status_code: Option<u16>,
    pub title: Option<String>,
    pub content_type: Option<String>,
    pub text_length: usize,
    pub content_hash: Option<String>,
    pub word_count: usize,
    pub error: Option<String>,
}

/// Input for recording an extraction result.
#[derive(Debug, Clone)]
pub struct NewResult {
    pub job_id: String,
    pub url: String,
    pub query: Option<String>,
    pub mode: String,
    pub format: String,
    pub data_json: String,
    pub warnings_json: String,
    pub provider: Option<String>,
    pub validated: bool,
    pub duration_ms: i64,
}

/// Input for creating or updating an AI provider.
#[derive(Debug, Clone, Default)]
pub struct NewProvider {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    /// Ciphertext from [`crate::security::SecretStore::encrypt`].
    pub api_key_enc: Option<String>,
    pub is_default: bool,
    pub max_tokens: i64,
    pub temperature: f64,
}

/// Input for caching a page body.
#[derive(Debug, Clone)]
pub struct NewCacheEntry {
    pub url: String,
    pub final_url: String,
    pub status_code: u16,
    pub content_type: String,
    pub body: String,
    pub content_hash: String,
    pub body_bytes: usize,
    pub ttl_secs: i64,
}

/// SQLite-backed store.
#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
    location: String,
}

impl std::fmt::Debug for SqliteStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteStore")
            .field("location", &self.location)
            .finish()
    }
}

const JOB_COLUMNS: &str = "id, kind, url, query, status, output_format, schema_json, max_pages, max_depth, pages_crawled, error, meta_json, created_at, started_at, finished_at";
const PAGE_COLUMNS: &str = "id, job_id, url, depth, status_code, title, content_type, text_length, content_hash, word_count, fetched_at, error";
const RESULT_COLUMNS: &str = "id, job_id, url, query, mode, format, data_json, warnings_json, provider, validated, duration_ms, created_at";
const PROVIDER_COLUMNS: &str = "id, name, kind, base_url, model, api_key_enc, is_default, max_tokens, temperature, created_at, updated_at";

impl SqliteStore {
    /// Opens (or creates) the database at `path` and runs migrations.
    pub async fn connect(path: &Path, max_connections: u32) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(10));
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections.max(1))
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;
        let store = Self {
            pool,
            location: path.display().to_string(),
        };
        store.migrate().await?;
        Ok(store)
    }

    /// Opens an isolated in-memory database (tests and `doctor`).
    pub async fn connect_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = Self {
            pool,
            location: "sqlite::memory:".into(),
        };
        store.migrate().await?;
        Ok(store)
    }

    /// Applies embedded migrations.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.pool)
            .await
            .map_err(|e| AicrawlError::Config(format!("database migration failed: {e}")))?;
        Ok(())
    }

    /// Human-readable database location.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Underlying pool (used by health checks).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Closes all pooled connections.
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// Verifies the database answers queries.
    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    // ---------------------------------------------------------------- jobs

    /// Inserts a queued job and returns it.
    pub async fn create_job(&self, new: NewJob) -> Result<CrawlJob> {
        let id = new_id("job");
        let now = now_rfc3339();
        sqlx::query(
            "INSERT INTO crawl_jobs (id, kind, url, query, status, output_format, schema_json, max_pages, max_depth, pages_crawled, meta_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
        )
        .bind(&id)
        .bind(new.kind.as_str())
        .bind(&new.url)
        .bind(&new.query)
        .bind(job_status::QUEUED)
        .bind(&new.output_format)
        .bind(&new.schema_json)
        .bind(new.max_pages.map(|v| v as i64))
        .bind(new.max_depth.map(|v| v as i64))
        .bind(&new.meta_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_job(&id)
            .await?
            .ok_or_else(|| AicrawlError::Database(sqlx::Error::RowNotFound))
    }

    /// Marks a job as running.
    pub async fn start_job(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE crawl_jobs SET status = ?, started_at = COALESCE(started_at, ?) WHERE id = ?",
        )
        .bind(job_status::RUNNING)
        .bind(now_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Updates only the status (used for queued -> running and cancellation).
    pub async fn set_job_status(&self, id: &str, status: &str, error: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE crawl_jobs SET status = ?, error = COALESCE(?, error) WHERE id = ?")
            .bind(status)
            .bind(error)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Finishes a job with an optional error message.
    pub async fn finish_job(
        &self,
        id: &str,
        status: &str,
        pages: u32,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE crawl_jobs SET status = ?, pages_crawled = ?, error = ?, finished_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(pages as i64)
        .bind(error)
        .bind(now_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Increments the page counter for a running crawl.
    pub async fn increment_job_pages(&self, id: &str, delta: i64) -> Result<()> {
        sqlx::query("UPDATE crawl_jobs SET pages_crawled = pages_crawled + ? WHERE id = ?")
            .bind(delta)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Fetches a single job.
    pub async fn get_job(&self, id: &str) -> Result<Option<CrawlJob>> {
        let sql = format!("SELECT {JOB_COLUMNS} FROM crawl_jobs WHERE id = ?");
        let job = sqlx::query_as::<_, CrawlJob>(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(job)
    }

    /// Lists jobs, newest first, optionally filtered by status.
    pub async fn list_jobs(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CrawlJob>> {
        let limit = limit.clamp(1, 500);
        let sql = match status {
            Some(_) => format!(
                "SELECT {JOB_COLUMNS} FROM crawl_jobs WHERE status = ? ORDER BY created_at DESC LIMIT ? OFFSET ?"
            ),
            None => format!(
                "SELECT {JOB_COLUMNS} FROM crawl_jobs ORDER BY created_at DESC LIMIT ? OFFSET ?"
            ),
        };
        let mut query = sqlx::query_as::<_, CrawlJob>(sqlx::AssertSqlSafe(sql));
        if let Some(status) = status {
            query = query.bind(status.to_string());
        }
        let jobs = query
            .bind(limit)
            .bind(offset.max(0))
            .fetch_all(&self.pool)
            .await?;
        Ok(jobs)
    }

    /// Most recent jobs (dashboard overview).
    pub async fn recent_jobs(&self, limit: i64) -> Result<Vec<CrawlJob>> {
        self.list_jobs(None, limit, 0).await
    }

    /// Requests cancellation of an active job. Returns true when it changed state.
    pub async fn cancel_job(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE crawl_jobs SET status = ?, finished_at = ? WHERE id = ? AND status IN (?, ?)",
        )
        .bind(job_status::CANCELLED)
        .bind(now_rfc3339())
        .bind(id)
        .bind(job_status::QUEUED)
        .bind(job_status::RUNNING)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // --------------------------------------------------------------- pages

    /// Records (or replaces) a crawled page.
    pub async fn insert_page(&self, new: NewPage) -> Result<CrawlPage> {
        let id = new_id("page");
        let now = now_rfc3339();
        sqlx::query(
            "INSERT INTO crawl_pages (id, job_id, url, depth, status_code, title, content_type, text_length, content_hash, word_count, fetched_at, error)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (job_id, url) DO UPDATE SET
                status_code = excluded.status_code,
                title = excluded.title,
                content_type = excluded.content_type,
                text_length = excluded.text_length,
                content_hash = excluded.content_hash,
                word_count = excluded.word_count,
                fetched_at = excluded.fetched_at,
                error = excluded.error",
        )
        .bind(&id)
        .bind(&new.job_id)
        .bind(&new.url)
        .bind(new.depth as i64)
        .bind(new.status_code.map(|s| s as i64))
        .bind(&new.title)
        .bind(&new.content_type)
        .bind(new.text_length as i64)
        .bind(&new.content_hash)
        .bind(new.word_count as i64)
        .bind(&now)
        .bind(&new.error)
        .execute(&self.pool)
        .await?;
        let sql = format!("SELECT {PAGE_COLUMNS} FROM crawl_pages WHERE job_id = ? AND url = ?");
        let page = sqlx::query_as::<_, CrawlPage>(sqlx::AssertSqlSafe(sql))
            .bind(&new.job_id)
            .bind(&new.url)
            .fetch_one(&self.pool)
            .await?;
        Ok(page)
    }

    /// Lists every page recorded for a job.
    pub async fn list_pages(&self, job_id: &str) -> Result<Vec<CrawlPage>> {
        let sql = format!(
            "SELECT {PAGE_COLUMNS} FROM crawl_pages WHERE job_id = ? ORDER BY depth, fetched_at"
        );
        let pages = sqlx::query_as::<_, CrawlPage>(sqlx::AssertSqlSafe(sql))
            .bind(job_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(pages)
    }

    /// Counts pages recorded for a job.
    pub async fn count_pages(&self, job_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM crawl_pages WHERE job_id = ?")
            .bind(job_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    // ------------------------------------------------------------- results

    /// Stores an extraction result.
    pub async fn insert_result(&self, new: NewResult) -> Result<ExtractedResult> {
        let id = new_id("res");
        let now = now_rfc3339();
        sqlx::query(
            "INSERT INTO extracted_results (id, job_id, url, query, mode, format, data_json, warnings_json, provider, validated, duration_ms, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.job_id)
        .bind(&new.url)
        .bind(&new.query)
        .bind(&new.mode)
        .bind(&new.format)
        .bind(&new.data_json)
        .bind(&new.warnings_json)
        .bind(&new.provider)
        .bind(if new.validated { 1i64 } else { 0i64 })
        .bind(new.duration_ms)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_result(&id)
            .await?
            .ok_or_else(|| AicrawlError::Database(sqlx::Error::RowNotFound))
    }

    /// Atomically finishes an extraction job and stores its result.
    pub async fn complete_extract_job(
        &self,
        job_id: &str,
        status: &str,
        pages: u32,
        error: Option<&str>,
        result: NewResult,
    ) -> Result<ExtractedResult> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE crawl_jobs SET status = ?, pages_crawled = ?, error = ?, finished_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(pages as i64)
        .bind(error)
        .bind(now_rfc3339())
        .bind(job_id)
        .execute(&mut *tx)
        .await?;

        let id = new_id("res");
        sqlx::query(
            "INSERT INTO extracted_results (id, job_id, url, query, mode, format, data_json, warnings_json, provider, validated, duration_ms, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&result.job_id)
        .bind(&result.url)
        .bind(&result.query)
        .bind(&result.mode)
        .bind(&result.format)
        .bind(&result.data_json)
        .bind(&result.warnings_json)
        .bind(&result.provider)
        .bind(if result.validated { 1i64 } else { 0i64 })
        .bind(result.duration_ms)
        .bind(now_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get_result(&id)
            .await?
            .ok_or_else(|| AicrawlError::Database(sqlx::Error::RowNotFound))
    }

    /// Fetches one result.
    pub async fn get_result(&self, id: &str) -> Result<Option<ExtractedResult>> {
        let sql = format!("SELECT {RESULT_COLUMNS} FROM extracted_results WHERE id = ?");
        let row = sqlx::query_as::<_, ExtractedResult>(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Lists results, newest first.
    pub async fn list_results(&self, limit: i64, offset: i64) -> Result<Vec<ExtractedResult>> {
        let sql = format!(
            "SELECT {RESULT_COLUMNS} FROM extracted_results ORDER BY created_at DESC LIMIT ? OFFSET ?"
        );
        let rows = sqlx::query_as::<_, ExtractedResult>(sqlx::AssertSqlSafe(sql))
            .bind(limit.clamp(1, 500))
            .bind(offset.max(0))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Lists results belonging to a job.
    pub async fn results_for_job(&self, job_id: &str) -> Result<Vec<ExtractedResult>> {
        let sql = format!(
            "SELECT {RESULT_COLUMNS} FROM extracted_results WHERE job_id = ? ORDER BY created_at"
        );
        let rows = sqlx::query_as::<_, ExtractedResult>(sqlx::AssertSqlSafe(sql))
            .bind(job_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Latest result for a job (MCP `get_crawl_result`).
    pub async fn latest_result_for_job(&self, job_id: &str) -> Result<Option<ExtractedResult>> {
        let sql = format!(
            "SELECT {RESULT_COLUMNS} FROM extracted_results WHERE job_id = ? ORDER BY created_at DESC LIMIT 1"
        );
        let row = sqlx::query_as::<_, ExtractedResult>(sqlx::AssertSqlSafe(sql))
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    // ------------------------------------------------------------ websites

    /// Records a site visit, creating metadata on first sight.
    pub async fn touch_website(
        &self,
        domain: &str,
        scheme: &str,
        status: Option<u16>,
        robots_allowed: Option<bool>,
    ) -> Result<()> {
        let now = now_rfc3339();
        sqlx::query(
            "INSERT INTO websites (domain, scheme, first_seen_at, last_seen_at, last_status, crawl_count, robots_allowed)
             VALUES (?, ?, ?, ?, ?, 1, ?)
             ON CONFLICT (domain) DO UPDATE SET
                scheme = excluded.scheme,
                last_seen_at = excluded.last_seen_at,
                last_status = COALESCE(excluded.last_status, websites.last_status),
                crawl_count = websites.crawl_count + 1,
                robots_allowed = COALESCE(excluded.robots_allowed, websites.robots_allowed)",
        )
        .bind(domain)
        .bind(scheme)
        .bind(&now)
        .bind(&now)
        .bind(status.map(|s| s as i64))
        .bind(robots_allowed.map(|b| if b { 1i64 } else { 0i64 }))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Lists known websites, most recently seen first.
    pub async fn list_websites(&self, limit: i64) -> Result<Vec<WebsiteMeta>> {
        let rows = sqlx::query_as::<_, WebsiteMeta>(
            "SELECT domain, scheme, first_seen_at, last_seen_at, last_status, crawl_count, robots_allowed, notes
             FROM websites ORDER BY last_seen_at DESC LIMIT ?",
        )
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ------------------------------------------------------------ settings

    /// Reads a settings value.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.0))
    }

    /// Writes a settings value.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(now_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns every settings row as a map.
    pub async fn all_settings(&self) -> Result<std::collections::BTreeMap<String, String>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM settings ORDER BY key")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().collect())
    }

    // ------------------------------------------------------------ api keys

    /// Stores a hashed API key.
    pub async fn create_api_key(
        &self,
        name: &str,
        hash: &str,
        prefix: &str,
        scopes: &str,
    ) -> Result<ApiKeyRecord> {
        let id = new_id("key");
        let now = now_rfc3339();
        sqlx::query(
            "INSERT INTO api_keys (id, name, key_hash, prefix, scopes, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(hash)
        .bind(prefix)
        .bind(scopes)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_api_key(&id)
            .await?
            .ok_or_else(|| AicrawlError::Database(sqlx::Error::RowNotFound))
    }

    /// Fetches an API key record by id.
    pub async fn get_api_key(&self, id: &str) -> Result<Option<ApiKeyRecord>> {
        let row = sqlx::query_as::<_, ApiKeyRecord>(
            "SELECT id, name, key_hash, prefix, scopes, created_at, last_used_at, revoked_at FROM api_keys WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Finds an active (non-revoked) key by its hash.
    pub async fn find_active_api_key(&self, hash: &str) -> Result<Option<ApiKeyRecord>> {
        let row = sqlx::query_as::<_, ApiKeyRecord>(
            "SELECT id, name, key_hash, prefix, scopes, created_at, last_used_at, revoked_at
             FROM api_keys WHERE key_hash = ? AND revoked_at IS NULL",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Records that a key was used.
    pub async fn touch_api_key(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
            .bind(now_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Lists API keys (metadata only).
    pub async fn list_api_keys(&self) -> Result<Vec<ApiKeyRecord>> {
        let rows = sqlx::query_as::<_, ApiKeyRecord>(
            "SELECT id, name, key_hash, prefix, scopes, created_at, last_used_at, revoked_at FROM api_keys ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Revokes a key. Returns true when a live key was revoked.
    pub async fn revoke_api_key(&self, id: &str) -> Result<bool> {
        let result =
            sqlx::query("UPDATE api_keys SET revoked_at = ? WHERE id = ? AND revoked_at IS NULL")
                .bind(now_rfc3339())
                .bind(id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    // ------------------------------------------------------------ providers

    /// Inserts or updates an AI provider.
    pub async fn upsert_provider(&self, new: NewProvider) -> Result<AiProviderRecord> {
        let id = new.id.clone().unwrap_or_else(|| new_id("prov"));
        let now = now_rfc3339();
        let mut tx = self.pool.begin().await?;
        if new.is_default {
            sqlx::query("UPDATE ai_providers SET is_default = 0")
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "INSERT INTO ai_providers (id, name, kind, base_url, model, api_key_enc, is_default, max_tokens, temperature, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                name = excluded.name,
                kind = excluded.kind,
                base_url = excluded.base_url,
                model = excluded.model,
                api_key_enc = COALESCE(excluded.api_key_enc, ai_providers.api_key_enc),
                is_default = excluded.is_default,
                max_tokens = excluded.max_tokens,
                temperature = excluded.temperature,
                updated_at = excluded.updated_at",
        )
        .bind(&id)
        .bind(&new.name)
        .bind(&new.kind)
        .bind(&new.base_url)
        .bind(&new.model)
        .bind(&new.api_key_enc)
        .bind(if new.is_default { 1i64 } else { 0i64 })
        .bind(new.max_tokens)
        .bind(new.temperature)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.get_provider(&id)
            .await?
            .ok_or_else(|| AicrawlError::Database(sqlx::Error::RowNotFound))
    }

    /// Fetches a provider by id.
    pub async fn get_provider(&self, id: &str) -> Result<Option<AiProviderRecord>> {
        let sql = format!("SELECT {PROVIDER_COLUMNS} FROM ai_providers WHERE id = ?");
        let row = sqlx::query_as::<_, AiProviderRecord>(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Lists configured providers.
    pub async fn list_providers(&self) -> Result<Vec<AiProviderRecord>> {
        let sql = format!(
            "SELECT {PROVIDER_COLUMNS} FROM ai_providers ORDER BY is_default DESC, created_at"
        );
        let rows = sqlx::query_as::<_, AiProviderRecord>(sqlx::AssertSqlSafe(sql))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Returns the default provider, if one is configured.
    pub async fn default_provider(&self) -> Result<Option<AiProviderRecord>> {
        let sql =
            format!("SELECT {PROVIDER_COLUMNS} FROM ai_providers WHERE is_default = 1 LIMIT 1");
        let row = sqlx::query_as::<_, AiProviderRecord>(sqlx::AssertSqlSafe(sql))
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Marks a provider as the default inside a transaction.
    pub async fn set_default_provider(&self, id: &str) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM ai_providers WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        if exists.is_none() {
            return Ok(false);
        }
        sqlx::query("UPDATE ai_providers SET is_default = 0, updated_at = ?")
            .bind(now_rfc3339())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE ai_providers SET is_default = 1, updated_at = ? WHERE id = ?")
            .bind(now_rfc3339())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Deletes a provider. Returns false when it did not exist.
    pub async fn delete_provider(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM ai_providers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ---------------------------------------------------------------- cache

    /// Returns a non-expired cached page.
    pub async fn get_cached_page(&self, url: &str) -> Result<Option<CachedPage>> {
        let row = sqlx::query_as::<_, CachedPage>(
            "SELECT url, final_url, status_code, content_type, body, content_hash, body_bytes, fetched_at, expires_at
             FROM page_cache WHERE url = ?",
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.filter(|entry| !is_expired(&entry.expires_at)))
    }

    /// Stores a page body and refreshes its expiry.
    pub async fn put_cached_page(&self, entry: NewCacheEntry) -> Result<()> {
        sqlx::query(
            "INSERT INTO page_cache (url, final_url, status_code, content_type, body, content_hash, body_bytes, fetched_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (url) DO UPDATE SET
                final_url = excluded.final_url,
                status_code = excluded.status_code,
                content_type = excluded.content_type,
                body = excluded.body,
                content_hash = excluded.content_hash,
                body_bytes = excluded.body_bytes,
                fetched_at = excluded.fetched_at,
                expires_at = excluded.expires_at",
        )
        .bind(&entry.url)
        .bind(&entry.final_url)
        .bind(entry.status_code as i64)
        .bind(&entry.content_type)
        .bind(&entry.body)
        .bind(&entry.content_hash)
        .bind(entry.body_bytes as i64)
        .bind(now_rfc3339())
        .bind(rfc3339_offset(entry.ttl_secs))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deletes expired cache entries and trims the cache to `max_entries`.
    pub async fn prune_cache(&self, max_entries: i64) -> Result<()> {
        sqlx::query("DELETE FROM page_cache WHERE expires_at <= ?")
            .bind(now_rfc3339())
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "DELETE FROM page_cache WHERE url IN (
                SELECT url FROM page_cache ORDER BY fetched_at DESC LIMIT -1 OFFSET ?
             )",
        )
        .bind(max_entries.max(1))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Empties the page cache.
    pub async fn clear_cache(&self) -> Result<()> {
        sqlx::query("DELETE FROM page_cache")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Number of cached pages.
    pub async fn cache_entries(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM page_cache")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Reads a cached robots.txt document.
    pub async fn get_robots(&self, origin: &str) -> Result<Option<RobotsCacheRow>> {
        let row = sqlx::query_as::<_, RobotsCacheRow>(
            "SELECT origin, body, status_code, fetched_at, expires_at FROM robots_cache WHERE origin = ?",
        )
        .bind(origin)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.filter(|entry| !is_expired(&entry.expires_at)))
    }

    /// Caches a robots.txt document.
    pub async fn put_robots(
        &self,
        origin: &str,
        body: &str,
        status_code: u16,
        ttl_secs: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO robots_cache (origin, body, status_code, fetched_at, expires_at) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (origin) DO UPDATE SET body = excluded.body, status_code = excluded.status_code,
                fetched_at = excluded.fetched_at, expires_at = excluded.expires_at",
        )
        .bind(origin)
        .bind(body)
        .bind(status_code as i64)
        .bind(now_rfc3339())
        .bind(rfc3339_offset(ttl_secs))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---------------------------------------------------------------- stats

    /// Aggregate counters for dashboards and health checks.
    pub async fn stats(&self) -> Result<StoreStats> {
        let jobs_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM crawl_jobs")
            .fetch_one(&self.pool)
            .await?;
        let jobs_running: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM crawl_jobs WHERE status IN ('queued', 'running')")
                .fetch_one(&self.pool)
                .await?;
        let jobs_failed: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM crawl_jobs WHERE status = 'failed'")
                .fetch_one(&self.pool)
                .await?;
        let jobs_succeeded: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM crawl_jobs WHERE status = 'succeeded'")
                .fetch_one(&self.pool)
                .await?;
        let pages_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM crawl_pages")
            .fetch_one(&self.pool)
            .await?;
        let results_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM extracted_results")
            .fetch_one(&self.pool)
            .await?;
        let websites_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM websites")
            .fetch_one(&self.pool)
            .await?;
        Ok(StoreStats {
            jobs_total: jobs_total.0,
            jobs_running: jobs_running.0,
            jobs_failed: jobs_failed.0,
            jobs_succeeded: jobs_succeeded.0,
            pages_total: pages_total.0,
            results_total: results_total.0,
            websites_total: websites_total.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn job_lifecycle_and_results() {
        let store = SqliteStore::connect_memory().await.unwrap();
        let job = store
            .create_job(NewJob {
                kind: JobKind::Extract,
                url: "https://example.com/".into(),
                query: Some("products".into()),
                output_format: "json".into(),
                schema_json: None,
                max_pages: None,
                max_depth: None,
                meta_json: None,
            })
            .await
            .unwrap();
        assert_eq!(job.status, job_status::QUEUED);
        store.start_job(&job.id).await.unwrap();
        let running = store.get_job(&job.id).await.unwrap().unwrap();
        assert_eq!(running.status, job_status::RUNNING);

        let result = NewResult {
            job_id: job.id.clone(),
            url: job.url.clone(),
            query: job.query.clone(),
            mode: "deterministic".into(),
            format: "json".into(),
            data_json: r#"{"items":[]}"#.into(),
            warnings_json: "[]".into(),
            provider: None,
            validated: true,
            duration_ms: 12,
        };
        let saved = store
            .complete_extract_job(&job.id, job_status::SUCCEEDED, 1, None, result)
            .await
            .unwrap();
        assert_eq!(saved.data()["items"], serde_json::json!([]));
        assert!(saved.is_validated());

        let fetched = store.get_job(&job.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, job_status::SUCCEEDED);
        assert!(fetched.finished_at.is_some());
        assert_eq!(
            store
                .latest_result_for_job(&job.id)
                .await
                .unwrap()
                .unwrap()
                .id,
            saved.id
        );
        let stats = store.stats().await.unwrap();
        assert_eq!(stats.results_total, 1);
        assert_eq!(stats.jobs_succeeded, 1);
    }

    #[tokio::test]
    async fn pages_are_upserted_per_job() {
        let store = SqliteStore::connect_memory().await.unwrap();
        let job = store
            .create_job(NewJob {
                kind: JobKind::Crawl,
                url: "https://example.com/".into(),
                query: None,
                output_format: "json".into(),
                schema_json: None,
                max_pages: Some(5),
                max_depth: Some(1),
                meta_json: None,
            })
            .await
            .unwrap();
        for title in ["First", "Second"] {
            store
                .insert_page(NewPage {
                    job_id: job.id.clone(),
                    url: "https://example.com/".into(),
                    depth: 0,
                    status_code: Some(200),
                    title: Some(title.into()),
                    content_type: Some("text/html".into()),
                    text_length: 10,
                    content_hash: Some("abc".into()),
                    word_count: 2,
                    error: None,
                })
                .await
                .unwrap();
        }
        let pages = store.list_pages(&job.id).await.unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title.as_deref(), Some("Second"));
    }

    #[tokio::test]
    async fn api_keys_are_hashed_and_revocable() {
        let store = SqliteStore::connect_memory().await.unwrap();
        let hash = crate::security::hash_api_key("ac_live_test");
        let key = store
            .create_api_key("dashboard", &hash, "ac_live_test…", "read,write")
            .await
            .unwrap();
        assert!(store.find_active_api_key(&hash).await.unwrap().is_some());
        assert!(store.revoke_api_key(&key.id).await.unwrap());
        assert!(store.find_active_api_key(&hash).await.unwrap().is_none());
        assert!(!store.revoke_api_key(&key.id).await.unwrap());
    }

    #[tokio::test]
    async fn default_provider_is_exclusive() {
        let store = SqliteStore::connect_memory().await.unwrap();
        let first = store
            .upsert_provider(NewProvider {
                name: "first".into(),
                kind: "openai_compatible".into(),
                base_url: "https://api.example.com/v1".into(),
                model: "m1".into(),
                is_default: true,
                max_tokens: 100,
                ..Default::default()
            })
            .await
            .unwrap();
        let second = store
            .upsert_provider(NewProvider {
                name: "second".into(),
                kind: "openai_compatible".into(),
                base_url: "https://api.example.com/v1".into(),
                model: "m2".into(),
                is_default: true,
                max_tokens: 100,
                ..Default::default()
            })
            .await
            .unwrap();
        let default = store.default_provider().await.unwrap().unwrap();
        assert_eq!(default.id, second.id);
        assert!(!store
            .get_provider(&first.id)
            .await
            .unwrap()
            .unwrap()
            .is_default
            .eq(&1));

        // Updating without a new key preserves the stored ciphertext.
        let with_key = store
            .upsert_provider(NewProvider {
                id: Some(second.id.clone()),
                name: "second".into(),
                kind: "openai_compatible".into(),
                base_url: "https://api.example.com/v1".into(),
                model: "m3".into(),
                api_key_enc: Some("v1:abc".into()),
                is_default: true,
                max_tokens: 100,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(with_key.has_api_key());
        let updated = store
            .upsert_provider(NewProvider {
                id: Some(second.id.clone()),
                name: "second".into(),
                kind: "openai_compatible".into(),
                base_url: "https://api.example.com/v1".into(),
                model: "m4".into(),
                api_key_enc: None,
                is_default: true,
                max_tokens: 200,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(updated.has_api_key());
        assert_eq!(updated.model, "m4");
        assert_eq!(updated.max_tokens, 200);
    }

    #[tokio::test]
    async fn cache_respects_expiry_and_settings_round_trip() {
        let store = SqliteStore::connect_memory().await.unwrap();
        store
            .put_cached_page(NewCacheEntry {
                url: "https://example.com/".into(),
                final_url: "https://example.com/".into(),
                status_code: 200,
                content_type: "text/html".into(),
                body: "<html></html>".into(),
                content_hash: "h".into(),
                body_bytes: 13,
                ttl_secs: 60,
            })
            .await
            .unwrap();
        assert!(store
            .get_cached_page("https://example.com/")
            .await
            .unwrap()
            .is_some());
        store
            .put_cached_page(NewCacheEntry {
                url: "https://example.com/".into(),
                final_url: "https://example.com/".into(),
                status_code: 200,
                content_type: "text/html".into(),
                body: "x".into(),
                content_hash: "h".into(),
                body_bytes: 1,
                ttl_secs: -10,
            })
            .await
            .unwrap();
        assert!(store
            .get_cached_page("https://example.com/")
            .await
            .unwrap()
            .is_none());
        assert_eq!(store.cache_entries().await.unwrap(), 1);

        store.set_setting("theme", "dark").await.unwrap();
        assert_eq!(
            store.get_setting("theme").await.unwrap().as_deref(),
            Some("dark")
        );
        assert_eq!(store.all_settings().await.unwrap().len(), 1);
    }
}
