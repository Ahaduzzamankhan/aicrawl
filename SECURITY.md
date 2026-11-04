# Security Policy

AICrawl fetches untrusted web content, stores crawl data and exposes local
REST/MCP servers (plus an optional local rendering bridge). Security reports are
taken seriously, and responsible disclosure is appreciated.

## Supported versions

| Version | Supported |
| --- | --- |
| `1.0.x` | :white_check_mark: |
| `0.0.1-experimental.x` and older | :x: |

Only the latest published `1.0.x` build receives security fixes. The 0.0.1
experimental builds shipped an AI provider registry that no longer exists.

## Reporting a vulnerability

**Do not open a public issue for a vulnerability.**

Use GitHub's private vulnerability reporting:

- <https://github.com/Ahaduzzamankhan/aicrawl/security/advisories/new>

If that flow is unavailable, open a minimal public issue asking the maintainer
for a private channel. Do not include exploit details, secrets or affected
third-party data in the public issue.

Please include:

- Affected AICrawl version, commit or release asset.
- Operating system and how AICrawl was started (CLI, API, MCP, daemon).
- A clear description of the impact and the security boundary crossed.
- Reproduction steps, configuration and a minimal proof of concept.
- Any suggested fix or mitigation, if you have one.
- Whether you want public credit, and under what name.

## What to expect

- Acknowledgement within **5 business days** (best effort).
- Initial triage within **10 business days**.
- Coordination on a fix and disclosure timeline before any public write-up.
- Credit in the advisory and release notes unless you prefer to stay anonymous.

This is a volunteer-run, experimental project. There is **no bug bounty** and no
guaranteed response time, but reports are read and triaged.

## In scope

- SSRF, DNS rebinding or redirect-validation bypasses.
- `robots.txt`, domain policy or rate-limit enforcement bypasses.
- Authentication or authorization bypasses in the REST API or dashboard.
- Leakage of API keys, dashboard tokens or crawl data.
- Stored XSS, CSRF or other injection in the embedded dashboard.
- Prompt-injection content escaping the untrusted-content wrapper.
- Path traversal, SQL injection, unsafe deserialization or command injection.
- Supply-chain or build issues in code maintained by this repository.
- Remote denial of service that is reachable with the default configuration.

## Out of scope

- Vulnerabilities in third-party websites, services or local rendering tools.
- Requests to bypass CAPTCHAs, paywalls, authentication or anti-bot systems.
  AICrawl deliberately does not implement these features.
- Missing security headers without a demonstrated impact.
- Reports that require an already compromised host, browser extension or
  administrator account.
- Social engineering, spam or physical attacks.
- Automated scanner output without a reproducible, AICrawl-specific finding.
- Issues in unsupported or modified builds.

## Safe harbor

When you act in good faith, avoid privacy violations and data destruction, and
give the maintainers reasonable time to fix the issue before disclosure, we will
not pursue legal action or report you to platform providers for your research.
Do not test against systems you do not own or lack permission to test.

## Disclosure policy

- Please keep the report private until a fix or mitigation is available.
- Maintainers will coordinate a disclosure date with the reporter.
- Published advisories will describe the impact, affected versions, fix and
  credit, without exposing unrelated user data.
- If a report is declined as out of scope, the reasoning will be documented
  privately.

## Hardening checklist for operators

- Bind the API and MCP servers to `127.0.0.1` unless you have added network
  controls and API-key authentication.
- Set `[api].require_api_key = true` before binding beyond loopback, and store
  the generated key in a secret manager.
- Keep `allow_private_networks = false` and review domain allow/deny lists.
- Run the latest release and review the changelog before upgrading.
- Never publish `.aicrawl/`, `secret.key`, SQLite databases or logs containing
  crawled content.
