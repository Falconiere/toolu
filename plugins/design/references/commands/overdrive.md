# overdrive — Enhance

**Invocation:** `/design overdrive [target]`   **Status:** active

## Purpose
Push a surface past conventional limits with technically ambitious effects — scroll-driven reveals, spring physics, shaders, particle fields — engineered to hold 60fps. This is the deliberately aggressive enhance pass and the **most performance- and accessibility-risky command in the catalog**: the same ambition that makes a moment unforgettable can drop frames, trigger vestibular discomfort, or hide content on weak hardware. It only runs when the brand moment justifies it, it budgets every effect against the frame deadline, and it ships a mandatory low-power and reduced-motion fallback. The pass edits source in place.

## Flow
1. **Setup.** Run the SKILL.md setup workflow (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). This is an Enhance command, so the `PRODUCT.md` gate applies — run `init` first if it is missing. Open the existing source before editing. Overdrive suits brand surfaces far more than product tools.
2. **Choose an effect that serves the moment.** Pick one ambitious effect that amplifies a real brand beat (`registers/brand.md`), not spectacle for its own sake. One staged effect beats several competing ones (`motion.md`: staging/focus).
3. **Budget for performance.** Stay inside the frame deadline — ~16.7ms at 60fps (`motion.md`, performance). Animate compositor-friendly `transform`/`opacity`, avoid layout thrash, and promote layers deliberately (over-promotion harms performance). Measure the frame cost rather than assuming it.
4. **Ship the mandatory fallbacks.** A `@media (prefers-reduced-motion: reduce)` alternative is required, not optional (`accessibility.md`, SC 2.2.2 / 2.3.3) — drop spinning, depth-scaling, parallax, and multi-axis motion. Add a low-power / weak-GPU fallback so the surface degrades to a static or lightweight version.
5. **Never gate content on the effect.** Content visibility must never depend on a scroll-reveal or shader completing (`bans.md`: reveal safety) — the surface must be fully usable with the effect stripped out. Reveals enhance an already-visible default.
6. **Confirm the result.** No live-browser/screenshot is available — reason over the code and frame budget, then ask the user to confirm the rendered effect holds 60fps on their target hardware and that both fallbacks behave correctly.

## Cites
`motion.md` (performance: frame budget, compositor layers, no layout thrash; staging), `accessibility.md` (reduced motion, SC 2.2.2 / 2.3.3, vestibular triggers), `bans.md` (reveal safety: never gate visible content on a transition), `registers/brand.md` (the brand moment that justifies the ambition), `slop-test.md` (ambition that isn't a category cliché).

## Output
The edited source with the effect plus its reduced-motion and low-power fallbacks, and a note that content is never gated on it — plus a short summary: the effect, its measured frame cost, and how it degrades.
