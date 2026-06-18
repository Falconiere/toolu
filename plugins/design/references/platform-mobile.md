# Platform Conventions — Mobile (iOS & Android)

The platform-specific conventions the plugin must respect — and why the rule is detect-don't-unify.

> Provenance: seeded from adversarially-verified deep research (3-vote). Apple HIG / Material 3 are JS-rendered SPAs — values confirmed via convergent search extracts + corroborating primary docs; substance high-confidence, exact phrasing not byte-for-byte.

## Verified findings (3-0)

- **Design systems:** iOS follows [Apple HIG](https://developer.apple.com/design/human-interface-guidelines/); Android follows [Material Design 3](https://m3.material.io/) — including **Material You dynamic color** (Android 12+).
- **System typefaces:** iOS = **San Francisco (SF)**, with **New York** as the serif companion; Android = **Roboto**. See [Apple Fonts](https://developer.apple.com/fonts).
- **Navigation model:** Android provides **persistent system navigation** (back, home, recent apps; **predictive back** on Android 13+). iOS has **no system/persistent back button** — it relies on a top navigation-bar back control plus a **left-edge swipe-back gesture**. See [Android system bars](https://developer.android.com/design/ui/mobile/guides/foundations/system-bars).
- **iOS tab bar:** the control for navigating an app's **top-level sections** (the top of the content hierarchy — *not* drill-down). It **must remain persistently anchored to the bottom**, including during push transitions. Sections must be meaningful/descriptive. **Screen- or context-specific actions** (e.g. a checkout button) do **not** belong in the tab bar — put them with the content (use a toolbar). Current iOS includes a dedicated bottom **Search** tab. iOS 26 "Liquid Glass" restyles the tab bar, but its **role is unchanged**. See [HIG — Tab bars](https://developer.apple.com/design/human-interface-guidelines/tab-bars).
- **Material 3 typography:** treats line-height / leading as a **first-class readability concern**, encoded as **per-style type-scale tokens**. See [M3 — Applying type](https://m3.material.io/styles/typography/applying-type).

## iOS (Apple HIG) highlights

- **Navigation bar:** centered title; back chevron + previous-screen label; large titles collapse to standard on scroll.
- **Tab bar:** 3–5 items, bottom, navigation-only.
- **Back-swipe edge:** reserve the **left ~20pt edge** for the system back-swipe gesture.
- **Modals:** present from the bottom.
- **Icons:** SF Symbols icon system.
- **Motion:** spring-driven default transitions.
- **Targets:** **44pt** minimum touch target.

## Android (Material 3) highlights

- **Top app bar:** **LEFT-aligned** title; small / center / medium / large variants.
- **Navigation:** bottom **navigation bar** (3–5 items, pill-shaped active indicator) → **navigation rail** (medium/expanded screens) → **drawer** / expanded rail (M3 Expressive).
- **FAB:** the single **highest-priority action** — **one at a time**.
- **Adaptive layouts:** canonical patterns (list-detail / supporting-pane / feed) driven by **window size classes**.
- **Feedback:** ripple touch feedback.
- **Targets:** **48dp** minimum touch target.
- **Color:** Material You dynamic color (Android 12+).

> ⚠️ Do **not** assert "iOS is flat vs Android uses elevation" or "iOS scroll-wheel vs Android calendar date pickers" — that bundle was **refuted (1-2)** and is *(commonly cited — not independently verified here)*. The FAB's existence/role is fine (M3-sourced); the elevation and date-picker generalizations are not.

## Divergence

| Dimension | iOS | Android / M3 | Web |
|---|---|---|---|
| Back navigation | In-app back + left-edge swipe (no OS back) | System back gesture/button + predictive back | Browser back / History API (SPA-managed) |
| Nav placement | Bottom tab bar + top nav bar | Bottom nav bar → rail → drawer | No standard (sidebar / top / hamburger) |
| Gestures | Left edge = system back (reserved) | Edge swipe = back, up = home | None reserved |
| Icons | SF Symbols (Apple-only) | Material Symbols (open, cross-platform) | Any icon set |
| Dynamic color | None (static brand) | Material You (wallpaper-derived) | None |
| Typography | San Francisco | Roboto | Any web font / `system-ui` |
| Min target | 44pt | 48dp | 24px AA / 44px AAA (WCAG) |

## Why detect, don't unify

These differences are baked into **OS-level gestures, accessibility models, and user mental models.** Unifying to a single back metaphor or one nav placement breaks at least one platform's contract — e.g. forcing an on-screen back button onto iOS, or anchoring nav at the top on Android where the system back lives at the bottom. So the plugin should **detect the target platform and adapt** to its conventions rather than impose a lowest-common-denominator UI. Cross-link the stack / platform detector.

## Sources

- [Apple Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/)
- [Apple HIG — Tab bars](https://developer.apple.com/design/human-interface-guidelines/tab-bars)
- [Apple Fonts](https://developer.apple.com/fonts)
- [Material Design 3](https://m3.material.io/)
- [M3 — Applying type](https://m3.material.io/styles/typography/applying-type)
- [Android — System bars](https://developer.android.com/design/ui/mobile/guides/foundations/system-bars)
