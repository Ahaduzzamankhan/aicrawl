# Changelog

All notable changes to AICrawl are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Pre-`1.0.0` experimental builds may contain breaking changes.

## [Unreleased]

### Planned

- Broader extraction coverage and regression fixtures.
- Packaging and signed release artifacts.
- Additional documentation and examples for agent integrations.

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

[Unreleased]: https://github.com/Ahaduzzamankhan/aicrawl/compare/v0.0.1-experimental.1...HEAD
[0.0.1-experimental.1]: https://github.com/Ahaduzzamankhan/aicrawl/releases/tag/v0.0.1-experimental.1
