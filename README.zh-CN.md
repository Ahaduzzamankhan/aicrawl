<div align="center">

<img src="public/icon.png" alt="AICrawl logo" width="150">

# AICrawl

**一个可执行文件，让 AI 智能体和人类能够抓取获准的网站并提取结构化数据。**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?include_prereleases&style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Download](https://img.shields.io/badge/download-all%20platforms-0078D6?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-0EA5E9?style=for-the-badge)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-ready-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![PRs](https://img.shields.io/badge/PRs-welcome-EC4899?style=for-the-badge&logo=git&logoColor=white)](CONTRIBUTING.md)

**其他语言：** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[快速开始](#快速开始) · [下载](#下载) · [MCP 服务器](#ai-智能体mcp) · [REST API](#rest-api) · [仪表盘](#仪表盘) · [贡献](CONTRIBUTING.md) · [安全](SECURITY.md)

</div>

一个可执行文件，让 **AI 智能体**和**人类**能够抓取获准的网站并提取结构化数据。

> **实验性版本：** 当前版本位于 [`version.json`](version.json)。不同实验性构建之间的接口和存储可能变化；目前请勿将其用于无人值守的生产抓取。

适用于 **Windows x64、Linux x64、macOS Intel 和 macOS Apple Silicon** 的预编译二进制文件会随每个[版本](https://github.com/Ahaduzzamankhan/aicrawl/releases)发布，并附带 SHA-256 校验和。

一个 `aicrawl` 二进制文件包含：

| 界面 | 使用者 | 使用方式 |
| --- | --- | --- |
| CLI | 人类和脚本 | `aicrawl extract`、`crawl`、`inspect`、`jobs` 等 |
| 仪表盘 | 人类 | 由 `aicrawl start` 在 `http://127.0.0.1:8080` 提供的 React UI |
| REST API | 脚本和集成 | `http://127.0.0.1:8080/api/v1` |
| MCP 服务器 | AI 智能体 | `aicrawl mcp stdio` 或 `http://127.0.0.1:8787/mcp` |

<p align="center">
  <img src="public/flowchart.png" alt="AICrawl architecture flowchart: CLI, MCP, REST API and dashboard feed one Rust engine that fetches websites, extracts structured data, applies security rules and stores results in SQLite" width="100%">
</p>

仪表盘**嵌入在可执行文件中**；没有单独的 Web 服务器，也没有需要分发的资源文件夹。四个界面都调用同一个 `AicrawlEngine`，因此 SSRF 防护、robots.txt 处理、缓存和存储的行为完全一致。

AICrawl **不捆绑任何 AI 模型或 API 密钥**。确定性提取开箱即用。如果你配置外部 OpenAI 兼容端点或本地 Ollama 服务器，就可以使用语义提取；否则 MCP 会返回清理后的页面内容和结构化提示，供调用的智能体进行推理。

## 下载

每个[版本](https://github.com/Ahaduzzamankhan/aicrawl/releases)都附带预编译二进制文件。打开最新版本并下载适合你平台的资源：

| 平台 | 资源 |
| --- | --- |
| Windows x64 | `aicrawl-windows-x64.exe` |
| Linux x64 | `aicrawl-linux-x64` |
| macOS Intel (x64) | `aicrawl-macos-x64` |
| macOS Apple Silicon (arm64) | `aicrawl-macos-arm64` |

每个二进制文件都有对应的 `.sha256` 校验和文件。运行前请先校验：

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

下载后请为 Unix 二进制文件添加执行权限：

```bash
chmod +x aicrawl-linux-x64 aicrawl-macos-arm64
```

这些二进制文件是自包含的：仪表盘、数据库迁移和 SQLite 都已编译进去。只有可选的 JavaScript 渲染需要外部软件。

## 快速开始

```bash
# 1. 构建仪表盘，然后将其嵌入二进制文件
cd frontend && npm ci && npm run build && cd ..
cargo build --release

# 2. 启动 API + 仪表盘 + MCP（单进程）
./target/release/aicrawl start

# 3. 开始使用
./target/release/aicrawl extract https://example.com --query "Extract the page title and main headings"
./target/release/aicrawl inspect https://example.com
./target/release/aicrawl crawl https://example.com --max-pages 20 --max-depth 2 --query "Extract product prices"
```

首次 `start` 会打开 `~/.aicrawl/aicrawl.db`（SQLite），并打印仪表盘、API 和 MCP 的地址。设置 `AICRAWL_DATA_DIR` 或传入 `--data-dir` 可使用其他位置。

```bash
aicrawl start --daemon      # 后台进程，日志位于 <data-dir>/aicrawl.log
aicrawl status
aicrawl stop
```

## AI 智能体（MCP）

### stdio —— 桌面智能体的推荐方式

让智能体指向同一个可执行文件：

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

运行 `aicrawl start`（或 `aicrawl mcp start`），然后使用：

```json
{
  "mcpServers": {
    "aicrawl": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

工具：

- `extract_website_data` — `url`、`query`、可选 `schema`、`output_format`
- `crawl_website` — `url`、`max_pages`、`max_depth`
- `get_page_content` — `url`
- `inspect_website` — `url`
- `search_website` — `url`、`query`
- `get_crawl_result` — `job_id`
- `get_crawl_status` — `job_id`

页面内容被视为**不可信数据**。工具输出会如此标记，并且绝不会遵循网页中发现的指令。

## 人类

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

全局选项：`--no-color`（也会遵循 `NO_COLOR`）、`--config <path>`、`--data-dir <dir>`。

### 仪表盘

在 `aicrawl start` 之后打开 `http://127.0.0.1:8080`：

- **Overview** — 健康状态、任务/结果计数、最近活动
- **New Extraction** — URL、查询、schema、格式、渲染开关
- **Crawl Jobs** — 实时状态、进度、取消、任务详情
- **Results Explorer** — 已存储结果，带 JSON 查看器和复制/下载功能
- **MCP Configuration** — 可直接复制的 stdio/HTTP 配置和工具列表
- **API Keys** — 创建/撤销密钥（仅显示一次，存储哈希值）
- **AI Providers** — 可选的外部提供商，密钥静态加密
- **Settings** — 生效的安全限制、缓存和存储控制

### REST API

所有响应都采用 `{ "success": true, "data": … }` 或 `{ "success": false, "error": { "code", "message", "retryable" } }` 格式。

```bash
curl -s http://127.0.0.1:8080/api/v1/health

curl -s -X POST http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","query":"Extract the main headings","use_ai":false}'

curl -s -X POST http://127.0.0.1:8080/api/v1/crawl \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","max_pages":10,"max_depth":1}'
```

设置 `[api].require_api_key = true`（或绑定到回环地址之外），并发送 `Authorization: Bearer <key>` 或 `X-API-Key: <key>`。密钥以 SHA-256 哈希存储，并且只显示一次。

## 外部 AI（可选）

在**仪表盘 → AI Providers**中或通过 API 配置提供商：

- OpenAI 兼容：基础 URL、模型、API 密钥
- Ollama：基础 URL（`http://127.0.0.1:11434`）、模型、无需密钥

API 密钥使用 ChaCha20-Poly1305 加密。加密密钥来自 `AICRAWL_SECRET_KEY`（base64 编码的 32 字节密钥），或首次在 `<data-dir>/secret.key` 生成，且仅所有者有权限。明文密钥绝不会写入 SQLite、日志或仪表盘。

`aicrawl doctor` 会报告是否配置了提供商；没有提供商时提取是确定性的，并返回警告而不是失败。

## 安全

- SSRF 防护会解析 DNS，并在每次重定向跳转时拒绝回环、私有、链路本地、组播和元数据地址。
- 重定向会手动跟随（最多 5 次），并在每次跳转时重新校验。
- `robots.txt` 会被获取、缓存并默认强制执行。
- 域名允许/拒绝列表、按主机限流、页面/深度上限、实际耗时抓取预算、请求超时和取消。
- 响应正文有大小限制，HTML 会为文本提取进行净化，脚本/样式会被移除。
- 会检测提示注入标记，并在任何外部 AI 调用之前将页面文本包装为不可信数据。
- 可选的 API 密钥认证；恒定时间哈希比较；URL 中不包含密钥。
- 不绕过 CAPTCHA、身份验证或付费墙；没有隐身/规避功能。

## 存储

仅使用 SQLite（`sqlx`、WAL、连接池、预编译语句、迁移、索引、事务）。表：jobs、pages、results、websites、settings、API keys、AI providers、page cache、robots cache。

## 在没有 Visual Studio 的 Windows 上构建

捆绑的 SQLite 需要 C 编译器。如果没有 MSVC Build Tools，请使用 MinGW-w64 的 GNU 目标（本仓库忽略 `.tools/`）：

```bash
# 一次性：将 MinGW-w64 工具链（gcc、dlltool、ar）解压到 .tools/mingw64
export PATH="$PWD/.tools/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
export CARGO_TARGET_DIR=target-gnu
cargo +stable-x86_64-pc-windows-gnu build --release
# 二进制文件：target-gnu/release/aicrawl.exe
```

发布版可执行文件是自包含的：仪表盘、迁移和 SQLite 都已编译进去。只有可选的 JavaScript 渲染需要外部软件。

## 可选的 JavaScript 渲染

默认不渲染 SPA。如需渲染：

```bash
cd browser && npm install && npx playwright install chromium
```

然后在 extract/crawl 时传入 `--render`，或在仪表盘中启用渲染。没有桥接程序时，AICrawl 会返回明确的“rendering unavailable”错误，而不是静默返回空页面。

## 开发

```bash
cargo test --workspace          # 单元 + 集成测试（本地 HTTP 服务器）
cd frontend && npm run dev      # Vite 运行在 :5173，将 /api 代理到 :8080
cargo run -p aicrawl-cli -- doctor
```

仪表盘在编译时嵌入。开发 UI 时可以使用 Vite 开发服务器，或设置 `AICRAWL_DASHBOARD_DIR=frontend/dist`，无需重新编译 Rust 即可从磁盘提供重新构建的版本。

## 发布与版本管理

[`version.json`](version.json) 是发布版本的唯一事实来源。它决定两件事：

- 发布标签（`v` + 版本号，例如 `v0.0.1-experimental.2`）
- 编译进二进制文件的版本，可通过 `aicrawl --version`、`aicrawl version` 和 MCP 服务器查看

发布版本：

1. 更新 `version.json` 以及 `Cargo.toml` 中对应的 `version`，然后合并到 `main`。
2. 等待自动 **CI** 工作流在 Windows、Linux 和 macOS 上通过。
3. 手动运行 **Actions → Release → Run workflow**。该工作流会创建标签、构建四个二进制文件、验证其嵌入版本，并上传它们及 SHA-256 校验和。

包含连字符（`-`）的版本会作为预发布版本发布。

## 限制

- JavaScript 密集型页面需要可选的 Playwright 桥接程序。
- `search_website` 只搜索一个已抓取的页面；如需全站覆盖，请使用 `crawl_website`。
- 语义提取的质量取决于所配置的外部提供商。
- AICrawl 尊重网站政策；不会绕过身份验证、付费墙或 CAPTCHA。

## 社区

- [贡献指南](CONTRIBUTING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [安全政策](SECURITY.md)
- [支持](SUPPORT.md)
- [治理](GOVERNANCE.md)
- [更新日志](CHANGELOG.md)
- [讨论](https://github.com/Ahaduzzamankhan/aicrawl/discussions)
- [报告错误](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml)
- [请求功能](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml)

如果 AICrawl 对你有帮助，欢迎为仓库点亮星标——这能帮助更多人发现这个项目。

## 许可证

MIT
