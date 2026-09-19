---
name: verify-dual-host-plugins
description: Verify toolu plugin packaging, startup context, and REST wrappers on Codex and Claude Code. Use when adding or reviewing dual-host plugins.
metadata:
  toolu:
    origin: agent
    created: 2026-09-19T02:03:05Z
---
## When to Use

Use when adding or reviewing a toolu plugin that must install and deliver working
startup behavior in both Codex and Claude Code.

## Procedure

1. Recall relevant repo decisions. Compare the branch against its base, including
   untracked files, and inspect both manifests, marketplace entries, release
   configuration, hooks, skills, and docs.
2. Read the service's live API documentation and the hosts' hook contracts.
   Distinguish installing a skill from injecting mandatory session instructions.
3. Run the plugin's colocated tests. For REST wrappers, exercise real curl against
   a private loopback HTTPS fixture; check request JSON, retries, output
   validation, and error propagation.
4. Install from the checkout with both actual plugin CLIs in temporary,
   separately configured profiles. Invoke hook commands from the installed
   caches, including a path containing spaces. Verify host-specific published
   paths and startup context with and without credentials.
5. Run `bun run test`, the deterministic benchmark and its validation, and the
   colocated-test check from `.github/workflows/tests.yml`. Inspect every result.

## Pitfalls

- A skill description does not guarantee startup delivery. Standalone plugins
  should publish their own bounded context without relying on hook ordering.
- Codex installation does not grant hook trust. Do not bypass the user's trust
  controls or claim that manually invoking a hook verifies a full agent session.
- Offline fixtures prove transport behavior, not live inference quality. Report
  unavailable credentials explicitly; do not read keys from `.env`.
- Packaging and Codex smoke assertions contain explicit plugin/skill/hook counts.
  Update relevant counts together when adding a plugin.
- Keep host overrides scoped to the operation being verified. A Codex override
  inherited by the full suite redirects Claude fixtures away from their temporary
  configuration directories. Unset it in the test subprocess when running a
  Codex-scoped plan ledger.

## Verification

Require passing packaging, targeted regressions, and CI commands; successful
isolated installation on both hosts; and observed cached-hook output. Report
live-service and full-session checks separately. Keep user profile configuration
out of fixture cleanup and preserve unrelated workspace changes.
