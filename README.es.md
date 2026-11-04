> **Note:** this translation predates v1.0.0 (JSON-first requests, no AI provider).
> See [README.md](README.md) and [docs/request-schema.md](docs/request-schema.md) for current behaviour.

<div align="center">

<img src="public/icon.png" alt="AICrawl logo" width="150">

# AICrawl

**Un único ejecutable que permite a agentes de IA y personas rastrear sitios web permitidos y extraer datos estructurados.**

[![CI](https://img.shields.io/github/actions/workflow/status/Ahaduzzamankhan/aicrawl/ci.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/Ahaduzzamankhan/aicrawl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ahaduzzamankhan/aicrawl?include_prereleases&style=for-the-badge&logo=github&logoColor=white&label=release&color=7C3AED)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Download](https://img.shields.io/badge/download-all%20platforms-0078D6?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-0EA5E9?style=for-the-badge)](https://github.com/Ahaduzzamankhan/aicrawl/releases)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=for-the-badge&logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-ready-0EA5E9?style=for-the-badge)](https://modelcontextprotocol.io)
[![PRs](https://img.shields.io/badge/PRs-welcome-EC4899?style=for-the-badge&logo=git&logoColor=white)](CONTRIBUTING.md)

**Léelo en:** [English](README.md) · [বাংলা](README.bn.md) · [हिन्दी](README.hi.md) · [Español](README.es.md) · [Nederlands](README.nl.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Русский](README.ru.md)

[Inicio rápido](#inicio-rápido) · [Descarga](#descarga) · [Servidor MCP](#agentes-de-ia-mcp) · [API REST](#api-rest) · [Panel](#panel) · [Contribuir](CONTRIBUTING.md) · [Seguridad](SECURITY.md)

</div>

Un único ejecutable que permite a **agentes de IA** y **personas** rastrear sitios web permitidos y extraer datos estructurados.

> **Versión experimental:** la versión actual está en [`version.json`](version.json). Las interfaces y el almacenamiento pueden cambiar entre compilaciones experimentales; todavía no conviene usarlo para rastreo de producción sin supervisión.

Los binarios precompilados para **Windows x64, Linux x64, macOS Intel y macOS Apple Silicon** se adjuntan a cada [versión](https://github.com/Ahaduzzamankhan/aicrawl/releases), junto con sumas de verificación SHA-256.

Un único binario `aicrawl` contiene:

| Superficie | Quién la usa | Cómo |
| --- | --- | --- |
| CLI | personas y scripts | `aicrawl extract`, `crawl`, `inspect`, `jobs`, … |
| Panel | personas | interfaz React servida por `aicrawl start` en `http://127.0.0.1:8080` |
| API REST | scripts e integraciones | `http://127.0.0.1:8080/api/v1` |
| Servidor MCP | agentes de IA | `aicrawl mcp stdio` o `http://127.0.0.1:8787/mcp` |

<p align="center">
  <img src="public/flowchart.png" alt="AICrawl architecture flowchart: CLI, MCP, REST API and dashboard feed one Rust engine that fetches websites, extracts structured data, applies security rules and stores results in SQLite" width="100%">
</p>

El panel está **integrado en el ejecutable**; no hay un servidor web separado ni una carpeta de recursos que distribuir. Las cuatro superficies llaman al mismo `AicrawlEngine`, por lo que la protección SSRF, el manejo de robots.txt, la caché y el almacenamiento se comportan igual.

AICrawl **no incluye ningún modelo de IA ni claves de API**. La extracción determinista funciona sin configuración. Si configuras un endpoint externo compatible con OpenAI o un servidor Ollama local, se habilita la extracción semántica; de lo contrario, MCP devuelve el contenido limpio de la página y pistas estructuradas para que el agente que llama razone sobre ellas.

## Descarga

En cada [versión](https://github.com/Ahaduzzamankhan/aicrawl/releases) se adjuntan binarios precompilados. Abre la versión más reciente y descarga el recurso de tu plataforma:

| Plataforma | Recurso |
| --- | --- |
| Windows x64 | `aicrawl-windows-x64.exe` |
| Linux x64 | `aicrawl-linux-x64` |
| macOS Intel (x64) | `aicrawl-macos-x64` |
| macOS Apple Silicon (arm64) | `aicrawl-macos-arm64` |

Cada binario tiene un archivo de suma de verificación `.sha256` correspondiente. Verifícalo antes de ejecutarlo:

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

Haz ejecutables los binarios de Unix después de descargarlos:

```bash
chmod +x aicrawl-linux-x64 aicrawl-macos-arm64
```

Los binarios son autocontenidos: el panel, las migraciones y SQLite están compilados dentro. Solo la renderización opcional de JavaScript necesita software externo.

## Inicio rápido

```bash
# 1. Compila el panel y luego intégralo en el binario
cd frontend && npm ci && npm run build && cd ..
cargo build --release

# 2. Inicia API + panel + MCP (un solo proceso)
./target/release/aicrawl start

# 3. Úsalo
./target/release/aicrawl extract https://example.com --query "Extract the page title and main headings"
./target/release/aicrawl inspect https://example.com
./target/release/aicrawl crawl https://example.com --max-pages 20 --max-depth 2 --query "Extract product prices"
```

El primer `start` abre `~/.aicrawl/aicrawl.db` (SQLite) e imprime las URL del panel, la API y MCP. Define `AICRAWL_DATA_DIR` o pasa `--data-dir` para usar otra ubicación.

```bash
aicrawl start --daemon      # proceso en segundo plano, registro en <data-dir>/aicrawl.log
aicrawl status
aicrawl stop
```

## Agentes de IA (MCP)

### stdio — recomendado para agentes de escritorio

Apunta el agente al mismo ejecutable:

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

Ejecuta `aicrawl start` (o `aicrawl mcp start`) y usa:

```json
{
  "mcpServers": {
    "aicrawl": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

Herramientas:

- `extract_website_data` — `url`, `query`, `schema` opcional, `output_format`
- `crawl_website` — `url`, `max_pages`, `max_depth`
- `get_page_content` — `url`
- `inspect_website` — `url`
- `search_website` — `url`, `query`
- `get_crawl_result` — `job_id`
- `get_crawl_status` — `job_id`

El contenido de las páginas se trata como **datos no confiables**. La salida de las herramientas lo marca como tal y nunca sigue instrucciones encontradas en páginas web.

## Personas

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

Opciones globales: `--no-color` (también respeta `NO_COLOR`), `--config <path>`, `--data-dir <dir>`.

### Panel

Abre `http://127.0.0.1:8080` después de `aicrawl start`:

- **Overview** — estado, contadores de trabajos/resultados, actividad reciente
- **New Extraction** — URL, consulta, esquema, formato y control de renderizado
- **Crawl Jobs** — estado en vivo, progreso, cancelación y detalle del trabajo
- **Results Explorer** — resultados guardados con visor JSON y copiar/descargar
- **MCP Configuration** — configuraciones stdio/HTTP listas para copiar y lista de herramientas
- **API Keys** — crear/revocar claves (se muestran una vez, se guardan con hash)
- **AI Providers** — proveedores externos opcionales, claves cifradas en reposo
- **Settings** — límites de seguridad efectivos, controles de caché y almacenamiento

### API REST

Todas las respuestas usan `{ "success": true, "data": … }` o `{ "success": false, "error": { "code", "message", "retryable" } }`.

```bash
curl -s http://127.0.0.1:8080/api/v1/health

curl -s -X POST http://127.0.0.1:8080/api/v1/extract \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","query":"Extract the main headings","use_ai":false}'

curl -s -X POST http://127.0.0.1:8080/api/v1/crawl \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com","max_pages":10,"max_depth":1}'
```

Define `[api].require_api_key = true` (o enlaza más allá de loopback) y envía `Authorization: Bearer <key>` o `X-API-Key: <key>`. Las claves se guardan como hashes SHA-256 y solo se muestran una vez.

## IA externa (opcional)

Configura un proveedor en **Panel → AI Providers** o mediante la API:

- Compatible con OpenAI: URL base, modelo, clave de API
- Ollama: URL base (`http://127.0.0.1:11434`), modelo, sin clave

Las claves de API se cifran con ChaCha20-Poly1305. La clave de cifrado proviene de `AICRAWL_SECRET_KEY` (una clave de 32 bytes codificada en base64) o se genera una vez en `<data-dir>/secret.key` con permisos solo para el propietario. Las claves en texto plano nunca se escriben en SQLite, los registros ni el panel.

`aicrawl doctor` informa si hay un proveedor configurado; sin uno, la extracción es determinista y devuelve una advertencia en lugar de fallar.

## Seguridad

- La protección SSRF resuelve DNS y rechaza direcciones loopback, privadas, link-local, multicast y de metadatos en cada salto de redirección.
- Las redirecciones se siguen manualmente (máximo 5) y se vuelven a validar cada vez.
- `robots.txt` se obtiene, se cachea y se aplica por defecto.
- Listas de dominios permitidos/bloqueados, límite de tasa por host, topes de páginas/profundidad, presupuestos de tiempo de rastreo, tiempos de espera y cancelación.
- Los cuerpos de respuesta tienen tamaño limitado, el HTML se sanitiza para extraer texto y se eliminan scripts y estilos.
- Se detectan marcadores de inyección de prompts y el texto de la página se envuelve como datos no confiables antes de cualquier llamada a IA externa.
- Autenticación opcional por clave de API; comparación de hashes en tiempo constante; sin claves en las URL.
- Sin CAPTCHA, autenticación ni elusión de paywalls; sin funciones de sigilo o evasión.

## Almacenamiento

Solo SQLite (`sqlx`, WAL, conexiones agrupadas, sentencias preparadas, migraciones, índices, transacciones). Tablas: jobs, pages, results, websites, settings, API keys, AI providers, page cache, robots cache.

## Compilar en Windows sin Visual Studio

SQLite incluido necesita un compilador de C. Si no tienes MSVC Build Tools, usa el objetivo GNU con MinGW-w64 (este repositorio ignora `.tools/`):

```bash
# una vez: descomprime una cadena de herramientas MinGW-w64 en .tools/mingw64 (gcc, dlltool, ar)
export PATH="$PWD/.tools/mingw64/bin:$PATH"
export CC_x86_64_pc_windows_gnu=gcc
export AR_x86_64_pc_windows_gnu=ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc
export CARGO_TARGET_DIR=target-gnu
cargo +stable-x86_64-pc-windows-gnu build --release
# binario: target-gnu/release/aicrawl.exe
```

El ejecutable de la versión es autocontenido: el panel, las migraciones y SQLite están compilados dentro. Solo la renderización opcional de JavaScript necesita software externo.

## Renderización opcional de JavaScript

Las SPA no se renderizan por defecto. Para renderizarlas:

```bash
cd browser && npm install && npx playwright install chromium
```

Luego pasa `--render` en extract/crawl o activa render en el panel. Sin el puente, AICrawl devuelve un error explícito de “rendering unavailable” en lugar de una página vacía silenciosa.

## Desarrollo

```bash
cargo test --workspace          # pruebas unitarias + de integración (servidor HTTP local)
cd frontend && npm run dev      # Vite en :5173, redirige /api a :8080
cargo run -p aicrawl-cli -- doctor
```

El panel se integra en tiempo de compilación. Durante el trabajo de UI usa el servidor de desarrollo de Vite o define `AICRAWL_DASHBOARD_DIR=frontend/dist` para servir una recompilación desde el disco sin recompilar Rust.

## Versiones y publicación

[`version.json`](version.json) es la única fuente de verdad de la versión de publicación. Determina dos cosas:

- la etiqueta de la versión (`v` + la versión, por ejemplo `v0.0.1-experimental.2`)
- la versión compilada en el binario, que muestran `aicrawl --version`, `aicrawl version` y el servidor MCP

Para publicar una versión:

1. Actualiza `version.json` y el `version` correspondiente en `Cargo.toml`, luego fusiona en `main`.
2. Espera a que pase el flujo automático **CI** en Windows, Linux y macOS.
3. Ejecuta manualmente **Actions → Release → Run workflow**. El flujo crea la etiqueta, compila los cuatro binarios, verifica su versión integrada y los sube con sumas SHA-256.

Las versiones con guion (`-`) se publican como versiones preliminares.

## Limitaciones

- Las páginas con mucho JavaScript requieren el puente Playwright opcional.
- `search_website` busca en una sola página obtenida; usa `crawl_website` para cobertura de todo el sitio.
- La calidad de la extracción semántica depende del proveedor externo configurado.
- AICrawl respeta la política del sitio; no omite autenticación, paywalls ni CAPTCHA.

## Comunidad

- [Guía de contribución](CONTRIBUTING.md)
- [Código de conducta](CODE_OF_CONDUCT.md)
- [Política de seguridad](SECURITY.md)
- [Soporte](SUPPORT.md)
- [Gobernanza](GOVERNANCE.md)
- [Registro de cambios](CHANGELOG.md)
- [Debates](https://github.com/Ahaduzzamankhan/aicrawl/discussions)
- [Informar de un error](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml)
- [Solicitar una función](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml)

Si AICrawl te resulta útil, considera darle una estrella al repositorio; ayuda a que otras personas encuentren el proyecto.

## Licencia

MIT

## Star History

<a href="https://www.star-history.com/?repos=ahaduzzamankhan%2Faicrawl&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=ahaduzzamankhan/aicrawl&type=date&legend=top-left" />
 </picture>
</a>
