# Motion & Animation

The principles, tokens, physics, performance rules, and library guidance the plugin uses to generate and review motion.

> Provenance: seeded from adversarially-verified deep research (3-vote) plus primary-source token/value lookup. Where Apple HIG / Material 3 pages are JS-rendered SPAs, values were confirmed via convergent search extracts + corroborating primary docs (WWDC, API refs) — substance is high-confidence, exact phrasing is not byte-for-byte.

Motion is a design material, not decoration. Every animation the plugin emits must justify itself, run inside the frame budget, animate cheap properties, and degrade gracefully under reduced-motion. The sections below are the rules and the numbers.

## 1. The purpose of motion (Apple HIG, verified 3-0)

Motion must be **purposeful**. Apple's guidance: *"Add motion purposefully... Don't add motion for the sake of adding motion. Gratuitous or excessive animation can distract."* Motion earns its place by serving one of these roles:

- **Orientation** — show where a view came from and where it goes (spatial model).
- **Continuity** — connect states so a change reads as a transformation, not a cut.
- **Feedback** — confirm that an action registered.
- **Delight** — sparingly, and never at the cost of the above.

Hard constraints:

- Motion should be **optional** and **NEVER the sole channel** for important information. Supplement with **haptics and audio** so meaning survives when motion is reduced or unseen.
- Feedback motion should be **gesture-consistent**: a view revealed by sliding down should dismiss by **reversing** (sliding back up), not exiting sideways. Keep it **brief and precise**.

Recommendations (softer than constraints — treat as defaults, not gates):

- **Avoid custom motion on FREQUENT interactions.** The system already animates standard controls; re-animating them adds latency and inconsistency.
- **Let people cancel or interrupt** animations rather than forcing them to wait for a fixed run to finish. (This is also the spring story — see §5.)

Source: Apple HIG — Motion, corroborated by WWDC23 sessions.

## 2. Disney's 12 principles → UI (verified 3-0)

Two of Disney's principles map cleanly and load-bearingly onto UI motion:

- **Slow In and Slow Out → EASING.** Add more frames (apply an easing curve) at the **start and end** of a move so it ramps up and settles instead of snapping. Strictly **linear movement** *"appears unnatural to the human eye"* — it reads as robotic/mechanical. Reserve linear for non-physical progress (see §3).
- **Staging → FOCUS.** Direct the user's attention to the one element that matters and **minimize the motion of everything else**. **Never animate everything at once** — competing motion destroys the staging.

Scope note: do **NOT** present "Anticipation" as a hover / communicate-interactivity device. That framing was **REFUTED (1-2)** in research and is not part of the verified guidance.

Sources: IxDF — *UI Animation: How to Apply Disney's 12 Principles*, corroborated by Material Design motion guidance.

## 3. Easing tokens (Material Design 3 — exact cubic-bezier values)

Easing describes how velocity changes across a transition. M3 splits easing into a **Standard** set (small, utility transitions) and an **Emphasized** set (large, expressive transitions).

**Standard set** — small/utility transitions:

| Token | `cubic-bezier` | Use for |
|---|---|---|
| `standard` | `cubic-bezier(0.2, 0, 0, 1.0)` | Elements that begin **and** end on screen |
| `standard.decelerate` | `cubic-bezier(0, 0, 0, 1)` | **Entering** elements |
| `standard.accelerate` | `cubic-bezier(0.3, 0, 1, 1)` | **Exiting** elements |
| `linear` | `cubic-bezier(0, 0, 1, 1)` | Progress indicators / non-stylized, constant-rate motion |

**Emphasized set** — expressive/large transitions:

| Token | `cubic-bezier` | Use for |
|---|---|---|
| `emphasized` | *(no single bezier — see note)* | Elements that begin **and** end on screen |
| `emphasized.decelerate` | `cubic-bezier(0.05, 0.7, 0.1, 1.0)` | **Entering** elements |
| `emphasized.accelerate` | `cubic-bezier(0.3, 0.0, 0.8, 0.15)` | **Exiting** elements |

> **`emphasized` is not a single CSS `cubic-bezier`.** Its true curve is a **two-segment** bezier that a single `cubic-bezier()` cannot express. M3 says **use Standard as the fallback**, or approximate the two-segment curve with the CSS `linear()` easing function on **Chrome 113+** (see §5 for the `linear()` technique).

**The entering / exiting / persistent rule:**

- **Entering** elements → use a **DECELERATE** curve (fast in, gentle settle).
- **Exiting** elements → use an **ACCELERATE** curve (gentle start, fast off-screen).
- **Persistent** elements (already on screen, moving to a new position) → use the **base** (`standard` / `emphasized`).

> **Legacy gotcha.** Material 2's old standard easing `cubic-bezier(0.4, 0, 0.2, 1)` still appears in a lot of code and component libraries. M3's `standard` is **front-loaded** — `cubic-bezier(0.2, 0, 0, 1.0)` — **not** the symmetric M2 curve. When reviewing, flag `0.4, 0, 0.2, 1` as M2-era and prefer the M3 token.

Source: Material Design 3 — Easing and duration tokens & specs.

## 4. Duration tokens (Material Design 3, ms)

Duration is a token scale, not a free dial. The classic/stable M3 scale:

| Token | ms | | Token | ms |
|---|---|---|---|---|
| `short1` | 50 | | `long1` | 450 |
| `short2` | 100 | | `long2` | 500 |
| `short3` | 150 | | `long3` | 550 |
| `short4` | 200 | | `long4` | 600 |
| `medium1` | 250 | | `extra-long1` | 700 |
| `medium2` | 300 | | `extra-long2` | 800 |
| `medium3` | 350 | | `extra-long3` | 900 |
| `medium4` | 400 | | `extra-long4` | 1000 |

Pairing guidance:

- **Small / utility** transitions → **standard** easing + **short/medium** durations (**50–400ms**).
- **Large / expressive** transitions → **emphasized** easing + **long** durations (**450–600ms**).
- **Extra-long (700ms+)** is reserved for **ambient / auto-advancing** motion with **no user input** waiting on it (e.g., a hero that animates on its own). Never block a user gesture behind a 700ms+ animation.
- **Duration scales with the transition's area / distance** — a small chip toggling color is short; a full-screen sheet sliding in is long.

> **Evolution note (2025–2026).** Material 3 **"Expressive"** is shifting its motion system toward a **spring / physics engine** (see §5). Treat the duration and easing tokens above as the **classic/stable M3 token scale**, and cross-check the latest `m3.material.io/styles/motion` for **spring parameters** when targeting the newest M3 Expressive components.

Source: Material Design 3 — Easing and duration tokens & specs.

## 5. Spring physics vs duration-based (verified 3-0)

Two fundamentally different models drive animation.

**Duration-based (tween / cubic-bezier).** A fixed **duration** plus a **timing curve**. The animation runs from `0 → duration` on a clock, **regardless of what the user does**. Predictable; easy to reason about; the right tool for most discrete UI state changes.

**Spring.** Described by **physical parameters** — `mass`, `stiffness` (tension), and `damping` (friction). There is **no fixed duration**; duration **emerges** from the physics as the system settles. Springs feel more natural, especially in the **deceleration / settle** phase, because they decay the way physical objects do.

```
spring(mass, stiffness, damping)
  ↑ stiffness  → snappier, faster to target
  ↑ damping    → less oscillation (overshoot dies out sooner)
  ↑ mass       → more sluggish, more momentum
```

**Apple's simplified API (WWDC23)** abstracts the raw physics into two intuitive dials:

```swift
// duration = perceptual settle time; bounce = overshoot
withAnimation(.spring(duration: 0.5, bounce: 0.3)) { ... }
// bounce 0.0 → no overshoot   |   bounce 1.0 → very bouncy
```

SwiftUI's default `withAnimation` **is a spring**, and standard iOS transitions are **spring-driven** — spring is the platform default on Apple, not an exotic choice.

**The killer feature is interruptibility.** A spring **preserves velocity when re-targeted mid-flight**: drag something, let go, change your mind and drag the other way, and the motion stays smooth because current velocity carries into the new target. A cubic-bezier **resets velocity to zero** on re-target, producing a visible **jump / stutter**. This matters specifically for:

- drag and swipe-back gestures
- fast tab switches
- stacking toasts / notifications that re-layout while in motion

**Scope this carefully (verified caveat).** Do **NOT** claim "springs are always better."

- Springs are **more compute-intensive** than a fixed tween.
- **Low-bounce springs are closely emulable** by a good easing curve — for non-interruptible, fire-and-forget transitions, a cubic-bezier is often the simpler, cheaper, equally good choice.
- The spring's **irreplaceable** advantage is **velocity transfer on interruption**. If the animation can't be interrupted mid-flight, you usually don't need a spring.

**CSS has no spring primitive.** Approximate one by **sampling the spring curve** into the CSS `linear()` easing function (**Chrome 113+**, Baseline 2024):

```css
/* spring approximation: many sampled stops fed to linear() */
.sheet {
  transition: transform 0.6s linear(
    0, 0.009, 0.035, 0.078, 0.137, 0.21, 0.297, 0.394, 0.5,
    0.61, 0.717, 0.816, 0.9, 0.964, 1.005, 1.022, 1.02, 1.005, 1
  );
}
```

(Tools like Josh Comeau's spring generator and Motion's utilities produce these stop lists.)

Sources: Josh W. Comeau — *A Friendly Introduction to Spring Physics*; Motion — Performance docs; Apple WWDC23 session 10158 (*Animate with springs*).

## 6. Performance (verified 3-0)

**Frame budget.** The screen refresh sets the deadline:

- **60fps → ~16.7ms per frame**
- **120fps → ~8.3ms per frame**

Miss the budget and the browser drops frames — visible **jank**. Everything below is about staying inside that window.

**The render waterfall (per frame).** When something changes, the browser may run, in order:

```
Recalculate Style → Layout (reflow) → Paint → Composition
```

The earlier in the chain a change lands, the more work cascades after it. The goal is to touch the chain **as late as possible**.

**Cheap properties: `transform` and `opacity`.** Animating `transform` and `opacity` is the cheapest path — on a **promoted compositor layer** they're handled in the **Composition** step (often on the **GPU, off the main thread**), triggering only **style recalculation**, with **no layout and no paint**. Because they're off the main thread, they can **stay smooth even when the main thread is busy** (e.g., running JS).

> **CRITICAL PRECISION (an unconditional claim was REFUTED 1-2).** `transform`/`opacity` are cheap **ONLY** once the element is **promoted to a compositor layer** **AND** the property doesn't trigger layout/paint. **Layer promotion is conditional** (browser heuristics, `will-change`, etc.), and **over-promoting layers HARMS performance** (memory, layer-management cost). Do **NOT** say "compositor-only in every browser, unconditionally." The accurate statement is: *prefer `transform`/`opacity` because they **can** be composited cheaply — but promotion isn't free or guaranteed.*

**Expensive properties.** Animating **geometry / position** — `left`, `top`, `margin`, `width`, `height`, `font-size`, `border-width` — triggers the **full** `style → layout → paint → composite` chain **every frame**. Avoid animating these; reach for a `transform` equivalent (e.g., `translateX` instead of `left`, `scale` instead of `width`).

| Property | Pipeline cost |
|---|---|
| `transform`, `opacity` | Composite only *(once promoted; conditional)* |
| `color`, `background-color`, `box-shadow` | Paint + Composite |
| `width`, `height`, `top`, `left`, `margin`, `font-size` | **Layout + Paint + Composite** (most expensive) |

**Takeaway.** Hardware-accelerated (compositor) animations of `transform`/`opacity` stay smooth largely **regardless of main-thread load** — the single most reliable lever for jank-free motion.

Sources: MDN — *Animation performance and frame rate*; MDN — *CSS and JavaScript animation performance*; web.dev — *Rendering performance*; Motion — Performance docs.

## 7. Library & API landscape (verified 3-0)

- **Default to CSS** transitions/animations where possible; **escalate to JavaScript only when the animation is genuinely complex** (sequencing, dynamic targets, physics, scroll-linking) — MDN. The real performance lever is **which properties you animate** (compositor-only `transform`/`opacity`), **not** CSS-vs-JS per se. A bad CSS animation of `width` janks; a good JS animation of `transform` is smooth.
- **Web Animations API (WAAPI)** — the standards-based JS approach, available in most modern browsers. Use it when you need significant programmatic control without pulling in a library.
- **JS libraries: Motion (formerly Framer Motion) and GSAP** — declarative, spring-capable, and able to run animations **off the main thread** (WAAPI-backed) for many cases.
- **CSS spring approximation** — sample the spring curve into `linear()` (Chrome 113+, see §5).

Cross-platform native:

- **React Native — Reanimated** runs animations on the **UI thread (off the JS thread)**, so they hold **60fps** even when the JS thread is busy.
- **Flutter** — **implicit** animations (`AnimatedFoo` widgets) for simple state-driven motion vs **explicit** animations (`AnimationController` + `Tween`) for full control.

Sources: MDN / web.dev — *CSS vs JavaScript animation performance*; Flutter — *Animations* docs; Motion docs.

## 8. Reduced motion (accessibility — verified)

Reduced motion is a **generation-time obligation**, not an optional polish. Every animation the plugin emits must honor it. Maps to **WCAG SC 2.3.3 Animation from Interactions** (see `accessibility.md`).

**Honor the platform setting:**

| Platform | Hook |
|---|---|
| Web | `@media (prefers-reduced-motion: reduce)` |
| UIKit | `UIAccessibility.isReduceMotionEnabled` |
| SwiftUI | `@Environment(\.accessibilityReduceMotion)` |

**Replace, don't remove.** Functional animation carries meaning (orientation, continuity, feedback — see §1). Under reduced motion, **swap the movement for a dissolve / opacity fade / color shift** — **not nothing**. Removing the transition entirely can break the user's spatial model.

```css
@media (prefers-reduced-motion: reduce) {
  .panel {
    /* replace slide with a quick cross-fade, keep the feedback */
    transition: opacity 150ms ease;
    transform: none;
  }
}
```

**Problem triggers to drop or tame** under reduced motion:

- spinning / rotation
- depth scaling (large objects zooming toward/away)
- parallax
- multi-axis / vortex motion

**Apple specifics:**

- Maintain **30–60fps** for game-like / continuous motion.
- **visionOS** — avoid **sustained oscillation near 0.2 Hz** (roughly one cycle per 5 seconds); people are highly sensitive to it and it is a vestibular trigger.

Cross-link: see `accessibility.md` for the WCAG criteria (2.2.2 Pause/Stop/Hide, 2.3.3 Animation from Interactions) and the broader `prefers-reduced-motion` reset.

## Sources

- [Apple HIG — Motion](https://developer.apple.com/design/human-interface-guidelines/motion) (corroborated by WWDC23)
- [Apple WWDC23 — Animate with springs (session 10158)](https://developer.apple.com/videos/play/wwdc2023/10158/)
- [IxDF — UI Animation: How to Apply Disney's 12 Principles of Animation to UI Design](https://www.interaction-design.org/literature/article/ui-animation-how-to-apply-disney-s-12-principles-of-animation-to-ui-design)
- [Material Design 3 — Easing and duration: tokens & specs](https://m3.material.io/styles/motion/easing-and-duration/tokens-specs)
- [Josh W. Comeau — A Friendly Introduction to Spring Physics](https://www.joshwcomeau.com/animation/a-friendly-introduction-to-spring-physics/)
- [Motion — Performance](https://motion.dev/docs/performance)
- [MDN — Animation performance and frame rate](https://developer.mozilla.org/en-US/docs/Web/Performance/Guides/Animation_performance_and_frame_rate)
- [MDN — CSS and JavaScript animation performance](https://developer.mozilla.org/en-US/docs/Web/Performance/Guides/CSS_JavaScript_animation_performance)
- [web.dev — Rendering performance](https://web.dev/articles/rendering-performance)
- [web.dev — CSS vs. JavaScript animations](https://web.dev/articles/css-vs-javascript)
- [Flutter — Animations](https://docs.flutter.dev/ui/animations)
