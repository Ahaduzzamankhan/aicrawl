<div align="center">

<img src="public/icon.png" alt="AICrawl logo" width="150">

# AICrawl

**One executable that lets AI agents and humans crawl permitted websites and extract structured data.**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?include_prereleases&style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Download](https://img.shields.io/badge/download-all%20platforms-0078D6?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-0EA5E9?style=for-the-badge)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-ready-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![PRs](https://img.shields.io/badge/PRs-welcome-EC4899?style=for-the-badge&logo=git&logoColor=white)](CONTRIBUTING.md)

**Read this in:** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[Quick start](#quick-start) · [Download](#download) · [MCP server](#ai-agents-mcp) · [REST API](#rest-api) · [Dashboard](#dashboard) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

</div>

A single executable that lets **AI agents** and **humans** crawl permitted
websites and extract structured data.

> **Experimental release:** the current version lives in
> [`version.json`](version.json). Interfaces and storage may change between
> experimental builds; do not rely on it for unattended production crawling yet.

Prebuilt binaries for **Windows x64, Linux x64, macOS Intel and macOS Apple
Silicon** are attached to every [release](https://github.com/Ahaduzzamankhan/aicrawl/releases),
together with SHA-256 checksums.

One `aicrawl` binary contains:

| Surface | Who uses it | How |
| --- | --- | --- |
| CLI | humans & scripts | `aicrawl extract`, `crawl`, `inspect`, `jobs`, … |
| Dashboard | humans | React UI served by `aicrawl start` at `http://127.0.0.1:8080` |
| REST API | scripts & integrations | `http://127.0.0.1:8080/api/v1` |
| MCP server | AI agents | `aicrawl mcp stdio` or `http://127.0.0.1:8787/mcp` |

<p align="center">
  <img src="public/flowchart.png" alt="AICrawl architecture flowchart: CLI, MCP, REST API and dashboard feed one Rust engine that fetches websites, extracts structured data, applies security rules and stores results in SQLite" width="100%">
</p>

The dashboard is **embedded in the executable**; there is no separate web server
and no asset folder to ship. All four surfaces call the same `AicrawlEngine`, so
SSRF protection, robots.txt handling, caching and storage behave identically.

AICrawl bundles **no AI model and no API keys**. Deterministic extraction works
out of the box. If you configure an external OpenAI-compatible endpoint or a
local Ollama server, semantic extraction becomes available; otherwise MCP
returns the cleaned page content and structured hints for the calling agent to
reason over.

## Download

Prebuilt binaries are attached to every [release](https://github.com/Ahaduzzamankhan/aicrawl/releases).
Open the newest release and download the asset for your platform:

| Platform | Asset |
| --- | --- |
| Windows x64 | `aicrawl-windows-x64.exe` |
| Linux x64 | `aicrawl-linux-x64` |
| macOS Intel (x64) | `aicrawl-macos-x64` |
| macOS Apple Silicon (arm64) | `aicrawl-macos-arm64` |

Each binary has a matching `.sha256` checksum file. Verify it before running:

```bash
# Linux
sha256sum -c aicrawl-linux-x64.sha256

# macOS
shasum -a 256 -c aicrawl-macos-arm64.sha256
```

```powershell
# Windows
Get-FileHash .\aicrawl-windows-x64.exe -Algorithm SHA256
```

Make the Unix binaries executable after downloading:

```bash
chmod +x aicrawl-linux-x64 aicrawl-macos-arm64
```

The binaries are self-contained: dashboard, migrations and SQLite are compiled
in. Only optional JavaScript rendering needs external software.

## Quick start

```bash
# 1. Build the dashboard, then embed it in the binary
cd frontend && npm ci && npm run build && cd ..
cargo build --release

# 2. Start API + dashboard + MCP (one process)
./target/release/aicrawl start

# 3. Use it
./target/release/aicrawl extract https://example.com --query "Extract the page title and main headings"
./target/release/aicrawl inspect https://example.com
./target/release/aicrawl crawl https://example.com --max-pages 20 --max-depth 2 --query "Extract product prices"
```

The first `start` opens `~/.aicrawl/aicrawl.db` (SQLite) and prints the
dashboard, API and MCP URLs. Set `AICRAWL_DATA_DIR` or pass `--data-dir` to use
another location.

```bash
aicrawl start --daemon      # background process, log in <data-dir>/aicrawl.log
aicrawl status
aicrawl stop
```

## AI agents (MCP)

### stdio — recommended for desktop agents

Point the agent at the same executable:

```json
{
  "mcpServers": {
    "aicrawl": {
      "command": "C:\\path\\to\\aicrawl.exe",
      "args": ["mcp", "stdio"]
    }
  }
}
```

### Streamable HTTP

Run `aicrawl start` (or `aicrawl mcp start`) and use:

```json
{
  "mcpServers": {
    "aicrawl": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

Tools:

- `extract_website_data` — `url`, `query`, optional `schema`, `output_format`
- `crawl_website` — `url`, `max_pages`, `max_depth`
- `get_page_content` — `url`
- `inspect_website` — `url`
- `search_website` — `url`, `query`
- `get_crawl_result` — `job_id`
- `get_crawl_status` — `job_id`

Page content is treated as **untrusted data**. Tool output marks it as such and
never follows instructions found in web pages.

## Humans

### CLI

```text
aicrawl start [--api-port 8080] [--mcp-port 8787] [--daemon]
aicrawl stop | status
aicrawl extract <url> --query <text> [--schema <file|json>] [--format json|markdown|text]
                       [--no-ai] [--render] [--no-cache]
aicrawl crawl <url> [--max-pages N] [--max-depth N] [--query <text>] [--render]
aicrawl inspect <url> [--render]
aicrawl mcp start [--port 8787] | aicrawl mcp stdio
aicrawl api start [--port 8080]
aicrawl jobs [--status <status>] [--limit N]
aicrawl results [--job <id>] [--limit N] [--full]
aicrawl config show | set <key> <value> | path
aicrawl doctor [--offline]
aicrawl version
```

Global flags: `--no-color` (also honours `NO_COLOR`), `--config <path>`,
`--data-dir <dir>`.

### Dashboard

Open `http://127.0.0.1:8080` after `aicrawl start`:

- **Overview** — health, job/result counters, recent activity
- **New Extraction** — URL, query, schema, format, render toggle
- **Crawl Jobs** — live status, progress, cancel, job detail
- **Results Explorer** — stored results with JSON viewer and copy/download
- **MCP Configuration** — copy-ready stdio/HTTP configs and tool list
- **API Keys** — create/revoke keys (shown once, stored hashed)
- **AI Providers** — optional external providers, keys encrypted at rest
- **Settings** — effective security limits, cache and storage controls

### REST API

All responses use `{ "success": true, "data": … }` or
`{ "success": false, "error": { "code", "message", "retryable" } }`.

```bash
curl -s http://127.0.0.1:8080/api/v1/health

curl -s -X POST http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","query":"Extract the main headings","use_ai":false}'

curl -s -X POST http://127.0.0.1:8080/api/v1/crawl \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","max_pages":10,"max_depth":1}'
```

Set `[api].require_api_key = true` (or bind beyond loopback) and send
`Authorization: Bearer <key>` or `X-API-Key: <key>`. Keys are stored as SHA-256
hashes and are only displayed once.

## External AI (optional)

Configure a provider in **Dashboard → AI Providers** or via the API:

- OpenAI-compatible: base URL, model, API key
- Ollama: base URL (`http://127.0.0.1:11434`), model, no key

API keys are encrypted with ChaCha20-Poly1305. The encryption key comes from
`AICRAWL_SECRET_KEY` (a base64-encoded 32-byte key) or is generated once at
`<data-dir>/secret.key` with owner-only permissions. Plaintext keys are never
written to SQLite, logs or the dashboard.

`aicrawl doctor` reports whether a provider is configured; extraction without
one is deterministic and returns a warning rather than failing.

## Security

- SSRF protection resolves DNS and rejects loopback, private, link-local,
  multicast and metadata addresses on every redirect hop.
- Redirects are followed manually (max 5) and re-validated each time.
- `robots.txt` is fetched, cached and enforced by default.
- Domain allow/deny lists, per-host rate limiting, page/depth caps, wall-clock
  crawl budgets, request timeouts and cancellation.
- Response bodies are size-capped, HTML is sanitized for text extraction, and
  scripts/styles are stripped.
- Prompt-injection markers are detected and page text is wrapped as untrusted
  data before any external AI call.
- Optional API-key auth; constant-time hash comparison; no keys in URLs.
- No CAPTCHA, authentication or paywall bypass; no stealth/evasion features.

## Storage

SQLite only (`sqlx`, WAL, pooled connections, prepared statements, migrations,
indexes, transactions). Tables: jobs, pages, results, websites, settings, API
keys, AI providers, page cache, robots cache.

## Building on Windows without Visual Studio

The bundled SQLite needs a C compiler. If MSVC Build Tools are unavailable, use
the GNU target with MinGW-w64 (this repository ignores `.tools/`):

```bash
# once: unpack a MinGW-w64 toolchain (gcc, dlltool, ar) into .tools/mingw64
export PATH="$PWD/.tools/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
export CARGO_TARGET_DIR=target-gnu
cargo +stable-x86_64-pc-windows-gnu build --release
# binary: target-gnu/release/aicrawl.exe
```

The release executable is self-contained: dashboard, migrations and SQLite are
compiled in. Only optional JavaScript rendering needs external software.

## Optional JavaScript rendering

SPAs are not rendered by default. To render:

```bash
cd browser && npm install && npx playwright install chromium
```

Then pass `--render` on extract/crawl or enable render in the dashboard. Without
the bridge, AICrawl returns an explicit “rendering unavailable” error instead of
silently returning an empty page.

## Development

```bash
cargo test --workspace          # unit + integration tests (local HTTP server)
cd frontend && npm run dev      # Vite at :5173, proxies /api to :8080
cargo run -p aicrawl-cli -- doctor
```

The dashboard is embedded at compile time. During UI work either use the Vite
dev server or set `AICRAWL_DASHBOARD_DIR=frontend/dist` to serve a rebuild from
disk without recompiling Rust.

## Release and versioning

[`version.json`](version.json) is the single source of truth for the release
version. It drives two things:

- the release tag (`v` + the version, for example `v0.0.1-experimental.2`)
- the version compiled into the binary, shown by `aicrawl --version`,
  `aicrawl version` and the MCP server

To publish a release:

1. Update `version.json` and the matching `version` in `Cargo.toml`, then merge
   to `main`.
2. Wait for the automatic **CI** workflow to pass on Windows, Linux and macOS.
3. Run **Actions → Release → Run workflow** manually. The workflow creates the
   tag, builds all four binaries, verifies their embedded version and uploads
   them with SHA-256 checksums.

Versions containing a hyphen (`-`) are published as prereleases.

## Limitations

- JavaScript-heavy pages require the optional Playwright bridge.
- `search_website` searches one fetched page; use `crawl_website` for site-wide
  coverage.
- Semantic extraction quality depends on the external provider you configure.
- AICrawl respects site policy; it does not bypass authentication, paywalls or
  CAPTCHAs.

## Community

- [Contributing guide](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security policy](SECURITY.md)
- [Support](SUPPORT.md)
- [Governance](GOVERNANCE.md)
- [Changelog](CHANGELOG.md)
- [Discussions](https://github.com/Ahaduzzamankhan/aicrawl/discussions)
- [Report a bug](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml)
- [Request a feature](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml)

If AICrawl is useful to you, consider starring the repository — it helps other
people find the project.

## License

MIT
