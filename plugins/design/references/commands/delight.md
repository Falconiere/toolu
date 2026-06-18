# delight — Enhance

**Invocation:** `/design delight [target]`   **Status:** active

## Purpose
Add personality and a few memorable touches that lift a surface from merely functional to genuinely enjoyable. This command finds one or two high-leverage moments — an empty state, a success confirmation, a satisfying hover — and gives them character that fits the brand register, without ever undercutting clarity, accessibility, or performance. Delight is purposeful and rare by design: a couple of human details land far harder than personality sprinkled everywhere, and every touch must pass the slop-test so it reads as voice, not cliché. The pass edits source in place.

## Flow
1. **Setup.** Run the SKILL.md setup workflow (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). This is an Enhance command, so the `PRODUCT.md` gate applies — run `init` first if it is missing. Open the existing components before editing.
2. **Find one or two moments.** Pick the highest-leverage spots where a little character pays off — an empty state turned welcoming, a success moment made satisfying, a considered hover (`ux-usability.md`: recognition over recall, status visibility). Don't spread delight thin; restraint is what makes it land.
3. **Add character that fits the register.** Match the brand's voice and failure mode (`registers/brand.md`) — the same touch that delights on a campaign page may feel out of place on an admin tool. Keep copy and visuals consistent with the established tone.
4. **Keep it purposeful and safe.** Any motion stays subtle, on ease-out curves, and carries a `@media (prefers-reduced-motion: reduce)` alternative (`motion.md`). Delight must never block a task or reduce clarity (`ux-usability.md`).
5. **Run the slop-test.** Check the touch at both altitudes (`slop-test.md`) — if someone could guess it from the category alone (confetti on every success, a waving emoji on every empty state), rework it until it reads as this product's voice, not a default.
6. **Confirm the result.** No live-browser/screenshot is available — reason over the code, then ask the user to confirm the rendered moment feels delightful rather than gimmicky, and that reduced-motion still works.

## Cites
`motion.md` (subtle ease-out motion, reduced-motion alternative), `slop-test.md` (both-altitude distinctiveness check), `registers/brand.md` (voice, register fit, failure mode), `ux-usability.md` (status visibility, recognition over recall, delight never blocks the task).

## Output
The edited source with the one or two delight moments added, plus a short note: which moments were chosen, the character added, how it fits the register, and the slop-test result.
