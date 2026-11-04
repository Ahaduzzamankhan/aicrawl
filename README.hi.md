<div align="center">

# AICrawl

**एक ऐसा executable जो AI एजेंट और इंसानों को अनुमत वेबसाइटों को crawl करके संरचित डेटा निकालने देता है।**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?include_prereleases&style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Download](https://img.shields.io/badge/download-all%20platforms-0078D6?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-0EA5E9?style=for-the-badge)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-ready-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![PRs](https://img.shields.io/badge/PRs-welcome-EC4899?style=for-the-badge&logo=git&logoColor=white)](CONTRIBUTING.md)

**इसे पढ़ें:** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[त्वरित शुरुआत](#त्वरित-शुरुआत) · [डाउनलोड](#डाउनलोड) · [MCP सर्वर](#ai-एजेंट-mcp) · [REST API](#rest-api) · [डैशबोर्ड](#डैशबोर्ड) · [योगदान](CONTRIBUTING.md) · [सुरक्षा](SECURITY.md)

</div>

एक ऐसा executable जो **AI एजेंट** और **इंसानों** को अनुमत वेबसाइटों को crawl करके संरचित डेटा निकालने देता है।

> **प्रयोगात्मक रिलीज़:** मौजूदा संस्करण [`version.json`](version.json) में है। प्रयोगात्मक बिल्डों के बीच इंटरफ़ेस और स्टोरेज बदल सकते हैं; अभी इसे बिना निगरानी वाले production crawling के लिए न मानें।

**Windows x64, Linux x64, macOS Intel और macOS Apple Silicon** के लिए तैयार बाइनरी हर [रिलीज़](https://github.com/Ahaduzzamankhan/aicrawl/releases) के साथ SHA-256 checksum के साथ दी जाती हैं।

एक `aicrawl` बाइनरी में ये शामिल हैं:

| सतह | कौन उपयोग करता है | कैसे |
| --- | --- | --- |
| CLI | इंसान और स्क्रिप्ट | `aicrawl extract`, `crawl`, `inspect`, `jobs`, … |
| डैशबोर्ड | इंसान | `aicrawl start` द्वारा `http://127.0.0.1:8080` पर परोसा गया React UI |
| REST API | स्क्रिप्ट और इंटीग्रेशन | `http://127.0.0.1:8080/api/v1` |
| MCP सर्वर | AI एजेंट | `aicrawl mcp stdio` या `http://127.0.0.1:8787/mcp` |

डैशबोर्ड **executable के अंदर embedded** है; कोई अलग वेब सर्वर या भेजने के लिए asset फ़ोल्डर नहीं है। चारों सतहें एक ही `AicrawlEngine` को कॉल करती हैं, इसलिए SSRF सुरक्षा, robots.txt हैंडलिंग, caching और storage एक जैसा व्यवहार करते हैं।

AICrawl **कोई AI मॉडल या API key** साथ नहीं देता। Deterministic extraction बिना किसी सेटअप के काम करता है। यदि आप बाहरी OpenAI-संगत endpoint या स्थानीय Ollama सर्वर कॉन्फ़िगर करते हैं, तो semantic extraction उपलब्ध होता है; अन्यथा MCP कॉल करने वाले एजेंट के तर्क के लिए साफ़ पेज सामग्री और संरचित संकेत लौटाता है।

## डाउनलोड

हर [रिलीज़](https://github.com/Ahaduzzamankhan/aicrawl/releases) में तैयार बाइनरी जुड़ी होती हैं। नई रिलीज़ खोलकर अपने प्लेटफ़ॉर्म का asset डाउनलोड करें:

| प्लेटफ़ॉर्म | Asset |
| --- | --- |
| Windows x64 | `aicrawl-windows-x64.exe` |
| Linux x64 | `aicrawl-linux-x64` |
| macOS Intel (x64) | `aicrawl-macos-x64` |
| macOS Apple Silicon (arm64) | `aicrawl-macos-arm64` |

हर बाइनरी के साथ एक मिलता-जुलता `.sha256` checksum फ़ाइल होता है। चलाने से पहले सत्यापित करें:

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

डाउनलोड के बाद Unix बाइनरी को executable बनाएँ:

```bash
chmod +x aicrawl-linux-x64 aicrawl-macos-arm64
```

बाइनरी self-contained हैं: डैशबोर्ड, migrations और SQLite अंदर compiled हैं। केवल वैकल्पिक JavaScript rendering के लिए बाहरी सॉफ़्टवेयर चाहिए।

## त्वरित शुरुआत

```bash
# 1. डैशबोर्ड बनाएँ, फिर उसे बाइनरी में embed करें
cd frontend && npm ci && npm run build && cd ..
cargo build --release

# 2. API + डैशबोर्ड + MCP शुरू करें (एक प्रोसेस)
./target/release/aicrawl start

# 3. उपयोग करें
./target/release/aicrawl extract https://example.com --query "Extract the page title and main headings"
./target/release/aicrawl inspect https://example.com
./target/release/aicrawl crawl https://example.com --max-pages 20 --max-depth 2 --query "Extract product prices"
```

पहला `start` `~/.aicrawl/aicrawl.db` (SQLite) खोलता है और डैशबोर्ड, API तथा MCP URL प्रिंट करता है। दूसरी जगह उपयोग करने के लिए `AICRAWL_DATA_DIR` सेट करें या `--data-dir` दें।

```bash
aicrawl start --daemon      # बैकग्राउंड प्रोसेस, लॉग <data-dir>/aicrawl.log में
aicrawl status
aicrawl stop
```

## AI एजेंट (MCP)

### stdio — डेस्कटॉप एजेंट के लिए अनुशंसित

एजेंट को उसी executable की ओर इंगित करें:

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

`aicrawl start` (या `aicrawl mcp start`) चलाएँ और उपयोग करें:

```json
{
  "mcpServers": {
    "aicrawl": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

टूल:

- `extract_website_data` — `url`, `query`, वैकल्पिक `schema`, `output_format`
- `crawl_website` — `url`, `max_pages`, `max_depth`
- `get_page_content` — `url`
- `inspect_website` — `url`
- `search_website` — `url`, `query`
- `get_crawl_result` — `job_id`
- `get_crawl_status` — `job_id`

पेज सामग्री को **अविश्वसनीय डेटा** माना जाता है। टूल आउटपुट उसे ऐसा चिह्नित करता है और वेब पेजों में मिले निर्देशों का कभी पालन नहीं करता।

## इंसान

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

वैश्विक फ़्लैग: `--no-color` (`NO_COLOR` भी माना जाता है), `--config <path>`, `--data-dir <dir>`।

### डैशबोर्ड

`aicrawl start` के बाद `http://127.0.0.1:8080` खोलें:

- **Overview** — health, job/result काउंटर, हाल की गतिविधि
- **New Extraction** — URL, query, schema, format, render टॉगल
- **Crawl Jobs** — लाइव स्थिति, प्रगति, रद्द करना, job विवरण
- **Results Explorer** — JSON viewer और copy/download के साथ संग्रहीत परिणाम
- **MCP Configuration** — copy-ready stdio/HTTP कॉन्फ़िग और टूल सूची
- **API Keys** — key बनाएँ/रद्द करें (एक बार दिखाई जाती है, hash करके संग्रहीत)
- **AI Providers** — वैकल्पिक बाहरी providers, keys at rest encrypted
- **Settings** — प्रभावी सुरक्षा सीमाएँ, cache और storage नियंत्रण

### REST API

सभी प्रतिक्रियाएँ `{ "success": true, "data": … }` या `{ "success": false, "error": { "code", "message", "retryable" } }` रूप में।

```bash
curl -s http://127.0.0.1:8080/api/v1/health

curl -s -X POST http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","query":"Extract the main headings","use_ai":false}'

curl -s -X POST http://127.0.0.1:8080/api/v1/crawl \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","max_pages":10,"max_depth":1}'
```

`[api].require_api_key = true` सेट करें (या loopback से बाहर bind करें) और `Authorization: Bearer <key>` या `X-API-Key: <key>` भेजें। Keys SHA-256 hashes के रूप में संग्रहीत होती हैं और केवल एक बार दिखाई जाती हैं।

## बाहरी AI (वैकल्पिक)

**डैशबोर्ड → AI Providers** में या API से provider कॉन्फ़िगर करें:

- OpenAI-संगत: base URL, model, API key
- Ollama: base URL (`http://127.0.0.1:11434`), model, key नहीं

API keys ChaCha20-Poly1305 से encrypted हैं। Encryption key `AICRAWL_SECRET_KEY` (base64-encoded 32-byte key) से आती है या एक बार `<data-dir>/secret.key` में केवल owner permissions के साथ बनाई जाती है। Plaintext keys कभी SQLite, logs या dashboard में नहीं लिखी जातीं।

`aicrawl doctor` बताता है कि कोई provider कॉन्फ़िगर है या नहीं; उसके बिना extraction deterministic है और विफल होने के बजाय चेतावनी लौटाता है।

## सुरक्षा

- SSRF सुरक्षा DNS resolve करती है और हर redirect hop पर loopback, private, link-local, multicast और metadata पतों को अस्वीकार करती है।
- Redirects मैन्युअली follow होते हैं (अधिकतम 5) और हर बार फिर से validated होते हैं।
- `robots.txt` fetch, cache और डिफ़ॉल्ट रूप से लागू होता है।
- Domain allow/deny lists, per-host rate limiting, page/depth caps, wall-clock crawl budgets, request timeouts और cancellation।
- Response bodies का आकार सीमित है, text extraction के लिए HTML sanitize होता है, scripts/styles हटाए जाते हैं।
- Prompt-injection markers पहचाने जाते हैं और किसी भी बाहरी AI call से पहले page text को untrusted data के रूप में wrap किया जाता है।
- वैकल्पिक API-key auth; constant-time hash comparison; URLs में कोई key नहीं।
- कोई CAPTCHA, authentication या paywall bypass नहीं; कोई stealth/evasion सुविधा नहीं।

## स्टोरेज

केवल SQLite (`sqlx`, WAL, pooled connections, prepared statements, migrations, indexes, transactions)। Tables: jobs, pages, results, websites, settings, API keys, AI providers, page cache, robots cache।

## Visual Studio के बिना Windows पर बिल्ड

Bundled SQLite को C compiler चाहिए। यदि MSVC Build Tools नहीं हैं, तो MinGW-w64 के साथ GNU target उपयोग करें (यह repository `.tools/` को ignore करती है):

```bash
# एक बार: .tools/mingw64 में MinGW-w64 toolchain unpack करें (gcc, dlltool, ar)
export PATH="$PWD/.tools/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
export CARGO_TARGET_DIR=target-gnu
cargo +stable-x86_64-pc-windows-gnu build --release
# binary: target-gnu/release/aicrawl.exe
```

Release executable self-contained है: dashboard, migrations और SQLite अंदर compiled हैं। केवल वैकल्पिक JavaScript rendering के लिए बाहरी सॉफ़्टवेयर चाहिए।

## वैकल्पिक JavaScript rendering

SPAs डिफ़ॉल्ट रूप से render नहीं होते। Render करने के लिए:

```bash
cd browser && npm install && npx playwright install chromium
```

फिर extract/crawl पर `--render` दें या dashboard में render चालू करें। Bridge के बिना AICrawl चुपचाप खाली पेज लौटाने के बजाय स्पष्ट “rendering unavailable” त्रुटि देता है।

## विकास

```bash
cargo test --workspace          # unit + integration tests (स्थानीय HTTP server)
cd frontend && npm run dev      # Vite :5173 पर, /api को :8080 पर proxy करता है
cargo run -p aicrawl-cli -- doctor
```

Dashboard compile time पर embedded होता है। UI काम के दौरान या तो Vite dev server उपयोग करें या Rust दोबारा compile किए बिना disk से rebuild serve करने के लिए `AICRAWL_DASHBOARD_DIR=frontend/dist` सेट करें।

## रिलीज़ और संस्करण

[`version.json`](version.json) release version का एकल स्रोत है। यह दो चीज़ें तय करता है:

- release tag (`v` + version, जैसे `v0.0.1-experimental.2`)
- binary में compiled version, जो `aicrawl --version`, `aicrawl version` और MCP server में दिखता है

Release प्रकाशित करने के लिए:

1. `version.json` और `Cargo.toml` में मिलता-जुलता `version` अपडेट करके `main` में merge करें।
2. Windows, Linux और macOS पर स्वचालित **CI** workflow पास होने की प्रतीक्षा करें।
3. मैन्युअली **Actions → Release → Run workflow** चलाएँ। Workflow tag बनाता है, चारों binaries बनाता है, उनका embedded version सत्यापित करता है और SHA-256 checksums के साथ upload करता है।

हाइफ़न (`-`) वाले versions prereleases के रूप में प्रकाशित होते हैं।

## सीमाएँ

- JavaScript-भारी पेजों के लिए वैकल्पिक Playwright bridge चाहिए।
- `search_website` एक fetched page में खोजता है; site-wide coverage के लिए `crawl_website` उपयोग करें।
- Semantic extraction की गुणवत्ता आपके कॉन्फ़िगर किए गए बाहरी provider पर निर्भर करती है।
- AICrawl साइट नीति का सम्मान करता है; यह authentication, paywalls या CAPTCHAs bypass नहीं करता।

## समुदाय

- [योगदान गाइड](CONTRIBUTING.md)
- [आचरण संहिता](CODE_OF_CONDUCT.md)
- [सुरक्षा नीति](SECURITY.md)
- [सहायता](SUPPORT.md)
- [गवर्नेंस](GOVERNANCE.md)
- [चेंजलॉग](CHANGELOG.md)
- [चर्चाएँ](https://github.com/Ahaduzzamankhan/aicrawl/discussions)
- [बग रिपोर्ट करें](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml)
- [फ़ीचर का अनुरोध करें](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml)

यदि AICrawl आपके लिए उपयोगी है, तो repository को star करने पर विचार करें — इससे दूसरे लोगों को प्रोजेक्ट खोजने में मदद मिलती है।

## लाइसेंस

MIT
