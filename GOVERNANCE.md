# Governance

AICrawl is an open-source, maintainer-led project. This document describes how
decisions are made, how releases happen and how contributors can take on more
responsibility.

## Project scope

AICrawl exists to provide one executable for permitted, policy-respecting web
crawling and structured extraction, usable by humans, scripts, REST clients and
AI agents through MCP.

The project will not add features for CAPTCHA solving, paywall or authentication
bypass, stealth/evasion, or crawling that ignores site policy. Changes that
break the single-binary design or duplicate the shared engine across surfaces
need an accepted design discussion first.

## Roles

| Role | Responsibilities |
| --- | --- |
| Users | Report bugs, request features, follow the Code of Conduct and site policies. |
| Contributors | Submit issues and pull requests, review code, improve docs and tests. |
| Maintainers | Triage, review, merge, manage releases, security reports and repository settings. |
| Lead maintainer | Final tie-breaker, release approval and security coordination. |

The current maintainer is listed in [CODEOWNERS](.github/CODEOWNERS).

## Decision making

- **Small changes** (bugs, docs, tests, focused refactors) use lazy consensus:
  a pull request with review from a maintainer can merge.
- **Substantial changes** (new surfaces, storage schema, security model,
  breaking CLI/API changes) start as a GitHub Discussion or issue with a short
  design note covering motivation, alternatives, migration and tests.
- **Security decisions** may be handled privately by maintainers and the
  reporter until disclosure.
- Maintainers aim for consensus. If consensus is not reached, the lead
  maintainer decides and documents the reasoning.
- Reverted changes are allowed when they introduce regressions, security risk or
  unacceptable maintenance cost.

## Pull request requirements

- Tests pass in CI (`cargo test --workspace`, frontend typecheck and build).
- Public behavior is documented in the README or changelog.
- Security-sensitive changes are called out explicitly.
- Commits are focused and use imperative subjects.
- The contributor agrees to license the work under the MIT License.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow.

## Releases

AICrawl uses Semantic Versioning with an experimental pre-release suffix, for
example `0.0.1-experimental.2`. Before `1.0.0`, breaking changes may appear in
minor or experimental builds and are documented in the changelog.

Release process:

1. Update versions and [CHANGELOG.md](CHANGELOG.md).
2. Run the full test suite and build the frontend.
3. Merge to `main`, wait for CI to pass.
4. Create a signed or annotated `v*` tag.
5. The release workflow builds the Windows executable, generates a SHA-256
   checksum and publishes release notes.
6. Verify the downloaded artifact (`aicrawl version`) before announcing.

Experimental builds are marked as pre-releases on GitHub.

## Becoming a maintainer

Contributors who consistently submit high-quality changes, review constructively
and help with triage may be invited as maintainers. There is no fixed quota.
A maintainer nomination is discussed privately; the lead maintainer confirms the
decision. Maintainers can step down at any time.

## Code of Conduct

All project spaces follow the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
Enforcement is handled by maintainers. Reports are kept confidential.

## Security

Vulnerabilities are handled under [SECURITY.md](SECURITY.md). Do not discuss
unfixed vulnerabilities in public issues or discussions.

## Changes to this document

Governance changes are proposed as a pull request and require approval from a
maintainer. Substantial changes should be announced in Discussions.
