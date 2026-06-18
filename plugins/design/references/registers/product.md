# Register: Product — design SERVES the product

The posture for surfaces where design is in service of a task, and clarity, consistency, and low cognitive load win over self-expression. The dispatcher (`../../skills/design/SKILL.md`) loads exactly one register.

> **This is the DEFAULT register.** When no signal points clearly to brand work, load this one. The other register is `brand.md`.

## When this register applies

- **Surfaces:** application UI, admin panels, dashboards, internal tools, settings screens, forms, tables, data-dense views.
- **Task cues:** "add a settings page," "build the dashboard," "this form," "the admin screen," anything the user operates repeatedly to get work done.
- **Default rule:** absent an explicit signal that the design itself is the product, assume the design serves the product — load this register.

## Posture

- **Restraint, consistency, legibility.** The UI should **disappear into the task** — the user thinks about their work, not your interface.
- **Reuse before invention.** Use existing components, tokens, and patterns already in the codebase; a new bespoke control is a cost, not a feature.
- **Consistency beats cleverness.** A predictable, familiar pattern lowers cognitive load (`ux-usability.md` — Nielsen's "consistency and standards").

## Color

- Lean **restrained**: tinted neutrals plus **one accent**, kept to roughly **≤10%** of the surface. *(convention; see `principles.md` — neutral + accent structure.)*
- Color **earns its place by signaling** — state, hierarchy, status — never as decoration.
- **Don't rely on color alone** to convey meaning; pair with text, icon, or shape (`principles.md`, `accessibility.md`).
- Meet contrast thresholds: text ≥ 4.5:1, large text/UI ≥ 3:1 (`accessibility.md`, SC 1.4.3 / 1.4.11).

## Type

- **Clear hierarchy first.** Guide the eye by rank; readability over personality (`principles.md` — scale, visual hierarchy).
- Use **≤3 sizes per composition** (`principles.md` — the verified "no more than 3 sizes").
- Body measure **~65–75ch** for comfortable reading (`principles.md` — line length 45–75 characters).
- Default to the system/existing type stack; a custom display face is rarely the right call in a tool.

## Layout & interaction

- **8pt rhythm:** space and size in multiples of 8 (4 for fine tuning) for predictable spacing (`principles.md`). *(convention)*
- **Semantic z-index:** layer by meaning (base → raised → overlay → modal), not by ad-hoc numbers.
- **Real affordances:** controls should look operable; signifiers must match behavior (`ux-usability.md` — Norman affordances/signifiers).
- Apply **Fitts's** (size/proximity of frequent targets) and **Hick's** (fewer choices, faster decisions) where they bear on the task (`ux-usability.md`).

## Accessibility — non-negotiable

This is a tool people must be able to operate; treat `accessibility.md` (WCAG 2.2 AA) as a floor, not a goal.

- **Contrast:** text ≥ 4.5:1, large text & UI components ≥ 3:1 (SC 1.4.3 / 1.4.11).
- **Target size:** ≥ 24×24 CSS px (SC 2.5.8); reach for 44×44 (AAA) on primary controls.
- **Focus:** visible keyboard focus, not obscured (SC 2.4.7 / 2.4.11).
- **Reduced motion:** honor `prefers-reduced-motion: reduce` for any animation (`accessibility.md`, `motion.md`).

## Failure mode

**Inconsistency and decoration-as-noise** — over-styling a tool. Gratuitous gradients, animation, and one-off components that add visual weight without adding clarity raise cognitive load and break the pattern users rely on. If a flourish doesn't help the task, cut it. When in doubt, make it quieter.

## See also

- `principles.md` — hierarchy, 3-size rule, 8pt grid, neutral + accent
- `ux-usability.md` — heuristics, affordances, Fitts's & Hick's laws
- `accessibility.md` — WCAG 2.2 contrast, target size, focus, reduced motion
- `motion.md` — keep motion minimal and reduced-motion safe
- `brand.md` — the other register, for design-as-the-product surfaces
