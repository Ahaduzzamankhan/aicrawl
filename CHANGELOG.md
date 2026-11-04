# Changelog

All notable changes to AICrawl are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Pre-`1.0.0` experimental builds may contain breaking changes.

## [1.0.0] - 2026-11-04

### Added

- Versioned JSON request contract (schema v1) shared by the CLI, REST API, MCP
  tools and dashboard, documented in `docs/request-schema.md` with a JSON Schema
  in `shared/schemas/extract-request.schema.json`.
- Deterministic selector engine: CSS, a documented XPath subset, attribute/text/
  sanitized-HTML rules, nested objects, repeated elements, required and optional
  fields, defaults and per-rule transforms.
- Structured data extraction: JSON-LD, Open Graph, metadata, headings, tables,
  prices and absolute URL resolution, plus a `document` projection.
- JSON Schema validation of extracted payloads with `SCHEMA_VALIDATION_FAILED`.
- Pagination with next-link following, de-duplication by field keys and bounded
  batch extraction with per-page results and a merged payload.
- Incremental crawling via content hashes, sitemap discovery, crawl
  cancellation and per-page stored results with dataset export.
- CLI commands `extract`, `crawl`, `batch`, `map`, `validate` with
  `--input`/`--stdin`/`--output`, JSON on stdout, diagnostics on stderr and
  stable exit codes.
- REST endpoints `/api/v1/{extract,crawl,batch,map,validate,export,results,mcp}`
  using the shared success/error envelope.
- Ten MCP tools: `extract_structured_data`, `crawl_website`, `map_website`,
  `inspect_website`, `batch_extract`, `search_pages`, `compare_pages`,
  `get_crawl_status`, `get_crawl_result`, `export_dataset`.
- Dashboard request workbench: JSON editor, validation, run, response viewer,
  copy/download, saved templates, request history and crawl progress.
- Prompt-injection screening that flags instruction-like page text as warnings.

### Changed

- The engine is JSON-first: requests describe selectors and output, and the
  response envelope (`success`, `data`, `meta`, `errors`, `error`) is
  identical across interfaces.
- `aicrawl api` and `aicrawl mcp` run in the foreground; the background daemon
  (`start`/`stop`/`status`) and PID files were removed.
- Playwright rendering timeout is configured with `crawler.render_timeout_ms`.

### Removed

- The AI provider stack: `/providers` API and dashboard page, encrypted provider
  credentials, `use_ai` request fields and the `[ai]` configuration section.
  Migration `0002` drops the `ai_providers` table.

### Fixed

- Crawl pages are stored under their real job id; the previous empty-id insert
  was rejected by the foreign key.

## [Unreleased]

### Planned

- Broader extraction coverage and regression fixtures.
- Packaging and signed release artifacts.
- Additional documentation and examples for agent integrations.

## [0.0.1-experimental.2] - 2026-11-04

### Added

- Translated READMEs in Bengali, Hindi, Spanish, Dutch, Chinese, Japanese and
  Russian, with a language selector on every page.
- `version.json` as the single source of truth for the release version.
- Manual **Release** workflow that creates the tag from `version.json`, builds
  Windows, Linux and macOS binaries and publishes them with SHA-256 checksums.
- Prebuilt Linux x64, macOS Intel and macOS Apple Silicon binaries alongside the
  existing Windows x64 build.

### Changed

- The version embedded in release binaries, the CLI/MCP server version and the
  default crawler user agent now come from `version.json` at build time.
- Automatic CI now runs tests and a release build on Windows, Linux and macOS
  for every push and pull request, with formatting and lint checks on Linux.
- Corrected the crawler user-agent repository URL.

## [0.0.1-experimental.1] - 2026-11-04

### Added

- Single `aicrawl` executable containing the CLI, embedded React dashboard,
  REST API and MCP server.
- CLI commands: `start`, `stop`, `status`, `extract`, `crawl`, `inspect`, `mcp`,
  `api`, `jobs`, `results`, `config`, `doctor` and `version`.
- Deterministic extraction with optional OpenAI-compatible or Ollama providers.
- MCP tools for extraction, crawling, page content, inspection, site search and
  job/result lookup over stdio and streamable HTTP.
- REST API with a consistent success/error envelope, API-key authentication and
  dashboard endpoints.
- React dashboard with overview, extraction, crawl jobs, results, MCP config,
  API keys, AI providers and settings views.
- SQLite storage with migrations, WAL, jobs, pages, results, cache, settings,
  API keys and encrypted provider secrets.
- SSRF protection with DNS resolution and validation on every redirect hop.
- `robots.txt` fetching, caching and enforcement, plus domain policy, rate
  limiting, crawl budgets, response size caps, timeouts and cancellation.
- Prompt-injection scanning and untrusted-content wrapping for external AI
  calls.
- Optional Playwright rendering bridge for JavaScript-heavy pages.
- Windows x64 prebuilt release asset with SHA-256 checksum.
- GitHub Actions CI, release automation and Dependabot.

### Security

- ChaCha20-Poly1305 encryption for provider API keys.
- SHA-256 hashed API keys with constant-time comparison.
- Private vulnerability reporting and a documented security policy.

### Known limitations

- JavaScript rendering requires the optional Node.js/Playwright bridge.
- `search_website` searches a single fetched page; use `crawl_website` for
  site-wide coverage.
- Semantic extraction quality depends on the configured external provider.
- The project does not bypass authentication, paywalls or CAPTCHAs.

[Unreleased]: https://github.com/Ahaduzzamankhan/aicrawl/compare/v0.0.1-experimental.2...HEAD
[0.0.1-experimental.2]: https://github.com/Ahaduzzamankhan/aicrawl/releases/tag/v0.0.1-experimental.2
[0.0.1-experimental.1]: https://github.com/Ahaduzzamankhan/aicrawl/releases/tag/v0.0.1-experimental.1
