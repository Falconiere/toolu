# polish — Refine

**Invocation:** `/design polish [target]`   **Status:** active

## Purpose
The final pre-ship pass that turns "done" into "considered." It sweeps the target for the small inconsistencies that separate a careful product from a rushed one — alignment off by a pixel, spacing that breaks rhythm, mismatched radii or shadows, inconsistent label casing, and stray micro-details. It does **not** redesign and it does not add scope; it tightens what is already there so the surface reads as intentional everywhere the eye lands. The goal is fewer differences, not new ideas.

## Flow
1. **Setup.** Run the standard workflow from `SKILL.md`: detect-stack (`"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/detect-stack.sh" --json .`) to scope platform, load `PRODUCT.md`/`DESIGN.md` (run `init` first if `PRODUCT.md` is missing — polish is generative), pick the register, and open the existing CSS/tokens/theme so you tighten *toward* the established system rather than inventing a new one.
2. **Inventory the values.** Collect the actually-used spacing steps, radii, shadow definitions, font sizes/weights, and color tokens across the target. Differences that should be one value but are several are the raw material of this pass.
3. **Sweep for inconsistency against the grid and tokens.** Snap spacing to the 8pt grid (4 for fine detail); collapse near-duplicate radii to a single ceiling (12–16px for cards); reduce shadow variants to the system's defined elevations; align edges and baselines that drift by a pixel or two.
4. **Normalize labels and casing.** One casing convention for buttons, headings, and nav; consistent terminology for the same action; matching punctuation in helper text and placeholders.
5. **Re-check the law (`SKILL.md`).** Confirm the pass introduced no Absolute Bans — especially over-rounding and ghost-cards (a 1px border *and* a wide soft shadow on one surface) — and that body contrast still clears 4.5:1 after any color nudge.
6. **Confirm the result.** No live-browser or screenshot is available — reason over the code, then state which spacing/radius/shadow values changed and ask the user to confirm the rendered result reads as intended.

## Cites
`principles.md` (spacing rhythm, alignment, scale), `bans.md` (over-rounding, ghost-cards), `accessibility.md` (contrast after color nudges), `registers/brand.md` / `registers/product.md` (the system being tightened toward).

## Output
The refined code with values snapped to the grid and token set, plus a short note listing each inconsistency fixed (value before → after) and why — and a request to confirm the rendered result, since no visual verification was performed.
