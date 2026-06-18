# critique — Evaluate

**Invocation:** `/design critique [target]`   **Status:** active

## Purpose
A UX-perspective design review of the target — visual hierarchy, information architecture, cognitive load, usability heuristics, and emotional resonance. It is **review-only**: it diagnoses and cites, it never edits or auto-fixes. It runs **without** `PRODUCT.md` (use it if present; note its absence in the verdict), so it works on any surface. There is **no fabricated numeric score** — the result is a count of findings by severity plus a verdict line.

## Flow
1. **Setup.** Run `detect-stack` (`"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/detect-stack.sh" --json .`) to scope platform and pick the register. Load `PRODUCT.md`/`DESIGN.md` if present; if absent, proceed and note it.
2. **Served-URL target.** If the target is a running URL, review it **statically** (fetch the markup/CSS). Rendered visual and motion cannot be assessed this way — say so in the verdict suffix.
3. **Review each dimension, citing the criterion per finding:**
   - **Visual hierarchy** — scale, the ≤3-sizes rule, contrast as a hierarchy lever, balance (`principles.md`).
   - **Information architecture / grouping** — Gestalt proximity and common region; whitespace over boxes (`principles.md`).
   - **Cognitive load** — extraneous load, recognition over recall, Hick's Law on choice count (`ux-usability.md`).
   - **Heuristics** — Nielsen's 10 (status visibility, error prevention, consistency, etc.); Norman affordances/signifiers; Fitts's Law on frequent targets (`ux-usability.md`).
   - **Emotional resonance / distinctiveness** — run the slop-test at both altitudes; check the register's failure mode (`slop-test.md`, the active register).
   - **The law** — flag any Absolute Ban present (`bans.md`).
4. **Emit findings** in the format:
   `<location>: <severity> [<dimension>]: <problem>. <fix>. (<ref>)`
   - `<location>` = file:line or selector/region.
   - `<severity>` ∈ `blocker | major | minor | nit` (an opinion without a cited reference is at most a `nit`).
   - `<dimension>` = e.g. visual-hierarchy, usability, accessibility, consistency, motion.
   - `<ref>` = a KB doc + section (or a WCAG SC with level where one applies).
5. **Close with the verdict line:**
   `Verdict: <n> blocker, <n> major, <n> minor — <summary>.` Append `[platform-fit skipped: stack undetected]` and/or `[URL reviewed statically: rendered visual/motion not assessed]` when they apply, and note if `PRODUCT.md` was absent.

## Cites
- `ux-usability.md` — Nielsen's 10 heuristics, Norman affordances/signifiers, Hick's & Fitts's laws, slips vs mistakes.
- `principles.md` — scale, visual hierarchy, balance, contrast, Gestalt grouping.
- `bans.md` — Absolute Bans. `slop-test.md` — distinctiveness check.

## Output
A list of findings in the standard format followed by the severity-count verdict line — no numeric score, no edits.
