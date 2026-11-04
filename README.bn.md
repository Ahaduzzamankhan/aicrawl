> **Note:** this translation predates v1.0.0 (JSON-first requests, no AI provider).
> See [README.md](README.md) and [docs/request-schema.md](docs/request-schema.md) for current behaviour.

<div align="center">

<img src="public/icon.png" alt="AICrawl logo" width="150">

# AICrawl

**একটি এক্সিকিউটেবল যা AI এজেন্ট এবং মানুষকে অনুমোদিত ওয়েবসাইট ক্রল করে কাঠামোবদ্ধ ডেটা বের করতে দেয়।**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?include_prereleases&style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Download](https://img.shields.io/badge/download-all%20platforms-0078D6?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-0EA5E9?style=for-the-badge)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-ready-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![PRs](https://img.shields.io/badge/PRs-welcome-EC4899?style=for-the-badge&logo=git&logoColor=white)](CONTRIBUTING.md)

**এই README পড়ুন:** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[দ্রুত শুরু](#দ্রুত-শুরু) · [ডাউনলোড](#ডাউনলোড) · [MCP সার্ভার](#ai-এজেন্ট-mcp) · [REST API](#rest-api) · [ড্যাশবোর্ড](#ড্যাশবোর্ড) · [অবদান](CONTRIBUTING.md) · [নিরাপত্তা](SECURITY.md)

</div>

একটি এক্সিকিউটেবল যা **AI এজেন্ট** এবং **মানুষ**কে অনুমোদিত ওয়েবসাইট ক্রল করতে এবং কাঠামোবদ্ধ ডেটা বের করতে দেয়।

> **পরীক্ষামূলক রিলিজ:** বর্তমান সংস্করণ [`version.json`](version.json)-এ আছে। পরীক্ষামূলক বিল্ডগুলোর মধ্যে ইন্টারফেস ও স্টোরেজ বদলাতে পারে; এখনো এটিকে তদারকিহীন প্রোডাকশন ক্রলের জন্য ব্যবহার করবেন না।

**Windows x64, Linux x64, macOS Intel এবং macOS Apple Silicon**-এর জন্য প্রস্তুত বাইনারি প্রতিটি [রিলিজ](https://github.com/Ahaduzzamankhan/aicrawl/releases)-এর সাথে SHA-256 চেকসামসহ দেওয়া হয়।

একটি `aicrawl` বাইনারিতে আছে:

| পৃষ্ঠতল | কারা ব্যবহার করে | কীভাবে |
| --- | --- | --- |
| CLI | মানুষ ও স্ক্রিপ্ট | `aicrawl extract`, `crawl`, `inspect`, `jobs`, … |
| ড্যাশবোর্ড | মানুষ | `aicrawl start`-এ `http://127.0.0.1:8080`-এ পরিবেশিত React UI |
| REST API | স্ক্রিপ্ট ও ইন্টিগ্রেশন | `http://127.0.0.1:8080/api/v1` |
| MCP সার্ভার | AI এজেন্ট | `aicrawl mcp stdio` বা `http://127.0.0.1:8787/mcp` |

<p align="center">
  <img src="public/flowchart.png" alt="AICrawl architecture flowchart: CLI, MCP, REST API and dashboard feed one Rust engine that fetches websites, extracts structured data, applies security rules and stores results in SQLite" width="100%">
</p>

ড্যাশবোর্ড **এক্সিকিউটেবলের ভিতরে এমবেড করা**; আলাদা ওয়েব সার্ভার বা পাঠানোর জন্য কোনো অ্যাসেট ফোল্ডার নেই। চারটি পৃষ্ঠতলই একই `AicrawlEngine` ব্যবহার করে, তাই SSRF সুরক্ষা, robots.txt হ্যান্ডলিং, ক্যাশিং ও স্টোরেজ একইভাবে কাজ করে।

AICrawl **কোনো AI মডেল বা API কী** সাথে দেয় না। ডিটারমিনিস্টিক এক্সট্রাকশন কোনো সেটআপ ছাড়াই কাজ করে। যদি আপনি একটি বাহ্যিক OpenAI-সামঞ্জস্যপূর্ণ এন্ডপয়েন্ট বা স্থানীয় Ollama সার্ভার কনফিগার করেন, তাহলে সেমান্টিক এক্সট্রাকশন পাওয়া যায়; অন্যথায় MCP কলকারী এজেন্টকে যুক্তি করার জন্য পরিষ্কার পেজ কনটেন্ট ও কাঠামোবদ্ধ সংকেত ফেরত দেয়।

## ডাউনলোড

প্রতিটি [রিলিজ](https://github.com/Ahaduzzamankhan/aicrawl/releases)-এ প্রস্তুত বাইনারি সংযুক্ত থাকে। নতুন রিলিজ খুলে আপনার প্ল্যাটফর্মের অ্যাসেট ডাউনলোড করুন:

| প্ল্যাটফর্ম | অ্যাসেট |
| --- | --- |
| Windows x64 | `aicrawl-windows-x64.exe` |
| Linux x64 | `aicrawl-linux-x64` |
| macOS Intel (x64) | `aicrawl-macos-x64` |
| macOS Apple Silicon (arm64) | `aicrawl-macos-arm64` |

প্রতিটি বাইনারির সাথে একটি মিলিত `.sha256` চেকসাম ফাইল থাকে। চালানোর আগে যাচাই করুন:

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

ডাউনলোডের পরে Unix বাইনারিগুলোকে এক্সিকিউটেবল করুন:

```bash
chmod +x aicrawl-linux-x64 aicrawl-macos-arm64
```

বাইনারিগুলো স্বয়ংসম্পূর্ণ: ড্যাশবোর্ড, মাইগ্রেশন ও SQLite ভিতরে কম্পাইল করা থাকে। শুধু ঐচ্ছিক JavaScript রেন্ডারিংয়ের জন্য বাহ্যিক সফটওয়্যার লাগে।

## দ্রুত শুরু

```bash
# 1. ড্যাশবোর্ড বানান, তারপর এটি বাইনারিতে এমবেড করুন
cd frontend && npm ci && npm run build && cd ..
cargo build --release

# 2. API + ড্যাশবোর্ড + MCP চালু করুন (এক প্রসেস)
./target/release/aicrawl start

# 3. ব্যবহার করুন
./target/release/aicrawl extract https://example.com --query "Extract the page title and main headings"
./target/release/aicrawl inspect https://example.com
./target/release/aicrawl crawl https://example.com --max-pages 20 --max-depth 2 --query "Extract product prices"
```

প্রথম `start` `~/.aicrawl/aicrawl.db` (SQLite) খোলে এবং ড্যাশবোর্ড, API ও MCP URL ছাপে। অন্য জায়গা ব্যবহার করতে `AICRAWL_DATA_DIR` সেট করুন বা `--data-dir` দিন।

```bash
aicrawl start --daemon      # ব্যাকগ্রাউন্ড প্রসেস, লগ <data-dir>/aicrawl.log-এ
aicrawl status
aicrawl stop
```

## AI এজেন্ট (MCP)

### stdio — ডেস্কটপ এজেন্টের জন্য প্রস্তাবিত

এজেন্টকে একই এক্সিকিউটেবলের দিকে নির্দেশ করুন:

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

`aicrawl start` (বা `aicrawl mcp start`) চালান এবং ব্যবহার করুন:

```json
{
  "mcpServers": {
    "aicrawl": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

টুলসমূহ:

- `extract_website_data` — `url`, `query`, ঐচ্ছিক `schema`, `output_format`
- `crawl_website` — `url`, `max_pages`, `max_depth`
- `get_page_content` — `url`
- `inspect_website` — `url`
- `search_website` — `url`, `query`
- `get_crawl_result` — `job_id`
- `get_crawl_status` — `job_id`

পেজ কনটেন্টকে **অবিশ্বস্ত ডেটা** হিসেবে ধরা হয়। টুল আউটপুট এটিকে তেমনভাবে চিহ্নিত করে এবং ওয়েব পেজে পাওয়া নির্দেশ কখনো অনুসরণ করে না।

## মানুষ

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

গ্লোবাল ফ্ল্যাগ: `--no-color` (`NO_COLOR`-ও মানে), `--config <path>`, `--data-dir <dir>`।

### ড্যাশবোর্ড

`aicrawl start`-এর পরে `http://127.0.0.1:8080` খুলুন:

- **Overview** — স্বাস্থ্য, জব/ফলাফল কাউন্টার, সাম্প্রতিক কার্যকলাপ
- **New Extraction** — URL, query, schema, format, render টগল
- **Crawl Jobs** — লাইভ স্ট্যাটাস, অগ্রগতি, বাতিল, জবের বিস্তারিত
- **Results Explorer** — JSON ভিউয়ার ও কপি/ডাউনলোডসহ সংরক্ষিত ফলাফল
- **MCP Configuration** — কপি করার জন্য প্রস্তুত stdio/HTTP কনফিগ ও টুল তালিকা
- **API Keys** — কী তৈরি/বাতিল (একবার দেখানো হয়, হ্যাশ করে সংরক্ষণ)
- **AI Providers** — ঐচ্ছিক বাহ্যিক প্রোভাইডার, কী-গুলো রেস্টে এনক্রিপ্টেড
- **Settings** — কার্যকর নিরাপত্তা সীমা, ক্যাশ ও স্টোরেজ নিয়ন্ত্রণ

### REST API

সব উত্তর `{ "success": true, "data": … }` অথবা `{ "success": false, "error": { "code", "message", "retryable" } }` আকারে।

```bash
curl -s http://127.0.0.1:8080/api/v1/health

curl -s -X POST http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","query":"Extract the main headings","use_ai":false}'

curl -s -X POST http://127.0.0.1:8080/api/v1/crawl \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","max_pages":10,"max_depth":1}'
```

`[api].require_api_key = true` সেট করুন (বা লুপব্যাকের বাইরে বাইন্ড করুন) এবং `Authorization: Bearer <key>` বা `X-API-Key: <key>` পাঠান। কী-গুলো SHA-256 হ্যাশ হিসেবে সংরক্ষিত হয় এবং শুধু একবার দেখানো হয়।

## বাহ্যিক AI (ঐচ্ছিক)

**ড্যাশবোর্ড → AI Providers**-এ বা API-এর মাধ্যমে একটি প্রোভাইডার কনফিগার করুন:

- OpenAI-সামঞ্জস্যপূর্ণ: base URL, মডেল, API কী
- Ollama: base URL (`http://127.0.0.1:11434`), মডেল, কী নেই

API কী ChaCha20-Poly1305 দিয়ে এনক্রিপ্ট করা হয়। এনক্রিপশন কী `AICRAWL_SECRET_KEY` (base64-এনকোড করা 32-বাইট কী) থেকে আসে অথবা একবার `<data-dir>/secret.key`-এ শুধু মালিকের অনুমতিসহ তৈরি হয়। প্লেইনটেক্সট কী কখনো SQLite, লগ বা ড্যাশবোর্ডে লেখা হয় না।

`aicrawl doctor` জানায় কোনো প্রোভাইডার কনফিগার করা আছে কি না; এটি ছাড়া এক্সট্রাকশন ডিটারমিনিস্টিক এবং ব্যর্থ হওয়ার বদলে একটি সতর্কতা ফেরত দেয়।

## নিরাপত্তা

- SSRF সুরক্ষা DNS রিজলভ করে এবং প্রতিটি রিডাইরেক্ট হপে লুপব্যাক, প্রাইভেট, লিংক-লোকাল, মাল্টিকাস্ট ও মেটাডেটা ঠিকানা প্রত্যাখ্যান করে।
- রিডাইরেক্টগুলো ম্যানুয়ালি অনুসরণ করা হয় (সর্বোচ্চ ৫) এবং প্রতিবার পুনরায় যাচাই করা হয়।
- `robots.txt` আনা, ক্যাশ ও ডিফল্টভাবে প্রয়োগ করা হয়।
- ডোমেইন অনুমোদন/ব্লক তালিকা, প্রতি-হোস্ট রেট লিমিট, পেজ/গভীরতা সীমা, ওয়াল-ক্লক ক্রল বাজেট, রিকোয়েস্ট টাইমআউট ও বাতিলকরণ।
- রেসপন্স বডির আকার সীমিত, টেক্সট এক্সট্রাকশনের জন্য HTML স্যানিটাইজ করা হয়, স্ক্রিপ্ট/স্টাইল সরানো হয়।
- প্রম্পট-ইনজেকশন চিহ্ন শনাক্ত করা হয় এবং কোনো বাহ্যিক AI কলে যাওয়ার আগে পেজ টেক্সট অবিশ্বস্ত ডেটা হিসেবে মোড়ানো হয়।
- ঐচ্ছিক API-কী প্রমাণীকরণ; ধ্রুব-সময় হ্যাশ তুলনা; URL-এ কোনো কী নেই।
- কোনো CAPTCHA, প্রমাণীকরণ বা পেওয়াল বাইপাস নেই; কোনো স্টেলথ/এভেশন বৈশিষ্ট্য নেই।

## স্টোরেজ

শুধু SQLite (`sqlx`, WAL, পুল করা কানেকশন, প্রস্তুত স্টেটমেন্ট, মাইগ্রেশন, ইনডেক্স, ট্রানজ্যাকশন)। টেবিল: jobs, pages, results, websites, settings, API keys, AI providers, page cache, robots cache।

## Visual Studio ছাড়া Windows-এ বিল্ড

বান্ডল করা SQLite-এর জন্য একটি C কম্পাইলার দরকার। যদি MSVC Build Tools না থাকে, MinGW-w64-সহ GNU টার্গেট ব্যবহার করুন (এই রিপোজিটরি `.tools/` উপেক্ষা করে):

```bash
# একবার: .tools/mingw64-এ একটি MinGW-w64 টুলচেইন আনপ্যাক করুন (gcc, dlltool, ar)
export PATH="$PWD/.tools/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
export CARGO_TARGET_DIR=target-gnu
cargo +stable-x86_64-pc-windows-gnu build --release
# বাইনারি: target-gnu/release/aicrawl.exe
```

রিলিজ এক্সিকিউটেবল স্বয়ংসম্পূর্ণ: ড্যাশবোর্ড, মাইগ্রেশন ও SQLite ভিতরে কম্পাইল করা। শুধু ঐচ্ছিক JavaScript রেন্ডারিংয়ের জন্য বাহ্যিক সফটওয়্যার লাগে।

## ঐচ্ছিক JavaScript রেন্ডারিং

SPA ডিফল্টভাবে রেন্ডার হয় না। রেন্ডার করতে:

```bash
cd browser && npm install && npx playwright install chromium
```

তারপর extract/crawl-এ `--render` দিন বা ড্যাশবোর্ডে render চালু করুন। ব্রিজ না থাকলে AICrawl চুপচাপ খালি পেজ ফেরত না দিয়ে স্পষ্ট “rendering unavailable” ত্রুটি দেয়।

## ডেভেলপমেন্ট

```bash
cargo test --workspace          # ইউনিট + ইন্টিগ্রেশন টেস্ট (স্থানীয় HTTP সার্ভার)
cd frontend && npm run dev      # Vite :5173-এ, /api-কে :8080-এ প্রক্সি করে
cargo run -p aicrawl-cli -- doctor
```

ড্যাশবোর্ড কম্পাইল সময়ে এমবেড করা হয়। UI কাজের সময় হয় Vite ডেভ সার্ভার ব্যবহার করুন, নয়তো Rust পুনরায় কম্পাইল না করে ডিস্ক থেকে রিবিল্ড সার্ভ করতে `AICRAWL_DASHBOARD_DIR=frontend/dist` সেট করুন।

## রিলিজ ও সংস্করণ

[`version.json`](version.json) রিলিজ সংস্করণের একক সত্যের উৎস। এটি দুটি কাজে লাগে:

- রিলিজ ট্যাগ (`v` + সংস্করণ, যেমন `v0.0.1-experimental.2`)
- বাইনারিতে কম্পাইল করা সংস্করণ, যা `aicrawl --version`, `aicrawl version` ও MCP সার্ভারে দেখা যায়

রিলিজ প্রকাশ করতে:

1. `version.json` এবং `Cargo.toml`-এর মিলিত `version` আপডেট করে `main`-এ মার্জ করুন।
2. Windows, Linux ও macOS-এ স্বয়ংক্রিয় **CI** ওয়ার্কফ্লো পাস করার জন্য অপেক্ষা করুন।
3. ম্যানুয়ালি **Actions → Release → Run workflow** চালান। ওয়ার্কফ্লো ট্যাগ তৈরি করে, চারটি বাইনারি বানায়, তাদের এমবেড করা সংস্করণ যাচাই করে এবং SHA-256 চেকসামসহ আপলোড করে।

হাইফেন (`-`) থাকা সংস্করণগুলো প্রিরিলিজ হিসেবে প্রকাশিত হয়।

## সীমাবদ্ধতা

- JavaScript-প্রধান পেজের জন্য ঐচ্ছিক Playwright ব্রিজ দরকার।
- `search_website` একটি আনা পেজে খোঁজে; সাইট-ব্যাপী কভারেজের জন্য `crawl_website` ব্যবহার করুন।
- সেমান্টিক এক্সট্রাকশনের গুণমান আপনার কনফিগার করা বাহ্যিক প্রোভাইডারের উপর নির্ভর করে।
- AICrawl সাইটের নীতি মানে; এটি প্রমাণীকরণ, পেওয়াল বা CAPTCHA বাইপাস করে না।

## কমিউনিটি

- [অবদান গাইড](CONTRIBUTING.md)
- [আচরণবিধি](CODE_OF_CONDUCT.md)
- [নিরাপত্তা নীতি](SECURITY.md)
- [সহায়তা](SUPPORT.md)
- [গভর্নেন্স](GOVERNANCE.md)
- [চেঞ্জলগ](CHANGELOG.md)
- [আলোচনা](https://github.com/Ahaduzzamankhan/aicrawl/discussions)
- [বাগ রিপোর্ট করুন](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml)
- [ফিচার অনুরোধ করুন](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml)

AICrawl যদি আপনার কাজে লাগে, রিপোজিটরিতে স্টার দেওয়ার কথা ভাবুন — এটি অন্য মানুষকে প্রকল্পটি খুঁজে পেতে সাহায্য করে।

## লাইসেন্স

MIT

## Star History

<a href="https://www.star-history.com/?repos=ahaduzzamankhan%2Faicrawl&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
 </picture>
</a>
