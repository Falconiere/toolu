# Project Context — PRODUCT.md & DESIGN.md

The dispatcher loads project context during **Setup step 2**; the `init` and `document` commands write these files. This doc defines what they contain, where they live, and how the register and palette are chosen.

> Provenance: describes this plugin's own conventions. Concept lineage from the `impeccable` skill (github.com/pbakaus/impeccable, Apache-2.0); reimplemented clean-room, MIT. OKLCH color model per [MDN — oklch()](https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/oklch). Opinion-level items tagged **(convention)**.

---

## PRODUCT.md — strategic context *(required for generative commands)*

The *why* and *for whom*. Generative commands (generate, from-source) require it; without it they cannot make non-reflexive decisions.

Skeleton:

```md
# Product

## Target users
Who they are, what they're trying to do, the context they work in.

## Brand & tone
Voice, personality, the feeling the product should leave.

## Anti-references
What to deliberately avoid — competitors, clichés, the obvious category look.

## Strategic principles
The few decisions everything else should serve.

register: brand | product   # optional; see Register selection below
```

## DESIGN.md — visual system *(optional)*

The *how it looks*. If absent, the model reasons the system inline; if present, it is authoritative.

Skeleton:

```md
# Design System

## Color        # OKLCH; bg / surface / ink / accent / muted (+ states)
## Typography   # families, modular scale, weights
## Spacing      # base unit + scale (see principles.md — 8pt grid)
## Radii        # ceiling ~12–16px for cards (see bans.md)
## Elevation    # border XOR shadow per surface (see bans.md)
## Components    # recurring patterns and their rules
```

`document` **auto-generates DESIGN.md from the existing code** — it reads the implemented styles and writes the system back out, so an established codebase gets a real spec rather than a guess.

---

## Locations & precedence

Searched in order, first hit wins:

1. project root
2. `.agents/context/`
3. `docs/`

Override the search with the `DESIGN_CONTEXT_DIR` environment variable.

---

## Register selection

Two registers; pick by **what the design is to the product**.

- **brand** — *design IS the product.* Marketing sites, landing pages, campaigns, portfolios, long-form. The visual is the value.
- **product** — *design SERVES the product.* App UI, admin, dashboards, internal tools. The visual stays out of the way of the task.

Default to **product** when no signal points clearly to brand. The optional `register:` field in PRODUCT.md sets it explicitly. *(convention)*

---

## Inline OKLCH palette guidance *(new projects, no committed tokens)*

When there are no existing tokens, the model reasons the palette inline — **no script**:

1. **Pick a brand seed** color (informed by the scene sentence from [slop-test.md](./slop-test.md), not the category reflex).
2. **Compose roles around it** in OKLCH: `bg`, `surface`, `ink`, `accent`, `muted`. Move primarily on **L** for the neutral ramp; reserve higher **C** for `accent`.
3. **Tint neutrals toward the brand hue** by adding only **0.005–0.015 chroma**, at the brand's hue — enough to relate, not enough to read as colored. **Do not default-tint warm** (the cream/sand trap; see [slop-test.md](./slop-test.md)).
4. **Verify every contrast pair** (ink-on-bg, ink-on-surface, accent-on-bg, etc.) against the thresholds in [accessibility.md](./accessibility.md) before committing.

*(convention; OKLCH per MDN, contrast thresholds per [accessibility.md](./accessibility.md))*
