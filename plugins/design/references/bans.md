# Absolute Bans

Match-and-refuse anti-patterns. These are not "use sparingly" guidance — they are tells of generated UI. If you are about to write one of these, stop and **rewrite the element with a different structure**. The terse always-on list lives in `SKILL.md`; this doc carries the *why*, the *rewrite*, and a citation/convention tag per rule.

> Provenance: reimplemented in our own words from the concepts behind the `impeccable` skill (github.com/pbakaus/impeccable, Apache-2.0). We are MIT and clean-room. Items rooted in a named visual principle cite it; aesthetic calls are tagged **(convention)**.

---

## Side-stripe borders

- **Match:** a colored `border-left` or `border-right` wider than 1px used as an accent on a card, list row, callout, or alert.
- **Why it's a tell:** the colored stripe is the single most reused "I styled this" gesture in generated UI. It signals category without communicating anything specific, and it stacks badly when rows repeat.
- **Rewrite:** use a full (even-weight) border, a background tint, a leading number or icon, or nothing at all. Pick the affordance that actually encodes the meaning (e.g. severity → background tint + icon).
- *(convention)*

## Gradient text

- **Match:** `background-clip: text` (or `-webkit-background-clip: text`) painting a gradient onto type.
- **Why it's a tell:** it is purely decorative and never carries meaning; it also fights legibility and contrast checking. It reads as "make this look premium" with no intent behind it.
- **Rewrite:** one solid color. Create emphasis with **weight or size**, which is what scale and hierarchy are for (see [principles.md](./principles.md)).
- *(convention; legibility implication ties to [accessibility.md](./accessibility.md))*

## Glassmorphism as default

- **Match:** blurred/translucent "glass" cards used as the standard surface treatment.
- **Why it's a tell:** background blur is expensive, hurts text contrast, and when applied to every surface it stops meaning anything. Default glass is the giveaway, not glass itself.
- **Rewrite:** reserve blur for the rare case where layering genuinely communicates depth (e.g. an overlay above live content). Otherwise use a solid surface — or nothing.
- *(convention)*

## The hero-metric template

- **Match:** giant number + small label + a row of supporting stats + a gradient accent — the stock SaaS hero.
- **Why it's a tell:** it is a layout chosen before the content existed. The same shell appears regardless of what the product actually does.
- **Rewrite:** design the layout around the *actual* content and what the user needs to see first. If the headline fact isn't a single number, don't force one.
- *(convention)*

## Identical card grids

- **Match:** a grid of same-size cards, each icon + heading + paragraph, repeated down the page. Nested cards (a card inside a card) are **always** wrong.
- **Why it's a tell:** uniform repetition is the default output when no structural decision was made. It flattens hierarchy — everything reads as equally (un)important.
- **Rewrite:** vary structure across items, and choose the right affordance per item (a list, a table, a feature row, a single emphasized item). Never nest cards; promote the inner content or split the container.
- *(convention; flattened hierarchy contradicts scale/hierarchy in [principles.md](./principles.md))*

## Eyebrow on every section

- **Match:** a tiny uppercase, wide-tracked kicker sitting above each section heading.
- **Why it's a tell:** **one** deliberate kicker is brand voice. One above *every* section is AI grammar — a template applied without thought.
- **Rewrite:** vary the section cadence. Let most sections start at the heading; use a kicker only where it earns its place.
- *(convention)*

## Numbered section markers as scaffold

- **Match:** `01 / 02 / 03` set above every section as decoration.
- **Why it's a tell:** numbering implies an ordered sequence. When the sections aren't actually ordered, the numbers are scaffolding pretending to be meaning.
- **Rewrite:** number only where the order **carries information** (steps, ranked items, a timeline). Elsewhere, drop them.
- *(convention; ordering-as-meaning relates to Gestalt grouping in [principles.md](./principles.md))*

## Text that overflows its container

- **Match:** long heading words + a large `clamp()` max + a narrow grid column — type that breaks out of or collides with its box.
- **Why it's a tell:** it betrays that the layout was never tested at real breakpoints with real copy.
- **Rewrite:** test every heading at every breakpoint. Lower the `clamp()` maximum, widen the column, or rewrite the copy. Fluid type still has to fit (see [responsive.md](./responsive.md)).
- *(convention; fluid-type mechanics in [responsive.md](./responsive.md))*

---

## Over-rounding & ghost-cards *(convention)*

- **Radius ceiling:** cards top out around **12–16px** radius. A full pill is fine for tags and buttons; it is not fine for content cards. Oversized radii read as toy-like and generic.
- **No ghost-cards:** don't pair a **1px border** with a **soft, wide drop shadow** (blur ≥ 16px) on the *same* element. The two say opposite things about elevation. Pick one — a crisp border *or* a shadow — per surface.
- *(convention)*
