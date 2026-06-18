# adapt — Fix

**Invocation:** `/design adapt [target]`   **Status:** active

## Purpose
Repair a design so it holds together across devices, screen sizes, and contexts — phone to wide desktop, touch to mouse, screen to print. This command edits the source to set a mobile-first base, add breakpoints where the content actually breaks, make type and spacing fluid, size touch targets for fingers, and honor each platform's conventions. It treats responsiveness as design, not a media-query afterthought: every viewport gets a layout that reads as intentional rather than merely shrunk, and headings are checked so nothing overflows its container.

## Flow
1. **Setup first.** Run the workflow in `SKILL.md` (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). adapt is generative, so the **PRODUCT.md gate applies** — if missing, run `init`. Use detect-stack to scope the platform; on `platform: unknown`, adapt only platform-agnostic dimensions and say platform-fit was skipped.
2. **Set a mobile-first base.** Write the unprefixed styles for the smallest viewport, then layer `min-width` queries to enhance upward — less CSS, progressive enhancement (`responsive.md`). Remember breakpoint-prefixed utilities apply at that size **and above**.
3. **Add breakpoints where content breaks, not at device widths.** Set a breakpoint at the viewport where *this* layout fails (a row that crowds, a line that gets too long), not at arbitrary phone/tablet sizes. Override framework defaults when the design needs it (`responsive.md`). Reach for `repeat(auto-fit, minmax(280px, 1fr))` to reflow grids without breakpoints at all.
4. **Make type fluid with `clamp`.** Use `clamp(MIN, PREFERRED, MAX)` with PREFERRED mixing `rem` + `vw` (pure `vw` breaks zoom) and **MAX ≤ 2.5× MIN** as a convention that keeps 200%-zoom reflow intact (WCAG SC 1.4.4 Resize Text) (`responsive.md`). Prefer `@container`/`cqi` for component-scoped sizing.
5. **Size touch targets for `pointer: coarse`.** Under `@media (pointer: coarse)`, give interactive controls ≥44px hit area; expand the hit region with padding or a transparent `::after`, not by enlarging the visible control (`responsive.md`; `accessibility.md` SC 2.5.5, AAA 44×44 / SC 2.5.8, AA 24×24). Never ship `user-scalable=no`.
6. **Honor platform conventions.** Follow `platform-web.md` / `platform-mobile.md` for navigation, gestures, density, and safe areas so the surface feels native to its context.
7. **Test headings for overflow.** Check h1–h3 at every breakpoint; if a heading would spill its container, lower the `clamp()` max or rewrite the copy — text overflow is an Absolute Ban (`bans.md`). No visual verification: reason over code and confirm the rendered result with the user.

## Cites
`responsive.md` (mobile-first, content-driven breakpoints, fluid `clamp` type, `@container`, `pointer: coarse`, viewport meta — heavily), `accessibility.md` (target size SC 2.5.8 / 2.5.5), `platform-web.md` / `platform-mobile.md` (conventions), `bans.md` (text overflows its container).

## Output
The edited source with the responsive changes in place, plus a short note of what changed and why — base styles, each breakpoint and the content reason for it, fluid type, target sizing, and any overflow fixes.
