<div align="center">

<img src="public/icon.png" alt="AICrawl logo" width="150">

# AICrawl

**Deterministic, JSON-first web data extraction — one Rust binary for the CLI, REST API, MCP and dashboard.**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-10%20tools-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![M8ven Score](https://m8ven.ai/badge/mcp/ahaduzzamankhan-aicrawl-1vyaeb?v=5370263b06ca8a087a3adde05ceaacc0)](https://m8ven.ai/mcp/ahaduzzamankhan-aicrawl-1vyaeb?s=readme)

**Read this in:** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[Install](#install-one-line) · [Quick start](#quick-start) · [Request schema](#request-schema) · [CLI](#cli) · [REST API](#rest-api) · [MCP](#mcp) · [Dashboard](#dashboard) · [Security](#security) · [Contributing](CONTRIBUTING.md)

</div>

Send a structured JSON request; get structured, validated JSON back. AICrawl has
no model in the request path, so results are fast, predictable and reproducible —
no API keys, no prompts, no provider accounts.

```bash
curl -s http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com/products","selectors":{"products":{"type":"list","selector":".product-card","fields":{"name":{"selector":".product-title"},"price":{"selector":".price","type":"number"}}}}}'
```

```json
{
  "success": true,
  "data": { "products": [{ "name": "Widget", "price": 19.99 }] },
  "meta": { "url": "https://example.com/products", "pages_processed": 1, "items_extracted": 1, "duration_ms": 118, "projection": "rules" },
  "errors": []
}
```

One `aicrawl` binary provides four interfaces over the same engine:

| Surface | Command | Typical use |
| --- | --- | --- |
| CLI | `aicrawl extract --input request.json` | scripts, cron, CI |
| REST API | `aicrawl api` → `POST /api/v1/extract` | services and integrations |
| MCP | `aicrawl mcp` (stdio) or `--http` | AI agents with strict tool schemas |
| Dashboard | `aicrawl api` → `http://127.0.0.1:8080/` | humans writing and testing requests |

## Install (one line)

Downloads the latest release for your platform, verifies its SHA-256 checksum, installs it and adds it to `PATH` (`%LOCALAPPDATA%\AICrawl\bin` on Windows, `~/.local/bin` on macOS and Linux).

**PowerShell 7 (Windows, macOS, Linux)**

```powershell
$ProgressPreference='SilentlyContinue';[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12;$b='https://github.com/Ahaduzzamankhan/aicrawl/releases/latest/download';$win=$env:OS -eq 'Windows_NT';if($win){$f='aicrawl-windows-x64.exe';$x='aicrawl.exe';$d=Join-Path $env:LOCALAPPDATA 'AICrawl\bin'}else{$s=(& uname -s);$m=(& uname -m);if($s -eq 'Darwin' -and $m -eq 'arm64'){$f='aicrawl-macos-arm64'}elseif($s -eq 'Darwin'){$f='aicrawl-macos-x64'}elseif($s -eq 'Linux' -and $m -eq 'x86_64'){$f='aicrawl-linux-x64'}else{throw "Unsupported platform: $s $m"};$x='aicrawl';$d=Join-Path $HOME '.local/bin'};$null=New-Item -ItemType Directory -Force $d;$null=Invoke-WebRequest "$b/$f" -OutFile (Join-Path $d $x) -UseBasicParsing;$null=Invoke-WebRequest "$b/$f.sha256" -OutFile (Join-Path $d "$x.sha256") -UseBasicParsing;if((Get-Content (Join-Path $d "$x.sha256") -TotalCount 1).Split()[0] -ne (Get-FileHash (Join-Path $d $x) -Algorithm SHA256).Hash.ToLower()){throw 'AICrawl checksum mismatch'};if($win){$p=[Environment]::GetEnvironmentVariable('Path','User');if(-not $p){$p=''};if(($p -split ';') -notcontains $d){[Environment]::SetEnvironmentVariable('Path',($p.TrimEnd(';')+';'+$d).Trim(';'),'User')};$sep=';'}else{chmod +x (Join-Path $d $x);$rc=if($env:SHELL -like '*zsh'){Join-Path $HOME '.zshrc'}elseif($env:SHELL -like '*bash'){Join-Path $HOME '.bashrc'}else{Join-Path $HOME '.profile'};if(-not (Test-Path $rc)){New-Item -ItemType File -Force $rc|Out-Null};if(-not (Select-String -Path $rc -SimpleMatch $d -Quiet)){Add-Content $rc "`nexport PATH=`"$d`:`$PATH`""};$sep=':'};$env:Path=$d+$sep+$env:Path;& (Join-Path $d $x) --version
```

**Windows CMD**

```bat
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ProgressPreference='SilentlyContinue';[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12;$d=Join-Path $env:LOCALAPPDATA 'AICrawl\bin';$u='https://github.com/Ahaduzzamankhan/aicrawl/releases/latest/download/aicrawl-windows-x64.exe';$null=New-Item -ItemType Directory -Force $d;$null=Invoke-WebRequest $u -OutFile (Join-Path $d 'aicrawl.exe') -UseBasicParsing;$null=Invoke-WebRequest ($u+'.sha256') -OutFile (Join-Path $d 'aicrawl.exe.sha256') -UseBasicParsing;if((Get-Content (Join-Path $d 'aicrawl.exe.sha256') -TotalCount 1).Split()[0] -ne (Get-FileHash (Join-Path $d 'aicrawl.exe') -Algorithm SHA256).Hash.ToLower()){throw 'AICrawl checksum mismatch'};$p=[Environment]::GetEnvironmentVariable('Path','User');if(-not $p){$p=''};if(($p -split ';') -notcontains $d){[Environment]::SetEnvironmentVariable('Path',($p.TrimEnd(';')+';'+$d).Trim(';'),'User')};$env:Path=$d+';'+$env:Path" & set "PATH=%PATH%;%LOCALAPPDATA%\AICrawl\bin" & aicrawl --version
```

**macOS / Linux**

```sh
_aicrawl_install(){ local d="$HOME/.local/bin";(b="https://github.com/Ahaduzzamankhan/aicrawl/releases/latest/download";case "$(uname -s):$(uname -m)" in Darwin:arm64)f=aicrawl-macos-arm64;;Darwin:x86_64)f=aicrawl-macos-x64;;Linux:x86_64)f=aicrawl-linux-x64;;*)echo "Unsupported platform: $(uname -s) $(uname -m)" >&2;exit 1;;esac;t=$(mktemp -d)||exit 1;trap 'rm -rf "$t"' EXIT;curl -fsSL "$b/$f" -o "$t/$f"||exit 1;curl -fsSL "$b/$f.sha256" -o "$t/$f.sha256"||exit 1;(cd "$t"&&if command -v sha256sum >/dev/null 2>&1;then sha256sum -c "$f.sha256";else shasum -a 256 -c "$f.sha256";fi)||exit 1;mkdir -p "$d"||exit 1;cp "$t/$f" "$d/aicrawl"||exit 1;chmod +x "$d/aicrawl"||exit 1;rc="$HOME/.profile";case "${SHELL:-}" in *zsh)rc="$HOME/.zshrc";;*bash)rc="$HOME/.bashrc";;esac;touch "$rc";grep -qsF "$d" "$rc"||printf '\nexport PATH="%s:$PATH"\n' "$d">>"$rc")||return 1;export PATH="$d:$PATH";"$d/aicrawl" --version;};_aicrawl_install;unset -f _aicrawl_install
```

`PATH` is also updated in the current shell. If a newly opened Windows terminal still cannot find `aicrawl`, sign out and back in once.

## Quick start

### Build from source

```bash
git clone https://github.com/Ahaduzzamankhan/aicrawl
cd aicrawl
cd frontend && npm ci && npm run build && cd ..
cargo build --release
./target/release/aicrawl --version
```

The dashboard is embedded at compile time; `npm run build` must run before the
Rust build if you want the UI inside the binary.

### First extraction

```bash
# Validate without touching the network
aicrawl validate --input examples/extract-products.json

# Run it and print JSON to stdout
aicrawl extract --input examples/extract-products.json

# Pipe a request in and a result out
cat examples/extract-products.json | aicrawl extract --stdin --compact > products.json
```

## Request schema

The contract is versioned (`"version": "1"`) and documented in
[`docs/request-schema.md`](docs/request-schema.md), with a JSON Schema at
[`shared/schemas/extract-request.schema.json`](shared/schemas/extract-request.schema.json).

```json
{
  "version": "1",
  "url": "https://example.com/products",
  "action": "extract",
  "render": "auto",
  "selectors": {
    "products": {
      "type": "list",
      "selector": ".product-card",
      "fields": {
        "name": { "selector": ".product-title", "required": true },
        "price": { "selector": ".price", "type": "number" },
        "link": { "selector": "a", "type": "url" }
      }
    }
  },
  "pagination": { "enabled": true, "next_selector": "a.next", "max_pages": 5 },
  "deduplicate": { "enabled": true, "keys": ["name"] }
}
```

Highlights:

- **Selectors**: CSS everywhere, plus a documented XPath subset
  (`/html/body/div`, `//h2`, `[@class='card']`, `[1]`, `[last()]`, `contains(...)`).
- **Types**: `text`, `html` (sanitized), `attribute`, `url`, `number`, `integer`,
  `boolean`, `list`, `object`, `json_ld`, `meta`.
- **Structured data**: JSON-LD, Open Graph, `<meta>` tags, headings, tables and
  absolute URL resolution.
- **Failures are explicit**: `required` fields fail with `FIELD_REQUIRED`;
  optional misses become `null`/`default` and are listed in `errors`.
- **Validation**: pass a JSON Schema in `schema` and non-conforming payloads are
  rejected with `SCHEMA_VALIDATION_FAILED`.

Running a request with no `selectors` returns a `document` summary (title,
metadata, Open Graph, JSON-LD, headings, tables, links, text).

## CLI

| Command | Purpose |
| --- | --- |
| `aicrawl extract --input req.json \| --stdin \| --url <url>` | Extract one page. |
| `aicrawl crawl --input crawl.json` | Crawl within page/depth limits. |
| `aicrawl batch --input urls.json` | Apply one request to many URLs. |
| `aicrawl map https://example.com` | Discover reachable URLs. |
| `aicrawl validate --input req.json` | Check a request without network access. |
| `aicrawl inspect https://example.com` | Structural report (metadata, JSON-LD, headings). |
| `aicrawl jobs [--status failed]` | List stored jobs as JSON. |
| `aicrawl results <job-id> [--format jsonl]` | Export a job's dataset. |
| `aicrawl api [--port 8080]` | Serve REST + dashboard. |
| `aicrawl mcp [--http]` | Serve MCP over stdio or streamable HTTP. |
| `aicrawl config show \| path \| init` | Inspect or create configuration. |
| `aicrawl doctor [--json]` | Check database, assets, renderer, policy. |

Machine-readable commands print JSON on **stdout** and diagnostics on **stderr**.
Exit codes: `0` success, `2` invalid request or required-field failure, `1` other
errors. `--no-color` (or `NO_COLOR`) disables ANSI output.

## REST API

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/v1/extract` | Extract one page. |
| POST | `/api/v1/crawl` | Crawl and project each page. |
| POST | `/api/v1/batch` | Apply one request to `urls`. |
| POST | `/api/v1/map` | Discover URLs. |
| POST | `/api/v1/validate` | Validate a request. |
| GET | `/api/v1/jobs` | List jobs (`status`, `limit`, `offset`). |
| GET | `/api/v1/jobs/{id}` | Job with pages and results. |
| GET | `/api/v1/jobs/{id}/results` | Stored payloads for a job. |
| POST | `/api/v1/jobs/{id}/cancel` | Cancel a running crawl. |
| GET | `/api/v1/results` \| `/results/{id}` | Browse stored results. |
| POST | `/api/v1/export` | Export a dataset (`json` or `jsonl`). |
| GET | `/api/v1/health` \| `/stats` \| `/settings` \| `/schema` | Runtime introspection. |
| GET/POST/DELETE | `/api/v1/api-keys` | Manage API keys (hashes only). |

The action is implied by the endpoint; everything else is the same request
object. Errors use the shared envelope and appropriate HTTP status codes
(`400`, `403`, `404`, `422`, `429`, `502`, `503`, `504`).

Set `[api].require_api_key = true` to require `Authorization: Bearer ac_live_…`
or `X-API-Key`. Create keys with `POST /api/v1/api-keys`; only a SHA-256 hash is
stored.

## MCP

Ten task-oriented tools, each with a strict JSON schema:

`extract_structured_data`, `crawl_website`, `map_website`, `inspect_website`,
`batch_extract`, `search_pages`, `compare_pages`, `get_crawl_status`,
`get_crawl_result`, `export_dataset`.

```json
{
  "mcpServers": {
    "aicrawl": {
      "command": "/absolute/path/to/aicrawl",
      "args": ["mcp"]
    }
  }
}
```

For HTTP transport run `aicrawl mcp --http` and point clients at
`http://127.0.0.1:8787/mcp`.

Tool arguments are structured requests, not natural-language instructions.
Crawled content is **untrusted data**: the engine flags prompt-injection patterns
in extracted text as warnings and never follows instructions found in pages.

## Dashboard

`aicrawl api` serves the embedded React dashboard at `http://127.0.0.1:8080/`.
The primary workflow is the **Request Workbench**: edit a JSON request, validate
it, run it, and inspect/copy/download the JSON response. Saved templates and
recent requests live in the browser; jobs, results and crawl progress come from
SQLite. Natural-language prompting is not part of the workflow.

For dashboard development:

```bash
cd frontend
npm ci
npm run dev     # http://localhost:5173, proxies /api/v1 to :8080
```

## Crawling

- Bounded breadth-first crawl with `max_pages` (≤500) and `max_depth` (≤5).
- Optional `sitemap.xml` discovery and `incremental` content-hash comparison.
- One stored result per page, exportable as a dataset.
- Cancellation via `POST /api/v1/jobs/{id}/cancel` (or `aicrawl jobs`).
- Per-host rate limiting, redirect validation, body-size caps and the SQLite page
  cache apply to every interface.

## Security

- SSRF protection: loopback, RFC1918, link-local (including cloud metadata),
  CGNAT, unique-local IPv6 and IPv4-mapped addresses are refused after DNS
  resolution; private networks require an explicit opt-in.
- robots.txt is enforced by default (`crawler.respect_robots_txt`).
- Domain allow/block lists, redirect limits and per-host rate limits.
- No authentication, CAPTCHA, paywall or access-control bypass — ever.
- Extracted page text is data. Injection patterns are reported, not obeyed.

See [SECURITY.md](SECURITY.md) for reporting.

## Architecture

```
crates/aicrawl-core/src/
  request/      # versioned JSON contract + validation
  pipeline/     # fetch → parse → select → extract → validate
  selectors/    # CSS, XPath subset, sanitized HTML
  extraction/   # rules, JSON-LD, document summary, JSON Schema
  transform/    # whitespace, numbers, URLs, type conversion
  dedup.rs      # duplicate removal
  crawl/ map/   # crawl engine, sitemap and link discovery
  fetcher/      # HTTP, robots, cache, rate limits
  browser/      # optional Playwright bridge
  security/     # SSRF, domain policy, API key hashing
  output/       # shared success/error envelope
  engine/       # orchestration, jobs, storage wiring
  api/ mcp/     # REST and MCP transports
crates/aicrawl-cli/  # JSON-first commands
frontend/            # React dashboard (embedded)
```

Rust core, SQLite storage, React UI. One engine, one response contract.

## Configuration

`~/.aicrawl/config.toml` (see [`aicrawl.example.toml`](aicrawl.example.toml)):
`[server]`, `[crawler]`, `[security]`, `[extract]`, `[storage]`, `[cache]`,
`[api]`. Environment overrides include `AICRAWL_DATA_DIR`, `AICRAWL_API_PORT`,
`AICRAWL_ALLOW_PRIVATE_NETWORKS`, `AICRAWL_ALLOWED_DOMAINS`,
`AICRAWL_BLOCKED_DOMAINS` and `AICRAWL_MAX_ITEMS`.

Playwright is optional. Install it in `browser/` (`npm install`) to enable
`"render": "auto"`/`"on"`; HTTP-only extraction needs nothing beyond the binary.

## Testing

```bash
cargo test --workspace
cd frontend && npm run typecheck && npm run build
```

Integration tests run against a local fixture site (no external network) and
cover CSS/XPath extraction, nested lists, required/optional misses, pagination,
dedupe, schema validation, robots.txt, SSRF refusal, crawling, the REST
envelope and the MCP catalogue.

## Upgrading from 0.0.1-experimental

Version 1.0.0 is a breaking release:

- Natural-language `query` fields are gone; send structured `selectors` instead.
- The built-in AI provider registry was removed (migration `0002` drops
  `ai_providers`); no model or API key is used or stored.
- `aicrawl start/stop/status` were replaced by foreground `aicrawl api` and
  `aicrawl mcp` commands.
- CLI extraction commands now take `--input`/`--stdin` JSON requests.

## License

MIT — see [LICENSE](LICENSE).

## Star History

<a href="https://www.star-history.com/?repos=ahaduzzamankhan%2Faicrawl&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
 </picture>
</a>

---

## Contributors

Thanks to everyone who contributes to AICrawl.

<a href="https://github.com/Ahaduzzamankhan/aicrawl/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=Ahaduzzamankhan/aicrawl&max=400&columns=18" alt="AICrawl contributors" />
</a>
