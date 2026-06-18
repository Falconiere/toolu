# Design Knowledge Base — `references/`

On-demand reference material for the `design` skill's `/design <command> [target]` dispatcher. The dispatcher (`skills/design/SKILL.md`) inlines the terse always-on law; everything here is loaded only when a command needs it. The whole `references/` tree publishes to `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/references/` via the SessionStart hook.

> **Provenance.** The cited KB docs (principles, ux-usability, accessibility, responsive, motion, platform-*) were seeded from a deep-research pass: fan-out web search → fetch → **3-vote adversarial verification** (a claim needs 2/3 refutes to be killed) → synthesis, plus a targeted primary-source lookup for values behind JS-rendered docs (Material 3, Apple HIG). 56 of 60 verified claims were confirmed; 4 were refuted and **excluded**. Primary sources: NN/g, W3C/WCAG 2.2, Apple HIG, Material Design 3, MDN, web.dev. The design law (`bans.md`, `slop-test.md`) and the registers are clean-room reimplementations inspired by [`pbakaus/impeccable`](https://github.com/pbakaus/impeccable) (Apache-2.0); each opinion-level claim is tagged `(convention)`.

## Cited knowledge base

| Doc | Covers |
|---|---|
| [principles.md](principles.md) | Visual foundations — scale, hierarchy, balance, contrast, Gestalt; spacing/8pt grid; type scale; color |
| [ux-usability.md](ux-usability.md) | Nielsen's 10 heuristics, Norman affordances/signifiers, slips vs mistakes, Hick's & Fitts's laws |
| [accessibility.md](accessibility.md) | WCAG 2.2 — contrast, target size, focus, motion criteria, `prefers-reduced-motion` |
| [responsive.md](responsive.md) | Touch targets, breakpoints, mobile-first, fluid type (`clamp`), container queries |
| [motion.md](motion.md) | **Deepest** — purpose, Disney→UI, M3 easing/duration tokens, spring vs duration, performance, libraries, reduced motion |
| [platform-web.md](platform-web.md) | Web-specific conventions; no native nav standard; history API; responsiveness |
| [platform-mobile.md](platform-mobile.md) | iOS HIG & Material 3 highlights; iOS/Android/web divergence table; detect-don't-unify |

## Design law (clean-room)

| Doc | Covers |
|---|---|
| [bans.md](bans.md) | Absolute bans (match-and-refuse anti-patterns) — rule, why, rewrite, citation/convention |
| [slop-test.md](slop-test.md) | The two-altitude AI-slop / category-reflex test; color-strategy commitment axis |
| [context.md](context.md) | `PRODUCT.md` / `DESIGN.md` format, locations, register selection, inline OKLCH palette guidance |

## Registers — `registers/`

The design posture a project is in; the dispatcher loads exactly one (Setup step 4).

| Register | When |
|---|---|
| [registers/brand.md](registers/brand.md) | Design **is** the product — marketing, landing, campaign, portfolio, long-form |
| [registers/product.md](registers/product.md) | Design **serves** the product — app UI, admin, dashboard, tool (the default) |

## Commands — `commands/`

One reference per catalog command (`commands/<id>.md`), loaded when that command is invoked. 22 commands are active; only `live` is deferred (needs a browser runtime) and routes to an active fallback so dispatch never dead-ends. The catalog table lives in `skills/design/SKILL.md`; `skills/design/__tests__/catalog.bats` asserts catalog ↔ files stay in sync.

## Confidence & caveats

- **High confidence:** WCAG 2.2 thresholds, NN/g visual principles, Nielsen heuristics, Norman model, error taxonomy, web animation-performance model — all 3-0 unanimous from primary sources.
- **SPA-sourced (substance high, exact phrasing not byte-for-byte):** Apple HIG and Material 3 are JS-rendered; values (M3 motion tokens, iOS tab-bar rules, fonts, target sizes) confirmed via convergent extracts + corroborating primary docs.
- **Design law is opinion-level:** the bans, slop test, and register guidance are craft conventions (tagged `(convention)`), not verified research findings.

Each KB doc lists its own `## Sources`.
