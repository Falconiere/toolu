# Responsive & Adaptive Design

How the plugin makes layouts adapt across viewports and inputs.

> Provenance: seeded from adversarially-verified deep research (3-vote). Primary sources cited.

## Touch targets (cross-platform)

Corroborated by Apple, Google, and W3C primary docs.

| Platform | Minimum | Source |
|---|---|---|
| iOS | **≥ 44×44 pt** | [Apple HIG tips](https://developer.apple.com/design/tips) |
| Android / Material 3 | **≥ 48×48 dp** | [Google accessibility](https://support.google.com/accessibility/android/answer/7101858) |
| Web (WCAG) | **24px (AA) / 44px (AAA)** | [WCAG 2.5.8](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html) |

**The visual element may be smaller than the target.** Expand the hit area with padding or a transparent `::after` overlay — not by enlarging the visible control.

```css
.icon-button {
  position: relative;
}
.icon-button::after {
  content: "";
  position: absolute;
  inset: 50%;
  width: 44px;
  height: 44px;
  transform: translate(-50%, -50%);
}
```

## Breakpoints

Research-sourced common ranges:

| Range (px) | Tier |
|---|---|
| 0–599 | compact / phone |
| 600–904 | medium |
| 905–1239 | expanded |
| 1240–1439 | large |
| 1440+ | wide |

Tailwind reference defaults: `sm` 640 / `md` 768 / `lg` 1024 / `xl` 1280 / `2xl` 1536.

**Key caveat — design for content, not devices.** Set breakpoints where *your* content breaks; override framework defaults when the design needs it.

## Mobile-first methodology

- Write **base (unprefixed) styles for the smallest viewport**, then layer `min-width` media queries to progressively enhance.
- Less CSS; aligns with progressive enhancement.
- **Gotcha:** breakpoint-prefixed utilities apply at that size **AND ABOVE**.

```css
/* base = smallest viewport */
.grid { display: block; }

/* enhance upward */
@media (min-width: 768px) {
  .grid { display: grid; grid-template-columns: repeat(2, 1fr); }
}
```

## Fluid typography — `clamp()`

Use CSS [`clamp(MIN, PREFERRED, MAX)`](https://developer.mozilla.org/en-US/docs/Web/CSS/clamp).

- **PREFERRED must mix `rem` + `vw`.** Pure `vw` breaks zoom and a custom base size.
- Keep **MAX ≤ 2.5× MIN** to meet WCAG zoom.

```css
font-size: clamp(1.875rem, 1.2rem + 3.375vw, 2.5rem); /* heading */
font-size: clamp(1rem, 0.9rem + 0.5vw, 1.125rem);     /* body */
```

Container-context upgrade — swap `vw` for `cqi`:

```css
font-size: clamp(1rem, 5cqi, 2.5rem);
```

## Container queries — `@container`

Respond to a **component's own available space**, not the viewport. The same component adapts whether placed in a narrow sidebar or a wide column. See [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_containment/Container_queries).

```css
.card-wrapper { container-type: inline-size; container-name: card; }

@container card (width > 700px) {
  .card-title { font-size: 1.5rem; }
}
```

- `cqi` / `cqb` units auto-resolve to the nearest container.
- **Rule of thumb:** `@container` for component/layout logic; `@media` for page-level structure.

## Pointer media feature

[`@media (pointer: coarse | fine)`](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/pointer) distinguishes touch (`coarse`) from a precise pointer (`fine`) for sizing and affordances.

```css
@media (pointer: coarse) {
  .toolbar button { min-height: 44px; min-width: 44px; }
}
```

## Accessibility rule (viewport meta)

**Never** use `user-scalable=no` or `maximum-scale=1` in the viewport meta — it blocks zoom and is a WCAG violation.

```html
<meta name="viewport" content="width=device-width, initial-scale=1" />
```

## Sources

- [Apple Human Interface Guidelines — Tips](https://developer.apple.com/design/tips)
- [Google: Make touch targets accessible (Android)](https://support.google.com/accessibility/android/answer/7101858)
- [Understanding WCAG SC 2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)
- [MDN: `clamp()`](https://developer.mozilla.org/en-US/docs/Web/CSS/clamp)
- [MDN: Container queries](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_containment/Container_queries)
- [MDN: `pointer` media feature](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/pointer)
