# Platform Conventions — Web

What's specific to web UI, so the plugin builds for the browser instead of importing mobile assumptions.

> Provenance: seeded from adversarially-verified deep research (3-vote). Apple HIG / Material 3 are JS-rendered SPAs — values confirmed via convergent search extracts + corroborating primary docs; substance high-confidence, exact phrasing not byte-for-byte.

## The core difference: there is no OS

Mobile platforms ship an OS with reserved gestures, a system navigation model, and a system typeface. The web ships none of that. The browser is the only "platform layer," and it is deliberately thin. Almost every UI convention is yours to define — which is freedom and a trap: there is no single right answer to copy, so **follow the project's existing pattern; do not impose a new one.**

## Navigation — no native convention

- Web has **no native OS navigation convention.** Sidebar, top nav, and hamburger are all common; none is "the standard."
- The plugin must match the host app's existing pattern, not introduce a competing one.
- Choice is driven by information architecture and viewport, not by an OS contract.

## History — browser vs. in-app

- The browser **Back** button / `history.back()` is a **separate concept** from in-app navigation.
- Single-page apps (SPAs) must manage their **own history stack** with the [History API](https://developer.mozilla.org/en-US/docs/Web/API/History_API) — `pushState` to record navigation, `popstate` to react to Back/Forward.
- If you don't wire this up, the browser Back button will leave your app, not navigate within it.

## Gestures — nothing reserved (but don't fight the browser)

- The web has **no reserved OS-level gestures.** Pointer and touch events are fully app-controlled.
- The flip side: the **browser** owns some native gestures. Don't unexpectedly hijack pull-to-refresh, edge back-swipe, or pinch-zoom — users expect those to behave normally.

## Controls — only the basics are native

- Only basic HTML form elements (`<button>`, `<input>`, `<select>`, `<textarea>`, etc.) are truly native.
- Anything richer (date pickers, comboboxes, dialogs, menus, tabs) requires a **library or design system** — or careful custom work.
- Native controls render differently per browser/OS; style them with cross-browser consistency in mind. See the [MDN CSS reference](https://developer.mozilla.org/en-US/docs/Web/CSS).

## Typography — any font, but `system-ui` for native feel

- Any web font is available (self-hosted or via a font service).
- A **`system-ui` font stack** gives a native feel on each OS and the **best performance** (no font download, no layout shift).
- Reserve custom web fonts for brand-critical type; weigh the load cost.

## Responsiveness — the web's superpower

- One codebase adapts from phone to wide desktop. This is what the web does that mobile platforms can't.
- Build with **media queries** (viewport-driven) and **container queries** (component-driven). See `responsive.md`.
- Treat layout as fluid by default; design breakpoints, not fixed device sizes.

## What the web CANNOT do

- **No Material You dynamic color** (wallpaper-derived theming is an Android 12+ OS feature).
- **No Apple SF Symbols** (an Apple-only, OS-bundled icon system).
- Use a **cross-platform icon set** instead — Material Symbols or similar.

## User-preference media queries — honor them

The browser exposes OS-level accessibility/appearance preferences as CSS media queries. Respect them:

- `prefers-reduced-motion` — reduce or remove non-essential animation. See `motion.md`.
- `prefers-color-scheme` — support light/dark per the user's OS setting.
- `prefers-contrast` — adapt for higher-contrast needs. See `accessibility.md`.

## Sources

- [MDN — Web/CSS reference](https://developer.mozilla.org/en-US/docs/Web/CSS)
- [MDN — History API](https://developer.mozilla.org/en-US/docs/Web/API/History_API)
