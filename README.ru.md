> **Note:** this translation predates v1.0.0 (JSON-first requests, no AI provider).
> See [README.md](README.md) and [docs/request-schema.md](docs/request-schema.md) for current behaviour.

<div align="center">

<img src="public/icon.png" alt="AICrawl logo" width="150">

# AICrawl

**Один исполняемый файл, позволяющий AI-агентам и людям обходить разрешённые сайты и извлекать структурированные данные.**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?include_prereleases&style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Download](https://img.shields.io/badge/download-all%20platforms-0078D6?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-0EA5E9?style=for-the-badge)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-ready-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![PRs](https://img.shields.io/badge/PRs-welcome-EC4899?style=for-the-badge&logo=git&logoColor=white)](CONTRIBUTING.md)

**Читать на:** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[Установка](#install-one-line) · [Быстрый старт](#быстрый-старт) · [Загрузка](#загрузка) · [MCP-сервер](#ai-агенты-mcp) · [REST API](#rest-api) · [Панель управления](#панель-управления) · [Участие](CONTRIBUTING.md) · [Безопасность](SECURITY.md)

</div>

Один исполняемый файл, позволяющий **AI-агентам** и **людям** обходить разрешённые сайты и извлекать структурированные данные.

> **Экспериментальный выпуск:** текущая версия находится в [`version.json`](version.json). Интерфейсы и хранилище могут меняться между экспериментальными сборками; пока не используйте его для необслуживаемого обхода сайтов в продакшене.

Готовые бинарные файлы для **Windows x64, Linux x64, macOS Intel и macOS Apple Silicon** прилагаются к каждому [выпуску](https://github.com/Ahaduzzamankhan/aicrawl/releases) вместе с контрольными суммами SHA-256.

Один бинарный файл `aicrawl` содержит:

| Интерфейс | Кто использует | Как |
| --- | --- | --- |
| CLI | люди и скрипты | `aicrawl extract`, `crawl`, `inspect`, `jobs`, … |
| Панель управления | люди | React UI, обслуживаемый `aicrawl start` на `http://127.0.0.1:8080` |
| REST API | скрипты и интеграции | `http://127.0.0.1:8080/api/v1` |
| MCP-сервер | AI-агенты | `aicrawl mcp stdio` или `http://127.0.0.1:8787/mcp` |

<p align="center">
  <img src="public/flowchart.png" alt="AICrawl architecture flowchart: CLI, MCP, REST API and dashboard feed one Rust engine that fetches websites, extracts structured data, applies security rules and stores results in SQLite" width="100%">
</p>

Панель управления **встроена в исполняемый файл**; отдельного веб-сервера и папки ресурсов для распространения нет. Все четыре интерфейса вызывают один и тот же `AicrawlEngine`, поэтому защита от SSRF, обработка robots.txt, кэширование и хранилище работают одинаково.

AICrawl **не включает AI-модель и API-ключи**. Детерминированное извлечение работает сразу. Если настроить внешний OpenAI-совместимый endpoint или локальный сервер Ollama, станет доступно семантическое извлечение; иначе MCP возвращает очищенное содержимое страницы и структурированные подсказки, по которым может рассуждать вызывающий агент.

## Install (one line)

Загружает последний выпуск для вашей платформы, проверяет контрольную сумму SHA-256, устанавливает его и добавляет в `PATH` (`%LOCALAPPDATA%\AICrawl\bin` в Windows, `~/.local/bin` в macOS и Linux).

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

`PATH` также обновляется в текущей оболочке. Если в новом окне терминала Windows `aicrawl` по-прежнему не найден, выйдите из системы и войдите снова.

## Загрузка

К каждому [выпуску](https://github.com/Ahaduzzamankhan/aicrawl/releases) прилагаются готовые бинарные файлы. Откройте последний выпуск и скачайте файл для вашей платформы:

| Платформа | Файл |
| --- | --- |
| Windows x64 | `aicrawl-windows-x64.exe` |
| Linux x64 | `aicrawl-linux-x64` |
| macOS Intel (x64) | `aicrawl-macos-x64` |
| macOS Apple Silicon (arm64) | `aicrawl-macos-arm64` |

У каждого бинарного файла есть соответствующий файл контрольной суммы `.sha256`. Проверьте его перед запуском:

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

После загрузки сделайте Unix-бинарные файлы исполняемыми:

```bash
chmod +x aicrawl-linux-x64 aicrawl-macos-arm64
```

Бинарные файлы самодостаточны: панель управления, миграции и SQLite вкомпилированы. Внешнее ПО требуется только для необязательного рендеринга JavaScript.

## Быстрый старт

```bash
# 1. Соберите панель управления и встройте её в бинарный файл
cd frontend && npm ci && npm run build && cd ..
cargo build --release

# 2. Запустите API + панель + MCP (один процесс)
./target/release/aicrawl start

# 3. Используйте
./target/release/aicrawl extract https://example.com --query "Extract the page title and main headings"
./target/release/aicrawl inspect https://example.com
./target/release/aicrawl crawl https://example.com --max-pages 20 --max-depth 2 --query "Extract product prices"
```

Первый `start` открывает `~/.aicrawl/aicrawl.db` (SQLite) и выводит URL панели, API и MCP. Задайте `AICRAWL_DATA_DIR` или передайте `--data-dir`, чтобы использовать другое расположение.

```bash
aicrawl start --daemon      # фоновый процесс, журнал в <data-dir>/aicrawl.log
aicrawl status
aicrawl stop
```

## AI-агенты (MCP)

### stdio — рекомендуется для настольных агентов

Укажите агенту тот же исполняемый файл:

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

Запустите `aicrawl start` (или `aicrawl mcp start`) и используйте:

```json
{
  "mcpServers": {
    "aicrawl": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

Инструменты:

- `extract_website_data` — `url`, `query`, необязательные `schema`, `output_format`
- `crawl_website` — `url`, `max_pages`, `max_depth`
- `get_page_content` — `url`
- `inspect_website` — `url`
- `search_website` — `url`, `query`
- `get_crawl_result` — `job_id`
- `get_crawl_status` — `job_id`

Содержимое страниц считается **недоверенными данными**. Вывод инструментов помечает его как таковое и никогда не следует инструкциям, найденным на веб-страницах.

## Для людей

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

Глобальные флаги: `--no-color` (также учитывает `NO_COLOR`), `--config <path>`, `--data-dir <dir>`.

### Панель управления

Откройте `http://127.0.0.1:8080` после `aicrawl start`:

- **Overview** — состояние, счётчики заданий/результатов, недавняя активность
- **New Extraction** — URL, запрос, схема, формат, переключатель рендеринга
- **Crawl Jobs** — текущий статус, прогресс, отмена, детали задания
- **Results Explorer** — сохранённые результаты с JSON-просмотрщиком и копированием/скачиванием
- **MCP Configuration** — готовые к копированию конфигурации stdio/HTTP и список инструментов
- **API Keys** — создание/отзыв ключей (показываются один раз, хранятся в виде хэша)
- **AI Providers** — необязательные внешние провайдеры, ключи зашифрованы при хранении
- **Settings** — действующие ограничения безопасности, управление кэшем и хранилищем

### REST API

Все ответы используют `{ "success": true, "data": … }` или `{ "success": false, "error": { "code", "message", "retryable" } }`.

```bash
curl -s http://127.0.0.1:8080/api/v1/health

curl -s -X POST http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","query":"Extract the main headings","use_ai":false}'

curl -s -X POST http://127.0.0.1:8080/api/v1/crawl \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","max_pages":10,"max_depth":1}'
```

Задайте `[api].require_api_key = true` (или привяжите к адресу за пределами loopback) и отправляйте `Authorization: Bearer <key>` или `X-API-Key: <key>`. Ключи хранятся как хэши SHA-256 и показываются только один раз.

## Внешний AI (необязательно)

Настройте провайдера в **Панель управления → AI Providers** или через API:

- Совместимый с OpenAI: базовый URL, модель, API-ключ
- Ollama: базовый URL (`http://127.0.0.1:11434`), модель, без ключа

API-ключи шифруются ChaCha20-Poly1305. Ключ шифрования берётся из `AICRAWL_SECRET_KEY` (ключ на 32 байта в base64) или создаётся один раз в `<data-dir>/secret.key` с правами только для владельца. Открытые ключи никогда не записываются в SQLite, журналы или панель управления.

`aicrawl doctor` сообщает, настроен ли провайдер; без него извлечение детерминировано и возвращает предупреждение вместо ошибки.

## Безопасность

- Защита от SSRF разрешает DNS и отклоняет loopback-, частные, link-local, multicast- и metadata-адреса на каждом шаге перенаправления.
- Перенаправления обрабатываются вручную (максимум 5) и повторно проверяются каждый раз.
- `robots.txt` загружается, кэшируется и применяется по умолчанию.
- Списки разрешённых/запрещённых доменов, ограничение частоты запросов на хост, лимиты страниц/глубины, бюджеты времени обхода, тайм-ауты запросов и отмена.
- Размер тел ответов ограничен, HTML очищается для извлечения текста, скрипты и стили удаляются.
- Маркеры внедрения подсказок обнаруживаются, а текст страницы оборачивается как недоверенные данные перед любым вызовом внешнего AI.
- Необязательная аутентификация по API-ключу; сравнение хэшей за постоянное время; ключи не передаются в URL.
- Никакого обхода CAPTCHA, аутентификации или платных стен; никаких функций скрытности/уклонения.

## Хранилище

Только SQLite (`sqlx`, WAL, пул соединений, подготовленные выражения, миграции, индексы, транзакции). Таблицы: jobs, pages, results, websites, settings, API keys, AI providers, page cache, robots cache.

## Сборка в Windows без Visual Studio

Встроенному SQLite нужен компилятор C. Если MSVC Build Tools недоступны, используйте GNU-цель с MinGW-w64 (этот репозиторий игнорирует `.tools/`):

```bash
# один раз: распакуйте инструментарий MinGW-w64 (gcc, dlltool, ar) в .tools/mingw64
export PATH="$PWD/.tools/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
export CARGO_TARGET_DIR=target-gnu
cargo +stable-x86_64-pc-windows-gnu build --release
# бинарный файл: target-gnu/release/aicrawl.exe
```

Релизный исполняемый файл самодостаточен: панель управления, миграции и SQLite вкомпилированы. Внешнее ПО требуется только для необязательного рендеринга JavaScript.

## Необязательный рендеринг JavaScript

SPA по умолчанию не рендерятся. Чтобы включить рендеринг:

```bash
cd browser && npm install && npx playwright install chromium
```

Затем передайте `--render` в extract/crawl или включите рендеринг в панели управления. Без моста AICrawl возвращает явную ошибку «rendering unavailable» вместо молчаливой пустой страницы.

## Разработка

```bash
cargo test --workspace          # модульные + интеграционные тесты (локальный HTTP-сервер)
cd frontend && npm run dev      # Vite на :5173, проксирует /api на :8080
cargo run -p aicrawl-cli -- doctor
```

Панель управления встраивается во время компиляции. При работе над UI используйте dev-сервер Vite или задайте `AICRAWL_DASHBOARD_DIR=frontend/dist`, чтобы отдавать пересборку с диска без перекомпиляции Rust.

## Выпуски и версионирование

[`version.json`](version.json) — единственный источник истины для версии выпуска. Он определяет две вещи:

- тег выпуска (`v` + версия, например `v0.0.1-experimental.2`)
- версию, вкомпилированную в бинарный файл, которую показывают `aicrawl --version`, `aicrawl version` и MCP-сервер

Чтобы опубликовать выпуск:

1. Обновите `version.json` и соответствующую `version` в `Cargo.toml`, затем выполните merge в `main`.
2. Дождитесь успешного прохождения автоматического workflow **CI** на Windows, Linux и macOS.
3. Вручную запустите **Actions → Release → Run workflow**. Workflow создаст тег, соберёт четыре бинарных файла, проверит встроенную версию и загрузит их с контрольными суммами SHA-256.

Версии с дефисом (`-`) публикуются как предварительные выпуски.

## Ограничения

- Для страниц с интенсивным JavaScript требуется необязательный мост Playwright.
- `search_website` ищет на одной загруженной странице; для охвата всего сайта используйте `crawl_website`.
- Качество семантического извлечения зависит от настроенного внешнего провайдера.
- AICrawl соблюдает политику сайтов; он не обходит аутентификацию, платные стены или CAPTCHA.

## Сообщество

- [Руководство по участию](CONTRIBUTING.md)
- [Кодекс поведения](CODE_OF_CONDUCT.md)
- [Политика безопасности](SECURITY.md)
- [Поддержка](SUPPORT.md)
- [Управление](GOVERNANCE.md)
- [Журнал изменений](CHANGELOG.md)
- [Обсуждения](https://github.com/Ahaduzzamankhan/aicrawl/discussions)
- [Сообщить об ошибке](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml)
- [Запросить функцию](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml)

Если AICrawl оказался полезен, поставьте звезду репозиторию — это помогает другим людям найти проект.

## Лицензия

MIT

## Star History

<a href="https://www.star-history.com/?repos=ahaduzzamankhan%2Faicrawl&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
 </picture>
</a>
