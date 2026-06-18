# Design Knowledge Base — `references/`

On-demand reference material for the `design` plugin's skills (generate, from-source, review, design-system). Each doc is a focused, citation-backed reference; skills load only what a task needs.

> **Provenance.** Seeded from a deep-research pass: fan-out web search → fetch → **3-vote adversarial verification** (a claim needs 2/3 refutes to be killed) → synthesis, plus a targeted primary-source lookup for values behind JS-rendered docs (Material 3, Apple HIG). 56 of 60 verified claims were confirmed; 4 were refuted and **excluded** (see below). Primary sources: NN/g, W3C/WCAG 2.2, Apple HIG, Material Design 3, MDN, web.dev.

## Index

| Doc | Covers |
|---|---|
| [principles.md](principles.md) | Visual foundations — scale, hierarchy, balance, contrast, Gestalt; spacing/8pt grid; type scale; color |
| [ux-usability.md](ux-usability.md) | Nielsen's 10 heuristics, Norman affordances/signifiers, slips vs mistakes, Hick's & Fitts's laws |
| [accessibility.md](accessibility.md) | WCAG 2.2 — contrast, target size, focus, motion criteria, `prefers-reduced-motion` |
| [responsive.md](responsive.md) | Touch targets, breakpoints, mobile-first, fluid type (`clamp`), container queries |
| [motion.md](motion.md) | **Deepest** — purpose, Disney→UI, M3 easing/duration tokens, spring vs duration, performance, libraries, reduced motion |
| [platform-web.md](platform-web.md) | Web-specific conventions; no native nav standard; history API; responsiveness |
| [platform-mobile.md](platform-mobile.md) | iOS HIG & Material 3 highlights; iOS/Android/web divergence table; detect-don't-unify |

## Confidence & caveats

- **High confidence:** the WCAG 2.2 thresholds, NN/g visual principles, Nielsen heuristics, Norman model, error taxonomy, and the web animation-performance model — all 3-0 unanimous from primary sources.
- **SPA-sourced (substance high, exact phrasing not byte-for-byte):** Apple HIG and Material 3 are JS-rendered; their values (M3 motion tokens, iOS tab-bar rules, fonts, target sizes) were confirmed via convergent search extracts + corroborating primary docs (WWDC, API refs, `developer.apple.com/fonts`).
- **Word strength:** Apple's optional/cancelable-motion guidance is *recommendation*-level — docs say "recommends/advises", not "requires".
- **Compositor precision:** `transform`/`opacity` are cheap **only once promoted to a compositor layer** and not triggering layout/paint; promotion is conditional and over-promoting harms performance.
- **M3 motion evolution:** Material 3 "Expressive" (2025–2026) is moving to a spring/physics engine; the documented duration/easing tokens are the classic/stable scale — cross-check the latest M3 motion docs for spring params.

## Excluded — refuted in verification (do not re-add without re-sourcing)

1. Disney "Anticipation" framed as hover / communicating-interactivity (1-2).
2. "Social signifier" defined *only* as deliberately-placed designer cues — Norman includes accidental ones (0-3).
3. `transform`/`opacity` as *unconditionally* compositor-only in every browser (1-2).
4. The bundle "iOS is flat vs Android uses elevation" + "iOS scroll-wheel vs Android calendar date pickers" (1-2). (FAB's role is fine — separately sourced from M3.)

## Thin spots (verified set was light; expand from primary sources when needed)

- 8pt-grid rationale and modular type-scale specifics (Material spacing; Refactoring UI / Smashing) — currently marked as convention in `principles.md`.
- Concrete Apple/SwiftUI default duration ranges for common transitions.
- Color theory depth (palettes, harmony) beyond contrast.

Each doc lists its own `## Sources`.
