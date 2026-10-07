# Epic engine config and secrets — Brainstorm

**Date:** 2026-10-06  **Issue:** #463  **Mode:** Delivery, full

## Outcome

The epic engine has one typed source for settings and secrets. Non-secret settings live under `epic` in `toolu.config.json`; status, notification and peer credentials live in `<config>/toolu/secrets.json` or the environment. `toolu epic token new` rotates the status token. Public output paths get a redacted snapshot rather than raw credentials.

## Evidence and decisions

- #414 already added an open `epic` namespace to the Rust envelope and TypeScript strict schema, with a shared fixture proving that unknown top-level keys still fail closed. Preserve it.
- `toolu-runtime` owns `Env`, `Roots`, the config loader and host-independent file access. The epic plugin is a placeholder; #445, #434 and #449 own config/doctor, journal and HTTP output. Put the common secret and redaction contract in `toolu-runtime` and the token command in `toolu-epic-orchestrator` (Jev: 0.96 and 0.98 for placement and scope).
- Use `TOOLU_EPIC_STATUS_TOKEN` as the primary status environment variable, with #449's older `TOOLU_EPIC_TOKEN` as an alias only when the primary is absent (Jev: 0.86). Environment values override the file.
- Treat every notification URL as secret. A credential may be embedded in a path or query that a generic URL parser cannot safely identify. Jev was split between unconditional secrecy and classifying URLs (0.46 versus 0.54, low confidence); the issue's no-leak requirement decides the safer rule. The `epic` section holds notification enablement only.

## Alternatives and boundaries

Putting secrets in the epic plugin would force future toolu management verbs to depend on a plugin crate across the layer rule. Putting them in the CLI would leave engine consumers without the same reader. Implementing #445's management verbs, #434's journal and #449's HTTP page here would preempt their contracts. Their owners must use the shared redacted view when those surfaces land; this PR tests the serializer with real canary values.

The secret file is user-only, and unsafe permissions, symlinks, malformed JSON and invalid values fail closed. Token writes replace the file atomically and preserve other fields. A consumer must resolve secrets on use so rotation invalidates the old token without a process restart.

## Risk and handoff

The main risks are file permission races, accidental display through `Debug` or error text, and dependent issues inventing a second secret path. The spec will pin the file and environment schema, error behavior and real-data checks; docs will tell later owners which API to call.
