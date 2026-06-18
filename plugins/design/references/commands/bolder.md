# bolder — Refine

**Invocation:** `/design bolder [target]`   **Status:** active

## Purpose
Amplify the impact of a design that plays it too safe — bland, generic, forgettable — without sacrificing usability or legibility. It diagnoses *why* the surface is flat and then raises contrast in the dimensions that carry impact: type scale, weight, color commitment, and a single strong focal point. "Bolder" is not "louder for its own sake," and it is not a license to reach for the obvious category reflex; it is a deliberate increase in confidence that still passes the slop-test and stays accessible.

## Flow
1. **Setup.** Run the `SKILL.md` workflow: detect-stack, load `PRODUCT.md`/`DESIGN.md` (run `init` first if `PRODUCT.md` is absent), and open the existing design. Pick the **brand** register when the surface is the product (marketing, landing, campaign); bolder leans hardest on `registers/brand.md`.
2. **Diagnose the flatness.** Name the specific causes: compressed type scale (everything one size), uniform weight, timid near-neutral color sitting at strategy level 1, no clear focal point, or hierarchy that reads as a list of equals.
3. **Choose a color strategy before touching color.** Move deliberately up the commitment axis (restrained → committed → full-palette → drenched) per the scene, rather than sprinkling a "tasteful accent."
4. **Raise contrast where it counts.** Widen the type scale and let the most important element get genuinely large; pull weight apart (the headline far heavier than body); commit the chosen color to emphasis and interactive states.
5. **Introduce one focal point.** Decide the single thing the eye should hit first and build the page around it — one anchor, not five competing ones.
6. **Strengthen motion, intentionally.** Where the build supports it, add ease-out entrance/emphasis per `motion.md` (no bounce, with a reduced-motion alternative) to reinforce the focal point — never to decorate.
7. **Run the slop-test (`SKILL.md` / `slop-test.md`).** Confirm "bolder" didn't collapse into the cliché (giant gradient hero, neon-on-dark). Check both altitudes; rework until neither the category nor the anti-reference answer is obvious. Verify contrast still clears AA after the color push.
8. **Confirm the result.** No live-browser or screenshot — reason over the code, then ask the user to confirm the rendered surface reads as bold-but-usable.

## Cites
`principles.md` (scale, contrast, hierarchy, focal point), `slop-test.md` (two-altitude check, color-strategy axis), `registers/brand.md` (impact register), `motion.md` (emphasis motion, reduced-motion), `accessibility.md` (contrast after the color push), `bans.md` (don't reach for gradient text / hero-metric).

## Output
The refined code with a stronger scale, weight, color commitment, and a single focal point, plus a short note naming what was flat, what was amplified and why, the slop-test result, and a request to confirm the rendered result.
