---
name: design
description: "Design, build, redesign, critique, audit, refine, or fix a web or mobile UI — one command surface for frontend craft. Fires on: \"design/redesign/build/generate this screen/page/component/landing page/dashboard/form\", \"make it bolder/quieter/cleaner\", \"add motion/animation\", \"add color\", \"fix the typography/spacing/layout\", \"improve the copy/error messages\", \"make it responsive/accessible/production-ready\", \"design review / critique / audit this UI\", \"is this accessible/responsive/on-brand\". Routes via `/design <command> [target]` (init, document, shape, craft, extract, critique, audit, polish, bolder, quieter, distill, harden, onboard, animate, colorize, typeset, layout, delight, overdrive, clarify, adapt, optimize, live). Does NOT fire for backend/non-UI work, bug/logic review (use /code-review), test fixes, refactors, debugging, search, or commits."
---

# Design

One skill for frontend design craft: build it, refine it, evaluate it, fix it. Invoked as `/design <command> [target]`. The first word selects a command; the rest is the target (a file, dir, component/selector, or a served URL). Inspired by the impeccable skill (github.com/pbakaus/impeccable); this is a clean-room, markdown/bash-native, zero-dependency reimplementation over a cited design knowledge base.

## Setup — run before any command

References publish to `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/` (this skill reads from there, not the plugin root).

1. **Detect stack.** `"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/detect-stack.sh" --json .` → `platform ∈ {web, mobile, both, unknown}` scopes findings. Honor a user-named platform over detection.
2. **Load context.** Read `PRODUCT.md` (strategic: users, brand, tone, anti-references, optional `register:`) and `DESIGN.md` (visual: colors, type, spacing, components) — root → `.agents/context/` → `docs/`, override `DESIGN_CONTEXT_DIR`. Format in `references/context.md`. **The PRODUCT.md gate applies only to generative commands** (Build/Refine/Enhance/Fix): if missing, run `init` first. **`critique`, `audit`, and the no-argument menu run without it** — use context if present, note its absence in the verdict.
3. **Load the command reference.** If a command was invoked, read `references/commands/<command>.md` next. Non-optional — it owns the command's flow.
4. **Load the register.** Read `references/registers/brand.md` (design IS the product: marketing, landing, campaign, portfolio, long-form) or `references/registers/product.md` (design SERVES the product: app UI, admin, dashboard, tool). Pick by first match: task cue → surface in focus → `register:` in PRODUCT.md. **Default `product`** when no signal.
5. **Read existing design.** Open at least one real CSS/tokens/theme/representative component before generating — preserve brand identity, reuse what works. New project (no committed tokens) → compose an OKLCH palette inline per `references/context.md`.

## Routing

1. **No argument** → "what should I do?": read setup signals (stack, `DESIGN.md` present?, dirty files), lead with the 2–3 highest-value commands (each with a one-line reason), then the full catalog grouped by category. Never auto-run; the user confirms.
2. **First word is a command** (catalog below) → load `references/commands/<id>.md`, follow it; everything after is the target.
3. **First word isn't a command but intent maps** (e.g. "fix the spacing" → `layout`, "rewrite this error" → `clarify`, "colors feel flat" → `colorize`) → load that command's reference and proceed. If two fit, ask once.
4. **No clear match** → general design invocation: apply Setup + the law below + the register, using the full argument as context.
5. **A planned (stub) command** → load its stub, state it's planned for a later phase, and offer the nearest active command.

## The law — always in force

These apply to every build and refine command, and back every critique/audit finding. Terse here; rationale, rewrites, and citations in `references/bans.md` and `references/slop-test.md`, loaded on demand.

### Absolute bans (match-and-refuse — rewrite the element instead)

- **Side-stripe borders** — a colored `border-left`/`border-right` >1px as a card/alert/callout accent. Use a full border, a background tint, a leading icon/number, or nothing.
- **Gradient text** — `background-clip: text` over a gradient. Solid color; emphasis via weight or size.
- **Glassmorphism by default** — decorative blur/glass. Rare and purposeful, or not at all.
- **The hero-metric template** — big number + small label + supporting stats + gradient accent. SaaS cliché.
- **Identical card grids** — same-size icon-+-heading-+-text cards repeated endlessly. Nested cards are always wrong.
- **Eyebrow on every section** — tiny uppercase tracked kicker above each heading. One deliberate kicker is voice; one per section is AI grammar.
- **Numbered section markers as scaffold** (01 / 02 / 03) — earn it only when the section truly is an ordered sequence.
- **Text that overflows its container** — test headings at every breakpoint; reduce the `clamp()` max or rewrite the copy. The viewport is part of the design.

### General design rules

- **Color** — verify contrast: body ≥4.5:1, large text (≥18px / bold ≥14px) ≥3:1, placeholders ≥4.5:1; thresholds in `references/accessibility.md`. Light-gray body text on a tinted near-white is the most common failure — push toward the ink end. Gray on a colored bg looks washed out; use a darker shade of the bg's own hue. New projects: OKLCH; pick a color *strategy* (restrained / committed / full-palette / drenched) before colors; avoid the cream/sand/beige warm-neutral default.
- **Typography** — body line length 65–75ch; pair fonts on a contrast axis (serif+sans, geometric+humanist) or one family in weights, never two near-identical sans; display `clamp()` max ≤6rem, letter-spacing floor ≥-0.04em; `text-wrap: balance` on h1–h3, `pretty` on prose. Scale + hierarchy + Gestalt per `references/principles.md`.
- **Layout** — vary spacing for rhythm (8pt grid, 4 for fine); flexbox for 1D, grid for 2D; responsive grid without breakpoints `repeat(auto-fit, minmax(280px, 1fr))`; a semantic z-index scale (dropdown→sticky→modal→toast→tooltip), never 999/9999; cards are the lazy answer — use them only when truly the best affordance.
- **Motion** — intentional, part of the build, not an afterthought; ease-out with exponential curves (no bounce/elastic); don't animate layout properties; reveals must enhance an already-visible default (never gate content on a class-triggered transition); reduced motion is not optional — every animation needs a `@media (prefers-reduced-motion: reduce)` alternative. Tokens + performance in `references/motion.md`.
- **Interaction** — dropdowns in an `overflow: hidden/auto` container get clipped; use `<dialog>`/popover, `position: fixed`, or a portal to escape the stacking context.
- **Responsive / platform** — touch targets ≥44px; mobile-first; fluid type via `clamp`; honor platform conventions (`references/platform-web.md` / `platform-mobile.md`). When `platform: unknown`, review only platform-agnostic dimensions and say platform-fit was skipped.

### The AI-slop test

Run at two altitudes (full detail + worked examples in `references/slop-test.md`):

- **First-order** — if someone could guess the theme + palette from the category alone, it's the first training-data reflex. Rework the scene + color strategy until the answer isn't obvious from the domain.
- **Second-order** — if someone could guess the aesthetic from category-plus-anti-reference ("AI tool that's *not* SaaS-cream → editorial-typographic"), that's the trap one tier deeper. Rework until neither answer is obvious.

If someone could look at the result and say "AI made that" without doubt, it failed.

## Commands

| Command | Category | Purpose | Reference | Status |
|---|---|---|---|---|
| `init` | Build | Set up project context (PRODUCT.md / DESIGN.md), recommend next steps | `commands/init.md` | active |
| `document` | Build | Generate DESIGN.md from existing project code | `commands/document.md` | active |
| `shape` | Build | Plan UX/UI before writing code | `commands/shape.md` | active |
| `craft` | Build | Shape, then build a feature end-to-end | `commands/craft.md` | active |
| `extract` | Build | Pull reusable tokens/components into the design system | `commands/extract.md` | active |
| `critique` | Evaluate | UX design review: hierarchy, IA, cognitive load, heuristics | `commands/critique.md` | active |
| `audit` | Evaluate | Technical checks: accessibility, performance, responsive | `commands/audit.md` | active |
| `polish` | Refine | Final quality pass before shipping | `commands/polish.md` | active |
| `bolder` | Refine | Amplify safe or bland designs | `commands/bolder.md` | active |
| `quieter` | Refine | Tone down aggressive or overstimulating designs | `commands/quieter.md` | active |
| `distill` | Refine | Strip to essence, remove complexity | `commands/distill.md` | active |
| `harden` | Refine | Production-ready: errors, i18n, edge cases, overflow | `commands/harden.md` | active |
| `onboard` | Refine | First-run flows, empty states, activation | `commands/onboard.md` | active |
| `animate` | Enhance | Add purposeful motion and micro-interactions | `commands/animate.md` | active |
| `colorize` | Enhance | Add strategic color to monochromatic UI | `commands/colorize.md` | active |
| `typeset` | Enhance | Improve typography hierarchy and fonts | `commands/typeset.md` | active |
| `layout` | Enhance | Fix spacing, rhythm, and visual hierarchy | `commands/layout.md` | active |
| `delight` | Enhance | Add personality and memorable touches | `commands/delight.md` | active |
| `overdrive` | Enhance | Push past conventional limits | `commands/overdrive.md` | active |
| `clarify` | Fix | Improve UX copy, labels, and error messages | `commands/clarify.md` | active |
| `adapt` | Fix | Adapt for different devices and screen sizes | `commands/adapt.md` | active |
| `optimize` | Fix | Diagnose and fix UI performance | `commands/optimize.md` | active |
| `live` | Iterate | In-browser variant iteration — **deferred** (see reference) | `commands/live.md` | planned (deferred) |

## Scope

This skill spans build and evaluate. `critique`/`audit` review only — they cite a primary-source criterion per finding and never auto-fix. Build/refine commands edit source; without live-browser/screenshot they reason over code, not rendered pixels — say so when visual confirmation matters. Keep findings tied to the cited knowledge base; an opinion without a reference is a nit.
