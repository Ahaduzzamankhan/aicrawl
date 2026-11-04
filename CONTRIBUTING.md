# Contributing to AICrawl

Thanks for helping improve AICrawl. This project is an **experimental** release
(`0.0.1-experimental.1`), so APIs, CLI flags and the database schema may change
before a stable release.

By participating you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

Useful links: [README](README.md) · [Support](SUPPORT.md) ·
[Governance](GOVERNANCE.md) · [Security](SECURITY.md) ·
[Changelog](CHANGELOG.md).

## Ways to contribute

- Report reproducible bugs, including the URL pattern, command and full error.
- Suggest security or policy improvements.
- Improve extraction coverage for real-world HTML.
- Improve the dashboard, documentation or tests.
- Send pull requests that keep the single-binary design intact.

Please **do not** open issues asking for CAPTCHA solving, paywall bypass,
authentication circumvention or stealth crawling. AICrawl deliberately respects
site policy and `robots.txt`.

## Development setup

Requirements:

- Rust (stable; 1.88 or newer)
- Node.js 20+ and npm
- A C compiler for bundled SQLite:
  - Windows: Visual Studio Build Tools with “Desktop development with C++”, or
    MinGW-w64 with the GNU Rust target
  - Linux/macOS: `build-essential` / Xcode command line tools

```bash
# Dashboard (must be built before compiling the binary, because it is embedded)
cd frontend
npm ci
npm run build
cd ..

# Rust
cargo build
cargo test --workspace

# Run the CLI
cargo run -p aicrawl-cli -- doctor --offline

# Dashboard dev server (proxies /api to :8080)
cd frontend && npm run dev
```

### Windows without MSVC

Use the GNU target plus MinGW-w64:

```bash
export PATH="/path/to/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu build --release
```

Do not commit a machine-specific toolchain path or a vendored compiler.

## Before opening a pull request

1. `cargo test --workspace` passes.
2. `cd frontend && npm run typecheck && npm run build` passes.
3. `cargo run -p aicrawl-cli -- doctor --offline` reports no blocking issues.
4. New behavior has a test when it touches fetching, extraction, storage or API
   routing.
5. No secrets, database files, `frontend/dist`, `.tools/` or build logs are
   committed.
6. The dashboard still works with the embedded assets (`aicrawl start`).

Keep changes focused. Explain the “why” in the PR description, not just the
“what”. Screenshots help for dashboard changes.

## Continuous integration

Every push to `main` and every pull request runs the [CI workflow](.github/workflows/ci.yml):

1. `npm ci`, `npm run typecheck` and `npm run build` in `frontend`.
2. `cargo fmt --all -- --check`.
3. `cargo clippy --workspace --all-targets`.
4. `cargo test --workspace`.
5. A release-profile build and `aicrawl version` smoke test.

Pushing a `v*` tag runs the [release workflow](.github/workflows/release.yml),
which rebuilds the Windows x64 binary, writes a SHA-256 checksum and publishes
the GitHub release. Do not attach binaries manually; let the workflow do it.

Maintainers may ask for a green CI run before merging. If a check fails, run the
same command locally and include the result in the PR.

## Code layout

| Path | Purpose |
| --- | --- |
| `crates/aicrawl-core/src/engine` | Shared facade used by every surface |
| `crates/aicrawl-core/src/fetcher` | SSRF policy, robots.txt, redirects, cache, rendering |
| `crates/aicrawl-core/src/extractor` | Deterministic extraction and external-AI prompting |
| `crates/aicrawl-core/src/crawler` | Bounded BFS crawler |
| `crates/aicrawl-core/src/storage` | SQLite schema access (sqlx) |
| `crates/aicrawl-core/src/api` | REST API and embedded dashboard host |
| `crates/aicrawl-core/src/mcp` | MCP tools and transports |
| `crates/aicrawl-cli` | `aicrawl` executable and commands |
| `frontend` | React dashboard (Vite, Tailwind, shadcn-style UI) |

There is one engine. CLI, REST, MCP and dashboard must all call it rather than
reimplementing fetch or extraction logic.

## Commit and PR style

- Use short imperative commit subjects, for example `Fix robots.txt cache key`.
- Keep unrelated formatting changes out of feature PRs.
- Reference issues with `Fixes #123` when applicable.
- Maintainers may ask for tests or a smaller scope before merging.

## Security issues

Do not open a public issue for a vulnerability. Follow [SECURITY.md](SECURITY.md)
and use GitHub’s **Report a vulnerability** flow under the repository’s Security
tab. Please include reproduction steps, affected versions and a minimal proof of
concept. Do not include secrets or private crawled data.

## License

By contributing you agree that your contribution is licensed under the
[MIT License](LICENSE).
