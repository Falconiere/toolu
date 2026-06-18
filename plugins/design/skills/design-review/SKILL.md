---
name: design-review
description: "Review/critique/audit a web or mobile UI for design quality against cited, primary-source criteria and the project's detected stack. Fires on: \"review this screen/component/layout/design\", \"design review\", \"is this accessible / responsive / on-brand\", \"check the UX / visual hierarchy / contrast / motion\", \"does this match iOS/Android conventions\", \"audit this interface\". Reviews source files and (statically) a served URL. Does NOT fire for code-correctness/bug review (use /code-review) or for generating/building UI. Screenshot/image review arrives in a later phase."
---

# Design Review

Audit a UI against the design knowledge base and the project's actual stack, and report findings a developer can act on — each tied to a primary-source criterion. This skill **reviews**; it does not rewrite the UI (generating fixes is a separate skill).

## When this fires

A request to judge the *quality* of an interface: "review this screen", "is this accessible", "design review", "check the visual hierarchy / contrast / spacing / motion", "does this feel native on iOS". The target is UI — components, screens, a stylesheet, a layout, or a served page.

**Does NOT fire for:** code-correctness or bug review (that's `/code-review`), or requests to *generate* UI. If the user wants both a review and fixes, review first, then hand the fixes to the appropriate build step.

## Inputs (Phase 1)

- **Source files** — component/markup/stylesheet files in the repo.
- **A running URL** — reviewed **statically**: fetch the served HTML/CSS (WebFetch) and analyze semantics, ARIA, heading structure, CSS-derived contrast and target sizes. This canNOT assess *rendered* visual hierarchy or actual motion (no rendering/vision yet) — say so in the verdict when a URL was the target.
- *(Screenshot/image review is a later phase — don't claim to "see" a rendered UI you were only given as code or HTML.)*

## How to run it

1. **Detect the stack.** Run the published detector:
   ```bash
   "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/detect-stack.sh" --json .
   ```
   Use `platform` to scope findings: `web` → web conventions, `mobile` → iOS/Android conventions.
   - **Fallback — `platform:"unknown"`** (non-JS repo, no manifest, or ambiguous): review only the platform-agnostic dimensions (visual-hierarchy, usability, accessibility, motion) and **state in the verdict that platform-fit was skipped because the stack was undetected**. Do not guess a platform. If the user named a platform, honor that over detection.

2. **Load references on demand — never preload all.** For each dimension you actually review, read the matching doc and cite the specific criterion. References are published at:
   ```
   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/references/<doc>.md
   ```

   | Dimension | Reference doc |
   |---|---|
   | visual-hierarchy | `principles.md` |
   | usability | `ux-usability.md` |
   | accessibility | `accessibility.md` |
   | responsive | `responsive.md` |
   | motion | `motion.md` |
   | platform-fit | `platform-web.md` / `platform-mobile.md` |
   | consistency | `principles.md` |

3. **Emit findings**, one per line, then a verdict.

## Finding format

```
<location>: <severity> [<dimension>]: <problem>. <fix>. (<ref>)
```

- **`severity`** — one uniform scale across all dimensions: `blocker` | `major` | `minor` | `nit`.
- **`<dimension>`** — one of the dimensions above.
- **`<ref>`** — the specific criterion that backs the finding: a WCAG success criterion (with level), or a reference doc + section. Accessibility findings cite the WCAG criterion **and** its level.
- **`<location>`** — `file:line` for source, or a selector/region for a URL.

Examples:
```
Button.tsx:42: major [accessibility]: tap target is 32×32px, below the minimum. Pad the hit area to ≥44px (visual size can stay). (WCAG SC 2.5.5 AAA; responsive.md)
Hero.tsx:8: minor [visual-hierarchy]: four competing font sizes in one block. Reduce to ≤3 sizes; make the primary heading largest. (principles.md — Scale)
nav.css:21: major [motion]: 600ms slide on every menu open with no reduced-motion guard. Add a prefers-reduced-motion fallback (cross-fade) and shorten. (motion.md §8; WCAG SC 2.3.3)
```

## Verdict

Close with a single line summarizing severity counts and the headline, and note any dimension that was skipped (unknown stack) or limited (static URL only):

```
Verdict: <n> blocker, <n> major, <n> minor — <one-sentence summary>. [platform-fit skipped: stack undetected] [URL reviewed statically: rendered visual/motion not assessed]
```

## Scope

Review only. No auto-fixing, no generating replacement UI, no screenshot/image input (Phase 1). Keep findings tied to a cited criterion — an opinion without a reference is a nit at best; prefer the knowledge base over taste.
