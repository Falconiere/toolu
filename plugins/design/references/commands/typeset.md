# typeset — Enhance

**Invocation:** `/design typeset [target]`   **Status:** active

## Purpose
Improve the typography of existing UI so text reads cleanly and hierarchy is obvious at a glance. This command sets a deliberate modular scale, fixes measure and line height for readability, pairs fonts on a real contrast axis, and tames display type so it never overflows or sets too loose. Typography carries most of a design's clarity; this pass makes that work intentional rather than accidental, editing source in place while preserving the brand voice already present.

## Flow
1. **Setup.** Run the SKILL.md setup workflow (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). This is an Enhance command, so the `PRODUCT.md` gate applies — run `init` first if it is missing. Open the existing type tokens/components before editing.
2. **Set a modular scale.** Use a constant ratio (≈1.2 / 1.25 / 1.333) rather than arbitrary sizes, and keep ≤3 sizes per composition (`principles.md`, type scale and scale/hierarchy). The most important element is the biggest.
3. **Fix the measure.** Set body line length to 65–75ch for sustained readability (`principles.md`); add `text-wrap: balance` on h1–h3 and `pretty` on prose so lines break sensibly.
4. **Pair on a contrast axis.** Pair fonts on a real axis (serif + sans, geometric + humanist) or use one family across weights — never two near-identical sans (`principles.md`). Build hierarchy with weight and size, not decoration.
5. **Tame display type.** Cap display `clamp()` max at ≤6rem and hold the letter-spacing floor at ≥-0.04em (SKILL.md law). Use fluid type with `clamp()` (`rem` + `vw`, max ≤2.5× min) so sizing scales without breakpoints, and test headings at every breakpoint so nothing overflows (`responsive.md`).
6. **Confirm the result.** No live-browser/screenshot is available — reason over the code, then ask the user to confirm the rendered hierarchy and measure read well across real viewport widths.

## Cites
`principles.md` (modular type scale, ≤3 sizes, scale/visual hierarchy, line length, font pairing), `responsive.md` (fluid type via `clamp`, max ≤2.5× min, breakpoints where content breaks), `accessibility.md` (legible text, contrast cross-link), `bans.md` (text that overflows its container), the active register (`registers/brand.md` or `registers/product.md`).

## Output
The edited source with the refined type scale, measure, pairing, and display settings, plus a short note: the scale and ratio chosen, the pairing rationale, and which readability/overflow issues were resolved.
