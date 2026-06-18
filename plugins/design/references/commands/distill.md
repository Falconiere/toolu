# distill — Refine

**Invocation:** `/design distill [target]`   **Status:** active

## Purpose
Strip a design down to its essence by removing complexity that doesn't earn its place. It finds the elements that exist out of habit rather than need — redundant chrome, decorative cards, extraneous copy, controls and steps that could merge — and cuts or collapses them so what remains serves the user's goal directly. It is reductive on purpose: fewer parts, sharper focus, and a clear core task path. The discipline is to remove *without* harming what the user actually came to do.

## Flow
1. **Setup.** Run the `SKILL.md` workflow: detect-stack, load `PRODUCT.md`/`DESIGN.md` (run `init` first if `PRODUCT.md` is absent), pick the register, and open the existing design so cuts respect the real system.
2. **Identify the core task path.** Name what the user is here to accomplish on this surface. Everything is judged against whether it serves that path — mark the core elements as protected before cutting anything.
3. **Find what doesn't earn its place.** List the candidates for removal: redundant chrome and dividers, decorative cards that wrap content without organizing it, duplicated controls, over-explained or restating copy, and steps that add ceremony without value.
4. **Cut or merge.** Remove what's dead weight; merge what's redundant (two controls into one, two cards into one region, three sentences into one). Prefer recognition over recall — let the remaining UI show the option rather than describe it (`ux-usability.md`).
5. **Protect the core.** Re-verify the task path is intact and now *shorter*: every step still reachable, no needed affordance removed, hierarchy still legible. Reducing parts should reduce cognitive load, not hide function.
6. **Re-check the law (`SKILL.md`).** Confirm the leaner version didn't leave (or create) an Absolute Ban — e.g. identical card grids that should now be a list, or nested cards that should be promoted to a single region.
7. **Confirm the result.** No live-browser or screenshot — reason over the code, then ask the user to confirm the rendered surface still does the job with less.

## Cites
`ux-usability.md` (extraneous cognitive load, recognition over recall, Hick's Law on choice count), `principles.md` (whitespace over boxes, hierarchy), `bans.md` (decorative/identical/nested cards, eyebrow and numbered scaffold).

## Output
The refined code with unnecessary elements cut or merged and the core task path preserved, plus a short note listing what was removed/collapsed and why it didn't earn its place — and a request to confirm the rendered result.
