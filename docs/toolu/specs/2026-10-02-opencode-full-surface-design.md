# OpenCode full catalog surface — Design

**Date:** 2026-10-02   **Status:** Approved   **Author:** Codex   **Topic:** OP-10 generated skills, agents, and commands

## Problem

The committed OpenCode catalog contains six of 16 plugins and skill names with consecutive hyphens. Its scalar YAML handling loses structured values, and generated agents carry Claude model and tool values that the pinned OpenCode host does not understand. Explicit workflow references and host paths can point away from generated resources.

## Non-Goals

1. Native installation and runtime selection of generated surfaces; OP-11 owns it.
2. Porting the content of host-specific workflows and leaf plugin behavior; their epic work packages own it.
3. Changing Claude Code or Codex source skills and agents.

## Architecture

The generator scans all plugin manifests by default, while `--enabled` remains a focused subset option. It builds a typed source index, assigns unique IDs globally, renders kind-specific Markdown, then emits a catalog entry for every selected plugin. Plugins without Markdown sources are explicitly marked `no-surface` with a reason. The statusline setup command is classified as host-specific because it edits Claude Code `settings.json`; OP-25 owns its OpenCode equivalent.

Names are slugged to lowercase ASCII and single hyphens. The base is `<plugin>-<local-id>`; collisions and names longer than 64 characters get a stable SHA-256-derived suffix within the length limit. Repeated identical source keys fail rather than overwrite an artifact.

Parse YAML with the pinned Bun 1.4 parser. Serialize supported nested values as JSON-compatible YAML flow values to preserve types and quote special characters. The generator validates the output shape before writing. Skill frontmatter retains supported metadata and requires a 1–1024 character description. Agent frontmatter uses `mode: subagent`, maps Claude tool lists to an explicit restrictive OpenCode permission map, and omits unqualified Claude model aliases. Command frontmatter has a description and only supported optional agent/model fields. A command with a matching skill becomes a thin native skill-tool wrapper.

Use the source index as a typed reference manifest: explicit `$plugin:local`, `/plugin:local`, and known source skill/command paths resolve to generated IDs/paths. Copy local skill resources, including linked files outside `references/` and `scripts/`, into the generated tree and rewrite their relative links. Rewrite plugin-root tokens to the OpenCode root. Record other host-specific prose in generated notes for owning work packages.

## Interfaces / Schema

- `assignSurfaceIds(candidates: ArtifactCandidate[]): Map<candidateKey, id>` assigns IDs across all selected plugins. IDs match `^[a-z0-9]+(-[a-z0-9]+)*$`; skill IDs have length ≤64.
- `parseFrontmatter(source)` returns a nested YAML record, body, and stripped key list; absent frontmatter is represented by an empty record. Invalid YAML fails with source context.
- `rewriteBody(body, references)` accepts a typed index of known surface IDs; returns rewritten body and diagnostics. Only known invocation/path tokens are changed.
- `opencode.toolu.json` keeps `version: 1`, `surfaceRoot`, and `plugins[]`; each plugin has `skills`, `agents`, `commands`, plus `classification: "generated" | "no-surface"` and optional `reason`. Excluded host-specific source paths carry an explicit reason.
- `SKILL.md` uses `name`, `description`, optional `license`, `compatibility`, and string-map `metadata`. Agent Markdown uses description, mode, permission, and compatible optional fields. Command Markdown uses description and compatible optional fields.

## Failure modes and edge cases

- Invalid, missing, or overlong skill descriptions, invalid YAML, duplicate source keys, unsupported agent tool names, and unresolved explicit references fail generation with source context; no partial committed tree is written.
- IDs remain deterministic across source order, plugin order, collisions, case differences, and 64-character boundaries. The hash suffix fits within 64 characters.
- Empty plugins get an explicit catalog classification. A host-specific source excluded from generation has a reason and owner.
- Existing relative links to copied local resources remain valid. Missing local link targets fail generation. External or deliberately host-specific prose is listed in notes; it is not silently rewritten to an invented path.

## Acceptance criteria

- **AC-1:** Given the actual 16-plugin tree, generation emits 16 catalog entries, every portable source contribution, and explicit no-surface classifications for empty plugins.
- **AC-2:** Given real sources and synthetic collision/length cases, every generated skill has a valid 1–64 character name matching its directory and a 1–1024 character description; IDs are unique and stable under reordering.
- **AC-3:** Given real agent/command Markdown and nested YAML fixtures, generated frontmatter parses on the pinned OpenCode host, retains structured metadata/permissions, excludes Claude-only aliases, and restricts agents to mapped tools.
- **AC-4:** Given real linked skill resources and explicit cross-surface references, generated links resolve within the output tree, command wrappers load existing generated skills, and no unresolved known Claude invocation remains.
- **AC-5:** Given an unchanged tree, the drift check passes; source edits, missing resources, or invalid frontmatter cause a failing check with a diagnostic. The full `bun run test` gate passes.

## Acceptance evidence

| AC | Real input and expected result | Boundary/failure case | Runnable check |
|---|---|---|---|
| AC-1 | `plugins/*` manifests produce 16 catalog entries and known source counts | Empty quality plugins are classified, statusline setup is explicitly excluded | `bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts` |
| AC-2 | Generated `SKILL.md` files pass documented regex, length and directory match | Synthetic colliding and 64+ character inputs stay stable | same targeted test; `bun run check:opencode-surface` |
| AC-3 | Generated real agents and commands load as Markdown on pinned `opencode-ai@1.18.34` | Nested permission/metadata fixtures and unknown tools fail safely | targeted test plus isolated real-host subprocess |
| AC-4 | Actual delivery-flow, epic, babysit, and toolu references resolve to generated files | A removed resource or unresolved explicit reference fails | targeted test and link-resolution check |
| AC-5 | Committed generated tree equals fresh plan | Changed source produces drift failure | `bun run check:opencode-surface`; `bun run test` |

## Documentation impact

Update `tools/toolu-opencode/README.md` and generated notes with catalog scope, naming, schema mapping, explicit exclusions, and the distinction between generation and native installation. Keep the host contract matrix consistent with the new surface counts.

## Open Questions

None. The issue and pinned host contract settle the generation boundary; later epic issues own native installation and workflow ports.
