# design

Web & mobile UI/UX design as a single command surface. Invoke `/design <command> [target]` to **build, refine, evaluate, or fix** a frontend interface over a cited, stack-aware knowledge base. Standalone — pure markdown + bash, zero dependencies, MIT.

> Clean-room reimplementation inspired by the excellent [`pbakaus/impeccable`](https://github.com/pbakaus/impeccable) skill (Apache-2.0). This plugin adapts impeccable's *skill architecture* — a command dispatcher, per-project context, design registers, anti-pattern law, and an AI-slop test — to a zero-dependency, markdown/bash-native plugin backed by an adversarially-verified, primary-source-cited knowledge base. No impeccable code or text is copied; the wording, structure, and citations are our own.

## Usage

```
/design                       # context-aware menu of the highest-value next commands
/design critique src/LoginForm.tsx
/design audit https://example.com/pricing
/design init                  # set up PRODUCT.md / DESIGN.md
/design document              # generate DESIGN.md from existing code
```

The first word selects a command; the rest is the target (a file, directory, component/selector, or a served URL). Before any command the skill runs a setup workflow: detect the stack, load `PRODUCT.md`/`DESIGN.md` context (required only for generative commands), load the command reference, load the register (brand vs product), and read the existing design.

## Command catalog

23 commands in six categories. **22 are active**; only `live` is deferred (it needs a Node/browser runtime, out of scope for this zero-dependency plugin) and routes to its nearest active fallback so dispatch never dead-ends.

| Category | Commands |
|---|---|
| Build | `init`* · `document`* · `shape` · `craft` · `extract` |
| Evaluate | `critique`* · `audit`* |
| Refine | `polish` · `bolder` · `quieter` · `distill` · `harden` · `onboard` |
| Enhance | `animate` · `colorize` · `typeset` · `layout` · `delight` · `overdrive` |
| Fix | `clarify` · `adapt` · `optimize` |
| Iterate | `live` (deferred — needs a Node/browser runtime, out of scope here) |

<sub>* active. All commands are active except `live`.</sub>

## Layout

```
skills/design/SKILL.md        # the dispatcher: setup, routing, always-on law, catalog
skills/design/evals/          # held-out trigger eval (cases.json, results.json)
skills/design/__tests__/      # catalog.bats — catalog↔files, size, zero-dep, docs-sync
references/                   # cited KB (principles, accessibility, motion, …)
references/bans.md            # absolute bans (clean-room + cited)
references/slop-test.md       # the two-altitude AI-slop test
references/context.md         # PRODUCT.md / DESIGN.md format + OKLCH palette guidance
references/registers/         # brand.md, product.md
references/commands/          # one reference per command (22 active, live deferred)
scripts/detect-stack.sh       # bash stack detector (web/mobile/unknown JSON)
hooks/session-start.sh        # publishes scripts/ + references/ to a stable path
```

## Knowledge base

Findings cite a primary source: WCAG 2.2 (accessibility), NN/g (visual principles, Nielsen heuristics), Apple HIG + Material 3 (platform), MDN/web.dev (web). The design law (`bans.md`, `slop-test.md`, registers) is craft convention, tagged as such. See [`references/README.md`](references/README.md) for the full index and provenance.

## Scope

`critique`/`audit` review only — they cite a criterion per finding and never auto-fix. Build/refine commands edit source but, without a live-browser/screenshot runtime, reason over code rather than rendered pixels. License: MIT.
