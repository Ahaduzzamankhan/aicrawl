# Support

Thanks for using AICrawl. This page explains where to get help and what to
include so your question can be answered quickly.

## Read this first

- [README](README.md) — installation, CLI, dashboard, REST API, MCP and
  configuration.
- [Troubleshooting and limitations](README.md#limitations) — rendering, search
  scope, AI providers and site-policy boundaries.
- [Changelog](CHANGELOG.md) — changes between experimental builds.
- [Security policy](SECURITY.md) — private vulnerability reporting only.

## Where to ask

| Need | Where |
| --- | --- |
| Bug with reproduction steps | [Open a bug report](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=bug_report.yml) |
| Feature or extraction idea | [Open a feature request](https://github.com/Ahaduzzamankhan/aicrawl/issues/new?template=feature_request.yml) |
| Usage question, configuration help, show and tell | [GitHub Discussions](https://github.com/Ahaduzzamankhan/aicrawl/discussions) |
| Security vulnerability | [Private advisory](https://github.com/Ahaduzzamankhan/aicrawl/security/advisories/new) — never a public issue |
| Contribution guidelines | [CONTRIBUTING.md](CONTRIBUTING.md) |

## What to include

- AICrawl version (`aicrawl version`) and how you run it (CLI, `start`, daemon,
  API, MCP).
- Operating system and install method (release binary or source build).
- The exact command or HTTP request, with secrets and private data removed.
- Expected versus actual behavior.
- Relevant logs. Redact URLs, crawled content, API keys and tokens.
- For rendering issues: Node.js version and whether Playwright Chromium is
  installed.

## Response expectations

AICrawl is maintained on a best-effort basis. There is no SLA. Well-scoped bug
reports with reproductions and tests are prioritized. Security reports follow
the timelines in [SECURITY.md](SECURITY.md).

## Not supported

- CAPTCHA solving, paywall bypass, authentication circumvention or stealth
  crawling.
- Crawling content you do not have permission to access.
- Support for modified builds or machine-specific toolchain patches.
- Guaranteed compatibility with every site's HTML or anti-bot configuration.

## Commercial and private support

There is no commercial support offering. If your organization needs private
help, open a Discussion describing the scope so maintainers can decide whether a
public guide or a paid arrangement makes sense.
