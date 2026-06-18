# optimize — Fix

**Invocation:** `/design optimize [target]`   **Status:** active

## Purpose
Diagnose and repair the things that make a UI feel slow — heavy initial loads, render thrash and layout shift, animation jank, oversized images, and bloated bundles — then edit the source to fix them. This command finds the bottleneck class, applies the targeted fix (composite-friendly animation, lazy and responsive images, deferred non-critical work), and explains the rendering cost being paid. Speed is a design quality. The one rule it never breaks: performance must be **measured**, and the actual profiling lives outside this plugin's reach — so it recommends the user verify.

## Flow
1. **Setup first.** Run the workflow in `SKILL.md` (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). optimize is generative, so the **PRODUCT.md gate applies** — if missing, run `init`. Use detect-stack to scope platform-specific fixes.
2. **Identify the bottleneck class.** Read the code and classify what's likely costing frames or load time: layout thrash (animating geometry, forced sync reflow), non-compositor animation, oversized/unoptimized images, render-blocking resources, or a large JS bundle. Name the class before touching anything.
3. **Fix animation on the compositor.** Move animations to `transform` and `opacity` so they can run in the Composition step — off the main thread, smooth even under JS load. Stop animating geometry (`left`, `top`, `width`, `height`, `margin`) which forces the full `style → layout → paint → composite` chain every frame (`motion.md` §6). Note that compositor cheapness is **conditional** on layer promotion, and over-promoting layers hurts — don't blanket `will-change` everything.
4. **Fix images.** Lazy-load below-the-fold media, serve responsive sizes (`srcset`/`sizes`, modern formats), and set intrinsic dimensions to prevent layout shift.
5. **Defer non-critical work.** Split and lazy-load non-critical bundles, defer/async non-blocking scripts, and move expensive work off the critical render path so first paint isn't blocked (`responsive.md` for the loading-vs-viewport interplay). Respect reduced-motion when reworking animation (`accessibility.md` SC 2.3.3).
6. **State the measurement limit.** Performance must be measured in a real browser/profiler (DevTools Performance panel, Lighthouse, frame timeline) — this plugin reasons over code, not a running profile, and can't confirm the gain. Recommend the user profile before and after to verify the fix is real, not assumed.

## Cites
`motion.md` (compositor `transform`/`opacity`, render waterfall, frame budget, expensive-property table, conditional layer promotion — the performance model), `responsive.md` (loading, responsive images, viewport), `accessibility.md` (reduced motion SC 2.3.3 when reworking animation).

## Output
The edited source with the performance fixes in place, plus a short note naming the bottleneck class, what changed and why, and an explicit recommendation to measure before/after in a real browser/profiler to confirm the improvement.
