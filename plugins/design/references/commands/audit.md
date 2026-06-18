# audit — Evaluate

**Invocation:** `/design audit [target]`   **Status:** active

## Purpose
A technical-quality review of the target — accessibility, performance, responsive behavior, motion safety, and coded anti-patterns. It is **review-only**: it measures, cites, and reports, but never edits. It runs **without** `PRODUCT.md` (use it if present; note its absence in the verdict). Where `critique` judges UX, `audit` checks the mechanically verifiable: contrast ratios, target sizes, focus, breakpoints, motion guards, and the Absolute Bans.

## Flow
1. **Scope the platform.** Run `detect-stack` (`"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/detect-stack.sh" --json .`). For `platform: unknown`, review only platform-agnostic dimensions and **skip platform-fit** — say so in the verdict.
2. **Served-URL target.** If the target is a running URL, WebFetch the HTML/CSS and audit **statically**. Rendered visual and motion can't be assessed — note it in the verdict suffix.
3. **Run the checks, citing a criterion per finding:**
   - **Accessibility (`accessibility.md`, WCAG 2.2):** contrast — body ≥4.5:1 (SC 1.4.3, AA), large/UI ≥3:1 (SC 1.4.3 / 1.4.11, AA); target size ≥24×24px (SC 2.5.8, AA), reach 44×44 (SC 2.5.5, AAA); visible, unobscured focus (SC 2.4.7 / 2.4.11, AA).
   - **Responsive (`responsive.md`):** mobile-first base styles, breakpoints set where content breaks, touch targets sized for `pointer: coarse`, fluid type with `rem`+`vw` and MAX ≤2.5× MIN, no `user-scalable=no`.
   - **Motion (`motion.md` + `accessibility.md`):** every animation has a `prefers-reduced-motion: reduce` alternative (generation-time obligation); animate compositor-friendly properties, no layout thrash; auto-running motion >5s respects SC 2.2.2.
   - **Anti-patterns (`bans.md`):** flag any Absolute Ban present in the code (side-stripe borders, gradient text, default glassmorphism, ghost-cards, etc.).
4. **Emit findings** in the format:
   `<location>: <severity> [<dimension>]: <problem>. <fix>. (<ref>)`
   - `<location>` = file:line or selector/region.
   - `<severity>` ∈ `blocker | major | minor | nit`.
   - `<dimension>` = e.g. accessibility, responsive, motion, performance, platform-fit, consistency.
   - `<ref>` = a WCAG SC (with level) or a KB doc + section.
5. **Close with the verdict line:**
   `Verdict: <n> blocker, <n> major, <n> minor — <summary>.` Append `[platform-fit skipped: stack undetected]` and/or `[URL reviewed statically: rendered visual/motion not assessed]` when they apply.

## Cites
- `accessibility.md` — WCAG 2.2 contrast, target size, focus, motion criteria, `prefers-reduced-motion`.
- `responsive.md` — touch targets, breakpoints, mobile-first, fluid type. `motion.md` — performance and reduced-motion guards.
- `bans.md` — Absolute Bans.

## Output
A list of findings in the standard format followed by the severity-count verdict line — measured, cited, and review-only.
