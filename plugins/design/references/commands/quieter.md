# quieter — Refine

**Invocation:** `/design quieter [target]`   **Status:** active

## Purpose
Tone down a design that is loud, busy, or overstimulating into something calmer — while keeping (not lowering) quality. It finds the sources of visual noise, reduces them to a single clear focal point, calms the palette, and strips decorative motion, so the important elements lead and the surface feels confident rather than frantic. The point is restraint that *raises* perceived quality and lowers cognitive load, not a loss of personality or hierarchy.

## Flow
1. **Setup.** Run the `SKILL.md` workflow: detect-stack, load `PRODUCT.md`/`DESIGN.md` (run `init` first if `PRODUCT.md` is absent), pick the register, and open the existing design so the calmer version stays inside the established system.
2. **Locate the noise.** Enumerate the specific sources: multiple competing focal points, over-saturated or too-many colors, animation that runs without purpose, and decoration (borders, badges, shadows, dividers) that fights the content instead of organizing it.
3. **Reduce to one focal point.** Decide the single thing that should lead and demote the rest; resolve the "everything shouts" condition into a clear primary / secondary order.
4. **Calm the palette.** Pull saturation back, cut the number of accents in play, and move toward a more restrained color strategy — without dropping body contrast below AA.
5. **Remove decorative motion.** Cut animation that isn't communicating state or guiding attention; keep only intentional, ease-out motion with a reduced-motion alternative (`motion.md`).
6. **Protect hierarchy and load.** Verify the quieter surface is still legibly ordered and that removing noise actually *lowered* extraneous cognitive load rather than flattening meaning (`ux-usability.md`). Run the slop-test so "quiet" doesn't become generic minimal-template blandness.
7. **Confirm the result.** No live-browser or screenshot — reason over the code, then ask the user to confirm the rendered surface reads calm-but-clear.

## Cites
`principles.md` (focal point, hierarchy, contrast as a lever), `slop-test.md` (avoid generic-minimal cliché), `motion.md` (cut decorative motion, reduced-motion), `ux-usability.md` (extraneous cognitive load), `accessibility.md` (contrast floor after desaturating).

## Output
The refined code with fewer competing accents, a calmer palette, one focal point, and decorative motion removed, plus a short note naming each noise source and what was dialed back and why — and a request to confirm the rendered result.
