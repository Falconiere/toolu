# Register: Brand — design IS the product

The posture for surfaces where the design itself is what's being sold, and being memorable matters more than being conventional. The dispatcher (`../../skills/design/SKILL.md`) loads exactly one register; load this one when distinctiveness is the goal.

> One register at a time. If the work serves a task rather than an impression, load `product.md` instead.

## When this register applies

- **Surfaces:** marketing sites, landing/launch pages, campaign microsites, portfolios, long-form and editorial layouts, brand or pitch decks.
- **Task cues:** "make it stand out," "this is the homepage / hero," "for the launch," "needs to feel like *us*," anything where the page *is* the deliverable rather than a means to a task.
- **Tie-breaker:** if the artifact's job is to leave an impression, this register. If its job is to get a job done, that's `product.md`.

## Posture

- **Identity over convention.** A strong, specific point of view beats a safe, familiar one. Generic reads as forgettable here.
- **Commitment is rewarded.** Half-measures look like indecision. Pick a direction and lean into it across color, type, and motion together.
- **Distinctiveness is the deliverable.** The bar is "does this look like *this* brand and no one else's," not "does this look fine."
- Convention (`principles.md`) still governs the underlying craft — hierarchy, balance, contrast, Gestalt grouping. Expressiveness is *built on* those, not in spite of them.

## Color

- Lean toward **committed** color strategies on the commitment axis (see `slop-test.md`): full-palette, color-drenched, or a single dominant brand hue that carries the surface. *(convention)*
- The brand color can be the surface — saturated backgrounds, large fields of a signature hue — rather than a timid accent on white.
- Still honor contrast thresholds for any text and UI on those fields (`accessibility.md`, SC 1.4.3 / 1.4.11). Committed ≠ illegible.

## Type

- **Display typography is a lead instrument**, not decoration. A distinctive display face can do most of the identity work.
- Pair on a **contrast axis** — e.g. an expressive display against a quiet, highly readable text face — rather than two faces that compete. *(convention; see type scale in `principles.md`.)*
- Use **expressive scale**: large, confident headline sizes — within the size ceilings defined in `../../skills/design/SKILL.md`. Bold, not arbitrary.

## Motion

- Motion is **part of the impression** here, not a courtesy. Entrances, scroll reveals, and hover states shape how the brand feels.
- Keep it **purposeful and performant** — animate compositor-friendly properties; avoid layout thrash (`motion.md`).
- **Reduced-motion safe is non-negotiable:** honor `prefers-reduced-motion: reduce` (`accessibility.md`, `motion.md`). Expressive default, calm fallback.

## Reflex-reject aesthetic lanes

Currently-saturated families — AVOID by default because they read as "every AI demo." Reach for one only with a deliberate, stated reason. *(convention — these saturate and rotate over time; re-check.)*

- **SaaS-cream + violet/indigo gradient** hero and pill buttons.
- **Navy-and-gold "fintech/premium"** with serif numerals.
- **Generic glassmorphism hero** — frosted cards floating over a blurred mesh gradient.
- **Sketchy-doodle / hand-drawn-blob illustration** as the whole identity.

These aren't banned forever; they're *defaults to resist*. Distinctiveness means not landing where the gravity pulls everyone.

## Failure mode

The brand surface that **looks like every other AI landing page** — generic gradient, safe geometric sans, stock glass cards, no point of view. Before shipping, run the slop test (`slop-test.md`): if it could be any startup, the register's whole goal has been missed. Commit harder.

## See also

- `slop-test.md` — commitment axis, distinctiveness check
- `principles.md` — hierarchy, contrast, type scale, color discipline
- `motion.md` — purposeful, performant motion
- `accessibility.md` — contrast and reduced-motion floors that still apply
- `product.md` — the other register, for task-serving UI
