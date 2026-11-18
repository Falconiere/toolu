# Grok plugin hooks for epic workers

**Date:** 2026-11-17   **Status:** Approved   **Topic:** Toolu gates for Grok Build worker sessions

## Problem

[PR #847](https://github.com/Falconiere/toolu/pull/847) added Grok routing, exact session resume, resource accounting, and live Herdr lifecycle evidence. Grok starts toolu plugins with lifecycle sources the shared startup matchers missed. The raw Grok terminal tool name also misses the engine's shell gates. [Issue #846](https://github.com/Falconiere/toolu/issues/846) calls for the delivery workflow to run under those gates.

## Non-goals

1. Keep the worker lifecycle, routing, and usage transport from #847.
2. Run the hook verification in a local project with a bare Git remote. The implementation PR is the only GitHub delivery for this change.

## Architecture and interfaces

The global `--host grok` resolves a Grok profile from `GROK_HOME` or `~/.grok`. Hook processes identify themselves through `GROK_HOOK_EVENT`. Commands from an agent shell identify themselves through `GROK_AGENT=1`. Keep project config and plan ledgers in `.claude/` so existing worktrees resume with their original state.

Use the existing Claude-compatible plugin manifests and exit/JSON contracts. Match Grok's `new` and `load` startup sources and canonicalize its tool names before event dispatch. Use Grok's native marketplace installation to activate hooks. The installer verifies the official toolu source before trusting it and protects foreign installations during bare-name updates and removals.

## Failure modes and boundaries

Malformed Grok tool payloads fail closed before execution. Missing or foreign marketplace metadata stops a native plugin mutation. Duplicate installed names prevent a bare-name update or removal. Grok treats startup output as passive, so the worker brief supplies delivery instructions and tool hooks return gate feedback.

## Acceptance criteria and evidence

- **AC-1:** The captured Grok terminal call becomes a shell event and `toolu --host grok` detects the correct profile. Check the real fixture and the worker ledger tests. Existing project ledgers remain readable.
- **AC-2:** In an isolated Grok Build 1.0.77 profile, `toolu plugins install delivery-flow --host grok` installs the five worker dependencies. `toolu --host grok doctor` finds them. Native update and removal succeed for toolu-owned plugins. The marketplace parser distinguishes an unrelated local source.
- **AC-3:** In real Grok calls with the strict preset, a denied protected write leaves `.env` unchanged. A TypeScript edit records failing quality state. The quality guard rejects the commit and push review rejects a push to a bare local remote. The structured tool transcripts show each refusal and the disk and Git refs confirm the outcomes. Startup registrations appear in the isolated Grok profile. An exact headless resume republished git-better’s semantic rule after its manifest was removed from the scratch profile. The session directory set stayed the same. The Herdr start, resume, cancel, and teardown observations are recorded in the [host evidence](../evidence/epic-hosts-v1.json).
- **AC-4:** Generated CLI help, the contract snapshot, and the installer and worker guides agree on Grok support. Run the docs, hooks, and workspace gates.

## Open questions

The requested verification runs locally. GitHub CI and review of the implementation PR can run after local verification.
