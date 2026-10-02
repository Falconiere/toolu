# OpenCode full catalog surface — Brainstorm

**Outcome:** Generate a deterministic OpenCode catalog for all 16 plugin manifests. Every generated skill has a valid name and description; agents and commands use supported OpenCode frontmatter and point to available generated resources.

## Evidence

- Issue #344 requires all 16 plugins, valid 1–64 character skill names, portable agent and command schemas, relative resources, and typed reference rewrites.
- The current generator selects only the default plugin closure (six plugins), uses `--` IDs, flattens nested YAML, and rewrites only `CLAUDE_PLUGIN_ROOT`.
- The pinned host contract (#335, `opencode-ai@1.18.34`) records file surfaces as supported but host validation of skill names as unsupported. The official [skills](https://opencode.ai/docs/skills/), [agents](https://opencode.ai/docs/agents/), and [commands](https://opencode.ai/docs/commands/) docs define their respective schemas.
- Source inventory has 18 skills, five agents, and five commands. Three plugins have no Markdown surface; quality-only plugins still need explicit catalog entries.
- Jev favored a complete catalog with explicit empty entries over retaining a selected-only catalog (0.80), and typed agent mapping over raw Claude aliases or dropping controls (0.96). These judgments informed the design; the contract and tests remain the authority.

## Decisions

1. Generate the committed catalog from all manifests. Keep `--enabled` as a scoped generation/testing option; runtime plugin selection remains a separate concern.
2. Derive lowercase, single-hyphen IDs from plugin and source names. Resolve collisions and overlength names with a deterministic hash suffix. Assign IDs over the entire catalog, not one plugin at a time.
3. Parse real YAML into structured values. Emit only supported fields per kind. Require a nonempty skill description; synthesize command descriptions from their headings when absent.
4. Map Claude agent tool lists to OpenCode permission keys with restrictive allowlists and `mode: subagent`. Omit Claude-only `opus`/`sonnet`/`haiku` aliases so the host inherits its configured model.
5. Resolve explicit skill and command references from the generated ID manifest. Copy referenced local resources and make command wrappers load their generated skills. Preserve shell variables and relative resource links that still point to copied resources.

## Trade-offs and risk

- A complete catalog increases generated files but makes absence visible and drift checkable. It does not enable every plugin at runtime.
- Host-specific Claude workflows inside source skills are owned by later epic packages. This issue ensures their files and explicit references are resolvable, while later packages adapt behavior and installation.
- Markdown prose can contain path examples rather than executable references. Rewrite only recognized typed references and host-root tokens; record unresolved host-specific literals for later porting.

**Handoff:** Write the design spec, then review it before planning implementation.
