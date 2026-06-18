# layout — Enhance

**Invocation:** `/design layout [target]`   **Status:** active

## Purpose
Fix the spatial structure of an existing surface — spacing, rhythm, visual hierarchy, alignment, and composition. This command replaces ad-hoc gaps and misaligned elements with a deliberate system on the 8pt grid, varies spacing to create rhythm, and strengthens hierarchy so the eye moves in order of importance. It leans on Gestalt grouping (proximity, common region) to explain structure through whitespace rather than boxes, and avoids the lazy identical-card grid. The pass edits source in place and leaves the layout calm and organized.

## Flow
1. **Setup.** Run the SKILL.md setup workflow (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). This is an Enhance command, so the `PRODUCT.md` gate applies — run `init` first if it is missing. Open the existing layout/CSS before editing.
2. **Apply the 8pt grid.** Space and size in multiples of 8 (use 4 for fine adjustments) so rhythm is predictable (`principles.md`, 8pt grid). Vary spacing intentionally — tight within a group, generous between groups — rather than spreading one uniform gap everywhere.
3. **Choose the right layout primitive.** Flexbox for 1D, grid for 2D; for card rows use `repeat(auto-fit, minmax(280px, 1fr))` to flow responsively without breakpoints (`responsive.md`). Use a semantic z-index scale (dropdown → sticky → modal → toast → tooltip), never 999/9999.
4. **Strengthen hierarchy.** Lead the eye in order of importance using scale, spacing, and placement (`principles.md`, visual hierarchy). Group related elements by proximity and common region, communicating relationships by varying whitespace over adding lines or boxes (`principles.md`, Gestalt).
5. **Avoid the lazy answers.** Don't repeat identical icon-+-heading-+-text cards, and never nest cards (`bans.md`); cards are the lazy default — use them only when truly the best affordance. Vary the composition where the content varies.
6. **Confirm the result.** No live-browser/screenshot is available — reason over the code, then ask the user to confirm the rendered spacing, alignment, and hierarchy hold across real viewport widths.

## Cites
`principles.md` (8pt grid, scale/visual hierarchy, Gestalt proximity and common region, balance), `responsive.md` (responsive grid without breakpoints, mobile-first, breakpoints where content breaks), `bans.md` (identical card grids, nested cards, z-index 999/9999), the active register (`registers/brand.md` or `registers/product.md`).

## Output
The edited source with the corrected spacing system, alignment, and hierarchy, plus a short note: the grid applied, where rhythm and grouping changed, and which structural problems were resolved.
