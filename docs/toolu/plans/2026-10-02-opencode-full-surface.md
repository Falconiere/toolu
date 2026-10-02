# OpenCode full catalog surface — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-full-surface-design.md   **Topic:** OP-10 valid catalog generation

## Evidence and approach

Issue #344, the approved spec, the pinned OP-01 host contract, and the real `plugins/*` tree establish the requirements. The existing generator in `tools/toolu-opencode/scripts/` and its colocated tests provide the narrowest implementation path. Generate all manifests by default, assign safe IDs globally, render structured kind-specific frontmatter, and use a typed source index for references and resources. The later installation and behavior-port issues remain separate.

## Workstream summary

ID/schema validation → full catalog and resource/reference generation → pinned-host and drift evidence → docs and full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Assign valid stable IDs and render structured frontmatter",
    "ac_refs": ["AC-2", "AC-3"],
    "paths": ["tools/toolu-opencode/scripts", "plugins", "docs"],
    "input": "Actual skill and agent frontmatter plus synthetic collision, length, and nested YAML cases",
    "check": "bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts --test-name-pattern '^(surface ids obey|frontmatter round)'"
  },
  {
    "id": "S2",
    "title": "Generate all plugin entries and resolve references and resources",
    "depends_on": ["S1"],
    "ac_refs": ["AC-1", "AC-4"],
    "paths": ["tools/toolu-opencode/scripts", "plugins", "docs"],
    "input": "All 16 plugin manifests, including linked workflows and three empty plugins",
    "check": "bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts --test-name-pattern '^(generate twice|drift check fails|toolu surface ids|generated skill resources|commands with matching|full catalog|resource links|missing real skill resource)'"
  },
  {
    "id": "S3",
    "title": "Regenerate committed tree and verify host parsing and drift",
    "depends_on": ["S2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "paths": ["package.json", "plugins", "docs", "tools/toolu-opencode/generated", "tools/toolu-opencode/scripts", "tooling/src/opencode-surface-probe.ts", "tools/toolu-opencode/contract/pin.json", "tooling/src/opencode-host"],
    "input": "Fresh isolated OpenCode 1.18.34 profile and generated Markdown tree",
    "check": "bun run check:opencode-surface && bun run probe:opencode-surface && bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts"
  },
  {
    "id": "S4",
    "title": "Document classifications and pass full repository gate",
    "depends_on": ["S3"],
    "ac_refs": ["AC-1", "AC-5"],
    "paths": ["tools/toolu-opencode/README.md", "tools/toolu-opencode/generated/GENERATED-NOTES.md", "tools/toolu-opencode/contract/capability-matrix.json", "docs/opencode-host-contract.md", "tools/toolu-opencode/scripts", "tools/toolu-opencode/generated"],
    "input": "Regenerated catalog and synchronized capability matrix",
    "check": "bun run check:opencode-host && bun run test"
  }
]
```

## Critical files

`tools/toolu-opencode/scripts/lib/{ids,frontmatter,rewrite,scan-surface,emit}.ts`, `generate-surface.ts`, its colocated tests, `generated/`, `contract/capability-matrix.json`, `tooling/src/opencode-surface-probe.ts`, `package.json`, `tools/toolu-opencode/README.md`, and `docs/opencode-host-contract.md`.

## Verification

Targeted tests cover real sources and synthetic boundaries, including malformed YAML, collisions, missing links, empty plugins and unsupported agent tools. An isolated pinned OpenCode subprocess parses generated agents and commands. `bun run check:opencode-surface` proves no generated drift, `bun run check:opencode-host` keeps the capability matrix synchronized, and `bun run test` covers the repository gate. Generated notes and the README state classifications and later-installation boundary.

Delivery follows the active `delivery-flow` procedure: final ledger verification, scoped commit, pre-push review and verdict, push, PR verification, then babysit. The worker brief forbids merging.

## Deviations

- S1 and S2 use targeted name patterns because the original full-file runner includes the committed-tree drift assertion that can only pass after S3 regeneration. S3 runs the complete test file. The new `render.ts` is declared in S2's paths.
