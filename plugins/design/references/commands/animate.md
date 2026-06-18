# animate — Enhance

**Invocation:** `/design animate [target]`   **Status:** active

## Purpose
Add purposeful motion and micro-interactions to existing UI so movement communicates rather than decorates. Every animation this command emits must serve a role — signal a state change, express a spatial relationship between two views, or confirm that an action registered. Motion is a design material with a frame budget, not a coat of polish: it stays cheap to render, it never gates content behind itself, and it always carries a reduced-motion alternative. The pass edits source in place and leaves the static design fully usable with motion stripped out.

## Flow
1. **Setup.** Run the SKILL.md setup workflow (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). This is an Enhance command, so the `PRODUCT.md` gate applies — run `init` first if it is missing. Open the real CSS/component source before editing.
2. **Pick the moments.** Find where motion would reduce confusion: a state change (toggle, open/close, validation), a spatial relationship (a panel revealing where it came from), or feedback (a tap that visibly registered). Skip everything else — competing motion destroys staging (`motion.md`). Do not animate frequent system-default interactions.
3. **Choose curve and duration.** Use ease-out exponential curves — decelerate for entering, accelerate for exiting, base for persistent (`motion.md`). No bounce or elastic. Pair small/utility moves with short/medium durations (50–400ms) and large/expressive moves with long (450–600ms).
4. **Animate cheap properties only.** Stick to `transform` and `opacity`, which can composite off the main thread; never animate `width`, `height`, `top`, `left`, `margin`, or other layout/paint properties (`motion.md`). Reveals must enhance an already-visible default — never start content hidden and depend on a class-triggered transition to show it (`bans.md`).
5. **Give every animation a reduced-motion alternative.** This is a generation-time obligation, not optional polish (`accessibility.md`, SC 2.3.3). Under `@media (prefers-reduced-motion: reduce)`, replace movement with a quick cross-fade or color shift — replace, don't remove — so the feedback survives.
6. **Confirm the result.** No live-browser/screenshot is available — reason over the code, then ask the user to confirm the rendered motion feels right and reduced-motion behaves as intended.

## Cites
`motion.md` (purpose, easing/duration tokens, performance, compositor-friendly properties, reduced-motion swap), `accessibility.md` (`prefers-reduced-motion`, SC 2.2.2 / 2.3.3), `bans.md` (reveal-safety: never gate visible content on a transition), `slop-test.md` (motion that isn't a category reflex), the active register (`registers/brand.md` or `registers/product.md`).

## Output
The edited source with the added transitions and micro-interactions, each with its reduced-motion alternative, plus a short note: which moments got motion, the curve/duration chosen, and why each animation earns its place.
