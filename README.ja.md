> **Note:** this translation predates v1.0.0 (JSON-first requests, no AI provider).
> See [README.md](README.md) and [docs/request-schema.md](docs/request-schema.md) for current behaviour.

<div align="center">

<img src="public/icon.png" alt="AICrawl logo" width="150">

# AICrawl

**AI エージェントと人間が、許可されたウェブサイトをクロールして構造化データを抽出できる単一の実行ファイル。**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?include_prereleases&style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Download](https://img.shields.io/badge/download-all%20platforms-0078D6?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-0EA5E9?style=for-the-badge)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-ready-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![PRs](https://img.shields.io/badge/PRs-welcome-EC4899?style=for-the-badge&logo=git&logoColor=white)](CONTRIBUTING.md)

**他の言語:** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[インストール](#install-one-line) · [クイックスタート](#クイックスタート) · [ダウンロード](#ダウンロード) · [MCP サーバー](#ai-エージェントmcp) · [REST API](#rest-api) · [ダッシュボード](#ダッシュボード) · [コントリビュート](CONTRIBUTING.md) · [セキュリティ](SECURITY.md)

</div>

**AI エージェント**と**人間**が、許可されたウェブサイトをクロールして構造化データを抽出できる単一の実行ファイルです。

> **実験的リリース:** 現在のバージョンは [`version.json`](version.json) にあります。実験的ビルド間でインターフェースやストレージが変わる可能性があります。現時点では無人での本番クロールには使用しないでください。

**Windows x64、Linux x64、macOS Intel、macOS Apple Silicon** 向けのビルド済みバイナリが、SHA-256 チェックサムとともに各[リリース](https://github.com/Ahaduzzamankhan/aicrawl/releases)に添付されます。

1 つの `aicrawl` バイナリに含まれるもの:

| サーフェス | 利用者 | 使用方法 |
| --- | --- | --- |
| CLI | 人間とスクリプト | `aicrawl extract`、`crawl`、`inspect`、`jobs` など |
| ダッシュボード | 人間 | `aicrawl start` が `http://127.0.0.1:8080` で提供する React UI |
| REST API | スクリプトと連携 | `http://127.0.0.1:8080/api/v1` |
| MCP サーバー | AI エージェント | `aicrawl mcp stdio` または `http://127.0.0.1:8787/mcp` |

<p align="center">
  <img src="public/flowchart.png" alt="AICrawl architecture flowchart: CLI, MCP, REST API and dashboard feed one Rust engine that fetches websites, extracts structured data, applies security rules and stores results in SQLite" width="100%">
</p>

ダッシュボードは**実行ファイルに埋め込まれています**。別のウェブサーバーや配布するアセットフォルダーはありません。4 つのサーフェスはすべて同じ `AicrawlEngine` を呼び出すため、SSRF 保護、robots.txt の処理、キャッシュ、ストレージの動作は同一です。

AICrawl は **AI モデルも API キーも同梱しません**。決定論的抽出はすぐに使用できます。外部の OpenAI 互換エンドポイントやローカルの Ollama サーバーを設定すると、セマンティック抽出が利用できます。設定しない場合、MCP は呼び出し元エージェントが推論できるよう、クリーンなページ内容と構造化ヒントを返します。

## Install (one line)

お使いのプラットフォームの最新リリースをダウンロードし、SHA-256 チェックサムを検証してインストールし、`PATH` に追加します（Windows は `%LOCALAPPDATA%\AICrawl\bin`、macOS と Linux は `~/.local/bin`）。

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

`PATH` は現在のシェルでも更新されます。新しく開いた Windows ターミナルで `aicrawl` が見つからない場合は、一度サインアウトしてからサインインし直してください。

## ダウンロード

各[リリース](https://github.com/Ahaduzzamankhan/aicrawl/releases)にはビルド済みバイナリが添付されています。最新リリースを開き、お使いのプラットフォーム用のアセットをダウンロードしてください:

| プラットフォーム | アセット |
| --- | --- |
| Windows x64 | `aicrawl-windows-x64.exe` |
| Linux x64 | `aicrawl-linux-x64` |
| macOS Intel (x64) | `aicrawl-macos-x64` |
| macOS Apple Silicon (arm64) | `aicrawl-macos-arm64` |

各バイナリには対応する `.sha256` チェックサムファイルがあります。実行前に検証してください:

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

ダウンロード後、Unix バイナリに実行権限を付与してください:

```bash
chmod +x aicrawl-linux-x64 aicrawl-macos-arm64
```

バイナリは自己完結型です。ダッシュボード、マイグレーション、SQLite はすべてコンパイル時に埋め込まれます。外部ソフトウェアが必要なのは、オプションの JavaScript レンダリングのみです。

## クイックスタート

```bash
# 1. ダッシュボードをビルドし、バイナリに埋め込む
cd frontend && npm ci && npm run build && cd ..
cargo build --release

# 2. API + ダッシュボード + MCP を起動（1 プロセス）
./target/release/aicrawl start

# 3. 使用する
./target/release/aicrawl extract https://example.com --query "Extract the page title and main headings"
./target/release/aicrawl inspect https://example.com
./target/release/aicrawl crawl https://example.com --max-pages 20 --max-depth 2 --query "Extract product prices"
```

最初の `start` で `~/.aicrawl/aicrawl.db`（SQLite）が開き、ダッシュボード、API、MCP の URL が表示されます。別の場所を使うには `AICRAWL_DATA_DIR` を設定するか、`--data-dir` を渡します。

```bash
aicrawl start --daemon      # バックグラウンドプロセス、ログは <data-dir>/aicrawl.log
aicrawl status
aicrawl stop
```

## AI エージェント（MCP）

### stdio — デスクトップエージェントに推奨

エージェントを同じ実行ファイルに向けます:

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

`aicrawl start`（または `aicrawl mcp start`）を実行し、次を使用します:

```json
{
  "mcpServers": {
    "aicrawl": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

ツール:

- `extract_website_data` — `url`、`query`、オプションの `schema`、`output_format`
- `crawl_website` — `url`、`max_pages`、`max_depth`
- `get_page_content` — `url`
- `inspect_website` — `url`
- `search_website` — `url`、`query`
- `get_crawl_result` — `job_id`
- `get_crawl_status` — `job_id`

ページ内容は**信頼できないデータ**として扱われます。ツール出力はそのようにマークされ、ウェブページ内で見つかった指示には決して従いません。

## 人間向け

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

グローバルフラグ: `--no-color`（`NO_COLOR` も尊重）、`--config <path>`、`--data-dir <dir>`。

### ダッシュボード

`aicrawl start` 後に `http://127.0.0.1:8080` を開きます:

- **Overview** — ヘルス、ジョブ/結果カウンター、最近のアクティビティ
- **New Extraction** — URL、クエリ、スキーマ、フォーマット、レンダリング切り替え
- **Crawl Jobs** — ライブ状態、進捗、キャンセル、ジョブ詳細
- **Results Explorer** — JSON ビューアーとコピー/ダウンロード付きの保存済み結果
- **MCP Configuration** — コピー可能な stdio/HTTP 設定とツール一覧
- **API Keys** — キーの作成/失効（一度だけ表示、ハッシュで保存）
- **AI Providers** — オプションの外部プロバイダー、キーは保存時に暗号化
- **Settings** — 有効なセキュリティ制限、キャッシュ、ストレージ設定

### REST API

すべてのレスポンスは `{ "success": true, "data": … }` または `{ "success": false, "error": { "code", "message", "retryable" } }` を使用します。

```bash
curl -s http://127.0.0.1:8080/api/v1/health

curl -s -X POST http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","query":"Extract the main headings","use_ai":false}'

curl -s -X POST http://127.0.0.1:8080/api/v1/crawl \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","max_pages":10,"max_depth":1}'
```

`[api].require_api_key = true` を設定する（またはループバック外にバインドする）と、`Authorization: Bearer <key>` または `X-API-Key: <key>` を送信します。キーは SHA-256 ハッシュとして保存され、一度だけ表示されます。

## 外部 AI（オプション）

**ダッシュボード → AI Providers** または API でプロバイダーを設定します:

- OpenAI 互換: ベース URL、モデル、API キー
- Ollama: ベース URL（`http://127.0.0.1:11434`）、モデル、キー不要

API キーは ChaCha20-Poly1305 で暗号化されます。暗号化キーは `AICRAWL_SECRET_KEY`（base64 エンコードされた 32 バイトのキー）から取得するか、初回に `<data-dir>/secret.key` へ所有者のみの権限で生成されます。平文キーが SQLite、ログ、ダッシュボードに書き込まれることはありません。

`aicrawl doctor` はプロバイダーが設定されているかを報告します。設定がない場合、抽出は決定論的で、失敗ではなく警告を返します。

## セキュリティ

- SSRF 保護は DNS を解決し、すべてのリダイレクトホップでループバック、プライベート、リンクローカル、マルチキャスト、メタデータアドレスを拒否します。
- リダイレクトは手動で追跡し（最大 5 回）、毎回再検証します。
- `robots.txt` は取得、キャッシュされ、デフォルトで適用されます。
- ドメインの許可/拒否リスト、ホストごとのレート制限、ページ/深さの上限、実時間クロール予算、リクエストタイムアウト、キャンセル。
- レスポンス本文はサイズ上限があり、テキスト抽出用に HTML をサニタイズし、スクリプト/スタイルを除去します。
- プロンプトインジェクションのマーカーを検出し、外部 AI 呼び出しの前にページテキストを信頼できないデータとしてラップします。
- オプションの API キー認証、定数時間のハッシュ比較、URL にキーを含めません。
- CAPTCHA、認証、ペイウォールの回避は行いません。ステルス/回避機能はありません。

## ストレージ

SQLite のみ（`sqlx`、WAL、接続プール、プリペアドステートメント、マイグレーション、インデックス、トランザクション）。テーブル: jobs、pages、results、websites、settings、API keys、AI providers、page cache、robots cache。

## Visual Studio なしで Windows にビルド

同梱の SQLite には C コンパイラーが必要です。MSVC Build Tools がない場合は、MinGW-w64 と GNU ターゲットを使用します（このリポジトリは `.tools/` を無視します）:

```bash
# 初回のみ: MinGW-w64 ツールチェーン（gcc、dlltool、ar）を .tools/mingw64 に展開
export PATH="$PWD/.tools/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
export CARGO_TARGET_DIR=target-gnu
cargo +stable-x86_64-pc-windows-gnu build --release
# バイナリ: target-gnu/release/aicrawl.exe
```

リリース用実行ファイルは自己完結型です。ダッシュボード、マイグレーション、SQLite はすべてコンパイル時に埋め込まれます。外部ソフトウェアが必要なのは、オプションの JavaScript レンダリングのみです。

## オプションの JavaScript レンダリング

SPA はデフォルトではレンダリングされません。レンダリングするには:

```bash
cd browser && npm install && npx playwright install chromium
```

その後、extract/crawl で `--render` を渡すか、ダッシュボードでレンダリングを有効にします。ブリッジがない場合、AICrawl は静かに空のページを返す代わりに、明示的な「rendering unavailable」エラーを返します。

## 開発

```bash
cargo test --workspace          # ユニット + 統合テスト（ローカル HTTP サーバー）
cd frontend && npm run dev      # Vite は :5173、/api を :8080 にプロキシ
cargo run -p aicrawl-cli -- doctor
```

ダッシュボードはコンパイル時に埋め込まれます。UI 作業中は Vite 開発サーバーを使用するか、`AICRAWL_DASHBOARD_DIR=frontend/dist` を設定して、Rust を再コンパイルせずにディスクから再ビルドを配信します。

## リリースとバージョニング

[`version.json`](version.json) はリリースバージョンの唯一の情報源です。次の 2 つを決定します:

- リリースタグ（`v` + バージョン、例: `v0.0.1-experimental.2`）
- バイナリにコンパイルされるバージョン。`aicrawl --version`、`aicrawl version`、MCP サーバーで表示されます

リリースを公開するには:

1. `version.json` と `Cargo.toml` の対応する `version` を更新し、`main` にマージします。
2. 自動 **CI** ワークフローが Windows、Linux、macOS で成功するのを待ちます。
3. **Actions → Release → Run workflow** を手動で実行します。ワークフローはタグを作成し、4 つのバイナリをビルドし、埋め込まれたバージョンを検証して、SHA-256 チェックサムとともにアップロードします。

ハイフン（`-`）を含むバージョンはプレリリースとして公開されます。

## 制限事項

- JavaScript を多用するページにはオプションの Playwright ブリッジが必要です。
- `search_website` は取得した 1 ページ内を検索します。サイト全体を対象にするには `crawl_website` を使用してください。
- セマンティック抽出の品質は、設定した外部プロバイダーに依存します。
- AICrawl はサイトポリシーを尊重します。認証、ペイウォール、CAPTCHA を回避しません。

## コミュニティ

- [コントリビューションガイド](CONTRIBUTING.md)
- [行動規範](CODE_OF_CONDUCT.md)
- [セキュリティポリシー](SECURITY.md)
- [サポート](SUPPORT.md)
- [ガバナンス](GOVERNANCE.md)
- [変更履歴](CHANGELOG.md)
- [ディスカッション](https://github.com/Ahaduzzamankhan/aicrawl/discussions)
- [バグを報告](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml)
- [機能をリクエスト](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml)

AICrawl が役に立ったら、リポジトリにスターを付けることを検討してください。他の人がプロジェクトを見つけやすくなります。

## ライセンス

MIT

## Star History

<a href="https://www.star-history.com/?repos=ahaduzzamankhan%2Faicrawl&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
 </picture>
</a>
