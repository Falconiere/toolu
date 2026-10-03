# OpenCode exa-search helper and research workflow

**Date:** 2026-10-03   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-03-opencode-exa-search-design.md   **Topic:** Issue #349 native helper and skill

## Evidence and approach

Issue #349 requires native helper discovery, real HTTPS calls, a no-key fallback, and deselection cleanup. The merged OP-08/OP-11 substrate already executes selected SessionStart entries, delivers their context, publishes helpers, and removes owned contributions. Comemory decision `5a002d41` and `plugins/agent-browser/hooks/src/session-start.ts` show the leaf-owned context pattern. `plugins/exa-search/hooks/src/search.ts` already implements all three commands with `@toolu/core/rest`; its loopback HTTPS tests cover the transport contract. The generated OpenCode skill still carries Codex/Claude paths. Reuse these paths and add exa-specific live evidence.

## Workstream summary

Prove the current gap, add leaf startup context and generated skill guidance, exercise the installed helper and lifecycle on the pinned host, then synchronize docs and run the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "startup",
    "title": "Publish OpenCode-only helper and credential guidance",
    "ac_refs": ["AC-1", "AC-3", "AC-4"],
    "input": "Run the real exa-search SessionStart launcher with OpenCode and existing-host environments, with and without EXA_API_KEY",
    "check": "bun test plugins/exa-search/hooks/src/__tests__/session-start.test.ts && bun run check:plugin-bundles"
  },
  {
    "id": "skill",
    "title": "Generate native OpenCode exa-search instructions",
    "ac_refs": ["AC-1", "AC-3", "AC-4"],
    "depends_on": ["startup"],
    "input": "The source exa-search skill and generated exa-search-exa-search skill, including all three command examples",
    "check": "bun run generate:opencode-surface && bun run check:opencode-surface && bun test tools/toolu-opencode/scripts"
  },
  {
    "id": "live",
    "title": "Prove installed helper, HTTPS transport, fallback, and disable on pinned OpenCode",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "depends_on": ["skill"],
    "input": "Isolated pinned OpenCode profile with a selected exa-search plugin, scripted provider, published helper, and loopback HTTPS Exa fixture; repeat without key and after deselection",
    "check": "bun test plugins/exa-search/hooks/src/__tests__/search.test.ts && bun run smoke:opencode-entry exa.enabled exa.transport exa.no-key exa.disabled"
  },
  {
    "id": "docs",
    "title": "Document OpenCode helper path and research fallback",
    "ac_refs": ["AC-1", "AC-3", "AC-4"],
    "depends_on": ["live"],
    "input": "OpenCode installation documentation and its generated package copy",
    "check": "bun run generate:opencode-surface && bun run check:opencode-surface"
  },
  {
    "id": "gate",
    "title": "Run the full repository quality gate",
    "ac_refs": ["AC-2", "AC-4"],
    "depends_on": ["docs"],
    "input": "Complete branch diff, committed bundles and generated OpenCode package",
    "check": "bun run test"
  }
]
```

## Critical files

- `plugins/exa-search/hooks/src/session-start.ts` and its colocated test and committed bundle
- `tools/toolu-opencode/scripts/lib/rewrite.ts` and its tests
- `tools/toolu-opencode/generated/skills/exa-search-exa-search/SKILL.md`
- `tooling/src/opencode-host/scenarios-exa.ts` and `tooling/src/opencode-entry-smoke.ts`
- `docs/opencode.md` and its generated package copy

## Verification

The live pinned host must discover the skill and run the published symlink, not only invoke source functions. A private loopback HTTPS fixture must observe correct search, contents, and findSimilar requests, including a success, HTTP 401, malformed JSON, and dropped connection; the key is a sentinel whose bytes must never appear in startup or errors. No-key and disabled sessions must show the fallback and cleanup. Existing-host SessionStart tests, surface and bundle drift, and `bun run test` must pass before delivery. The steps omit scoped `paths`, so ledger freshness covers the whole branch diff.

After implementation: run delivery preflight, commit the scoped changes, verify every ledger step against the final diff, run `toolu-review:review` and require a ready verdict, then push and open a verified `main` PR for #349. Handoff to `pr-babysit:babysit` and report ready only after its success stop.
