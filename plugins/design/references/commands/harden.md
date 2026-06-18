# harden — Refine

**Invocation:** `/design harden [target]`   **Status:** active

## Purpose
Make a surface production-ready by handling the cases a happy-path build skips. It enumerates and implements the states real use produces — loading, empty, error, partial, overflowing, RTL, and long strings — and tests the layout against real-world data so the UI holds up instead of breaking on first contact with users. This is the difference between a demo that works once and a feature that survives messy input, slow networks, and translated copy.

## Flow
1. **Setup.** Run the `SKILL.md` workflow: detect-stack (state size and overflow behavior differs per platform), load `PRODUCT.md`/`DESIGN.md` (run `init` first if `PRODUCT.md` is absent), pick the register, and open the existing components so new states match the established system.
2. **Enumerate the states.** For each data-bound region list the cases it must handle: loading, empty (first-run and zero-results), error/failure, partial/slow, overflowing content, RTL, and long-string / pluralization (i18n).
3. **Implement each state.** Give every one a designed treatment — a real empty state (not a blank box), a recoverable error with a clear next action (error prevention and recovery, `ux-usability.md`), a loading affordance, and graceful partial rendering. Don't leave a state to chance.
4. **Stress the text.** Test headings, labels, buttons, and badges at every breakpoint with the longest realistic and translated copy. Text that overflows its container is an Absolute Ban — lower the `clamp()` max, widen the column, allow wrap/truncation-with-affordance, or rewrite the copy (`bans.md`, `responsive.md`).
5. **Run real-world data.** Substitute long names, empty fields, huge numbers, and many/zero rows; confirm the layout degrades gracefully and touch targets and contrast survive (`responsive.md`, `accessibility.md`).
6. **Re-check the law (`SKILL.md`).** Confirm new states didn't introduce a ban and that error/empty states meet contrast and focus requirements.
7. **Confirm the result.** No live-browser or screenshot — reason over the code and describe how each state renders, then ask the user to confirm against their real data.

## Cites
`bans.md` (text that overflows its container), `responsive.md` (breakpoints, overflow, fluid type, touch targets), `accessibility.md` (contrast and focus across states), `ux-usability.md` (error prevention and recovery, status visibility).

## Output
The refined code with loading, empty, error, partial, overflow, RTL, and long-string states implemented and stress-tested, plus a short note listing each state added and the overflow/edge cases fixed — and a request to confirm against real data, since no visual verification was performed.
