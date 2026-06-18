# craft — Build

**Invocation:** `/design craft [target]`   **Status:** active

## Purpose
Take a confirmed brief through to working, production-grade UI. `craft` shapes first (or loads an existing brief), then builds the surface end-to-end: real components, real states, accessible markup, responsive behavior, and motion where it earns its place. It enforces the full design law while building — not as a cleanup pass afterward — so the result is something shippable, on-brand, and safe, not a prototype that needs a second pass to be honest.

## Flow
1. **Setup.** Run the dispatcher Setup in `SKILL.md`: `detect-stack`, load `PRODUCT.md`/`DESIGN.md` (Build command — the PRODUCT.md gate applies; if missing, run `init` first), load the command reference, and pick the register.
2. **Get a brief.** If `shape` already produced one, load and confirm it; otherwise run a brief inline (see `shape.md`) and get sign-off before writing code. Build to intent, never to a guess.
3. **Read before you write.** Open the existing components, tokens, and theme and reuse them — same buttons, same spacing scale, same color roles. New work must extend the system in `DESIGN.md`, not fork it. New project with no committed tokens → compose an OKLCH palette inline per `context.md`.
4. **Build under the full law (`SKILL.md`).** Color with verified contrast and a chosen strategy; typography on a contrast axis with sane line length and `clamp()` ceilings; layout on the 8pt rhythm with the right primitive (flex 1D / grid 2D) and a semantic z-index scale; motion that is ease-out, reduced-motion-aware, and never gates content; interaction that escapes clipping stacking contexts. Build every state from the brief — empty, loading, error, success, edge.
5. **Enforce the bans and run the slop-test before finishing.** Match-and-refuse every Absolute Ban — no side-stripe borders, gradient text, default glass, hero-metric template, identical/nested card grids, eyebrow-per-section, scaffold numbering, or overflowing text (`bans.md`). Then run the AI-slop test at both altitudes and rework until "AI made that" isn't an obvious read (`slop-test.md`).
6. **Self-check.** Verify body/large-text/placeholder contrast against `accessibility.md`; check the layout at each breakpoint and confirm no text overflows its container; check touch targets and mobile-first behavior per `responsive.md`; confirm a `prefers-reduced-motion` alternative exists for every animation per `motion.md`.
7. **No visual verification.** There is no browser runtime or screenshot here — `craft` reasons over code, not rendered pixels. State that visual confirmation (real rendering, real motion, real breakpoints) needs the user, and list exactly what to eyeball.

## Cites
- `principles.md` — scale, hierarchy, Gestalt, balance. `accessibility.md` — contrast thresholds, semantics, focus.
- `responsive.md` — breakpoints, fluid type, touch targets. `motion.md` — easing, reduced-motion, performance.
- `bans.md` — Absolute Bans (match-and-refuse). `slop-test.md` — distinctiveness. The active register (`registers/brand.md` or `registers/product.md`) for the build's failure mode.

## Output
Complete, on-brand, accessible source — components, states, styles, and motion — plus a self-check summary (contrast, responsive, reduced-motion, overflow, bans, slop-test) and an explicit list of what the user must confirm in a real browser.
