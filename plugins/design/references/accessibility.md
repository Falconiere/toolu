# Accessibility (WCAG 2.2)

Hard, testable accessibility thresholds the plugin enforces.

> Provenance: seeded from adversarially-verified deep research (3-vote). Primary sources cited.

## Standard & baseline

[WCAG 2.2](https://www.w3.org/TR/WCAG22/) became a W3C Recommendation on **5 October 2023**. It added 9 new success criteria over 2.1 for a **net +8** — SC 4.1.1 Parsing was removed. See [What's New in WCAG 2.2](https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/).

**Plugin baseline: target AA on everything; reach for AAA where feasible.**

## Contrast (testable)

Ratios are unchanged since WCAG 2.0.

| Criterion | Level | Threshold |
|---|---|---|
| SC 1.4.3 Contrast (Minimum) — text | AA | **≥ 4.5:1** |
| SC 1.4.3 — large text (≥18pt, or ≥14pt bold) | AA | **≥ 3:1** |
| SC 1.4.11 Non-text Contrast — UI components & graphical objects | AA | **≥ 3:1** vs adjacent colors |

How the plugin checks this: **mechanically testable.** Compute the contrast ratio between any text/background pair and any UI-component/adjacent-color pair; fail anything below threshold.

## Target size (testable)

| Criterion | Level | Threshold |
|---|---|---|
| [SC 2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html) — new in 2.2 | AA | **≥ 24×24 CSS px** |
| SC 2.5.5 Target Size (Enhanced) | AAA | **≥ 44×44 CSS px** |

The five exceptions to SC 2.5.8: **Spacing, Equivalent, Inline, User Agent Control, Essential.**

How the plugin checks this: **mechanically testable.** Measure the pointer target's hit area in CSS px; fail anything under 24×24 that does not meet a documented exception. (Cross-platform native target sizes live in `responsive.md`.)

## Focus

| Criterion | Level | Rule |
|---|---|---|
| SC 2.4.7 Focus Visible | AA | Keyboard focus indicator **must be visible**. |
| SC 2.4.11 Focus Not Obscured (Minimum) | AA | A focused component must **not be ENTIRELY hidden** by author content. Sticky headers/footers may *partially* obscure at AA. |
| SC 2.4.12 Focus Not Obscured (Enhanced) | AAA | Forbids **ANY** obscuring. |

**SC 2.4.13 Focus Appearance (AAA) — partial:** only the **"≥ 3:1 contrast between the focused and unfocused states"** prong is verified here. The "2 CSS px thick perimeter" size prong was **refuted** in research (1–2) — do **not** state the size prong as established.

## Motion

| Criterion | Level | Rule |
|---|---|---|
| SC 2.2.2 Pause, Stop, Hide | A | Auto-starting moving/blinking/scrolling content that lasts **> 5s** and runs in parallel with other content needs a pause/stop/hide mechanism — unless essential. |
| SC 2.3.3 Animation from Interactions | AAA | Interaction-triggered motion animation can be **disabled** unless essential. |

### `prefers-reduced-motion`

A CSS media feature that detects an OS/device setting to minimize non-essential motion. See [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion).

- Values: `no-preference` (false) and `reduce` (true).
- `@media (prefers-reduced-motion)` is equivalent to `@media (prefers-reduced-motion: reduce)`.
- Purpose: prevent discomfort for users with vestibular disorders. Scaling/panning large objects are vestibular triggers. (Vestibular is the **primary but not sole** rationale.)

```css
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

How the plugin checks this: **any animation the plugin generates MUST honor `prefers-reduced-motion: reduce`** (see `motion.md`). Unlike contrast and target size, this is a generation-time obligation, not a static measurement.

## How the plugin enforces this

- **Mechanically testable now:** contrast (1.4.3 / 1.4.11) and target size (2.5.8 / 2.5.5) — measure and fail below threshold.
- **Generation-time obligation:** reduced-motion must be respected in every animation the plugin emits — cross-link `motion.md`.
- **Default to AA; upgrade to AAA where feasible** (e.g., 44×44 targets, no focus obscuring at all).

## Sources

- [WCAG 2.2 (W3C Recommendation)](https://www.w3.org/TR/WCAG22/)
- [What's New in WCAG 2.2 (W3C WAI)](https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/)
- [Understanding SC 2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)
- [MDN: `prefers-reduced-motion`](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion)
