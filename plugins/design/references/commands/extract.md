# extract — Build

**Invocation:** `/design extract [target]`   **Status:** active

## Purpose
Pull repeated patterns, components, and values out of an existing UI into a consistent design system. `extract` finds the duplicated buttons, the near-identical cards, and the one-off colors and spacings that should have been variables, then consolidates them into named tokens and shared components and points the call sites at them. The outcome is a single source of truth that removes drift and speeds future work — refactoring toward the system the code already implies, not redesigning it.

## Flow
1. **Setup.** Run the dispatcher Setup in `SKILL.md`: `detect-stack`, load `PRODUCT.md`/`DESIGN.md` (Build command — the PRODUCT.md gate applies; if missing, run `init` first), and read the real styles/components so consolidation preserves the existing identity instead of normalizing it away.
2. **Scan for duplication.** Across the target, collect repeated and *near-duplicate* values — colors that differ by a hair, ad-hoc spacings off the rhythm, redundant type sizes — and structurally repeated markup (the same button, the same card laid out five ways). Near-duplicates are the real signal: they mark a token that drifted.
3. **Consolidate into named tokens.** Cluster each family and name one token per role: color in OKLCH with verified contrast pairs per `accessibility.md`, spacing snapped onto the 8pt scale, a single modular type scale, radii and elevation rules per `principles.md`. Choose the canonical value deliberately where a cluster disagrees — don't average it.
4. **Extract shared components.** Promote the repeated structures into reusable primitives with variants, so one button/card/field definition replaces the copies. Apply the design law in `SKILL.md` as you go — this is the moment to retire any Absolute Ban baked into the duplicates rather than enshrine it in a token.
5. **Rewrite the call sites.** Replace the inline literals and forked markup with references to the new tokens and components, leaving the rendered result unchanged. This is a refactor — semantics and behavior hold steady while the source converges.
6. **Update DESIGN.md.** Record the extracted tokens and components as the system of record; delegate the file's format and generation to the `document` command rather than duplicating its logic, per `context.md`.
7. **No visual verification.** There is no browser or screenshot here — `extract` reasons over code, so it cannot confirm the consolidated values render identically. List the screens and components the user should compare before and after to confirm nothing shifted.

## Cites
- `principles.md` — 8pt spacing rhythm, modular type scale, radii/elevation rules for token roles.
- `context.md` — `DESIGN.md` skeleton, location precedence, inline OKLCH palette guidance.
- `accessibility.md` — contrast thresholds to verify each consolidated color pair against.

## Output
A set of named tokens (OKLCH for color) and shared components, call sites rewritten to reference them, and an updated (or newly created, via `document`) `DESIGN.md` — plus a before/after list for the user to confirm the render is unchanged.
