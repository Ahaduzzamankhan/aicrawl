## Summary

<!-- Describe the change and the problem it solves. Explain the "why", not only the "what". -->

## Related issues

<!-- Use "Fixes #123" or "Refs #123". Remove this section if not applicable. -->

## Type of change

- [ ] Bug fix (non-breaking change)
- [ ] New feature (non-breaking change)
- [ ] Breaking change (CLI, API, MCP, storage or configuration)
- [ ] Documentation
- [ ] CI, build or dependency update
- [ ] Security hardening

## Affected surfaces

- [ ] CLI
- [ ] Dashboard
- [ ] REST API
- [ ] MCP server
- [ ] Core engine / extraction
- [ ] Storage / migrations
- [ ] Documentation or repository tooling

## Testing

<!-- List the commands you ran and their results. Include manual checks for dashboard changes. -->

```text
cargo test --workspace
cd frontend && npm run typecheck && npm run build
cargo run -p aicrawl-cli -- doctor --offline
```

## Checklist

- [ ] I read [CONTRIBUTING.md](../CONTRIBUTING.md) and agree to the [Code of Conduct](../CODE_OF_CONDUCT.md).
- [ ] My change is focused and does not include unrelated formatting or dependency updates.
- [ ] Tests cover new behavior, or I explained why a test is not practical.
- [ ] Public behavior and configuration are documented.
- [ ] I updated [CHANGELOG.md](../CHANGELOG.md) for user-visible changes.
- [ ] No secrets, API keys, database files, build logs or `frontend/dist` are committed.
- [ ] Security implications were considered; vulnerabilities are reported privately, not in this PR.
- [ ] The embedded dashboard still works after a source build.

## Screenshots or recordings

<!-- Required for dashboard/UI changes. Drag images here. -->

## Additional context

<!-- Migration notes, rollout concerns, follow-up work, or anything reviewers should know. -->
