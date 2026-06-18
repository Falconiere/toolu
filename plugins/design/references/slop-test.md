# The AI-Slop Test

**Goal:** if a person could glance at the interface and say "AI made that" with no hesitation, it failed. The test below is the gut-check to run *before* committing to a direction, and again on the result.

> Provenance: reimplemented in our own words from the concepts behind the `impeccable` skill (github.com/pbakaus/impeccable, Apache-2.0). Clean-room, MIT. The mechanism (category reflex) is the core idea; the specific axis and thresholds are tagged **(convention)**.

---

## The category-reflex check (two altitudes)

The failure mode is reaching for the **first thing training data associates with the category**. Test for it at two altitudes.

### First-order reflex

If someone could guess the theme and palette from the **category alone**, you've hit the first reflex.

- *"fintech"* → navy + gold.
- *"AI tool"* → SaaS-cream background + violet gradient.
- *"health app"* → soft teal + rounded everything.

**Worked example.** Brief: "design a landing page for an AI writing assistant." Reflex output: cream background, violet-to-indigo gradient hero, glass cards. Anyone who saw it would name the category instantly.

**Fix:** write **one concrete sentence of physical scene** — who uses this, where, under what light, in what mood — then pick a color strategy until the answer is no longer obvious from the domain. E.g. *"A novelist drafting at 1am at a wooden desk, one warm lamp on."* That scene points somewhere specific (deep ink, warm low-key light, paper-as-figure) rather than to the category default.

### Second-order reflex

One tier deeper: if someone could guess the aesthetic from **category + the obvious anti-reference**, you're still on rails — just the *contrarian* rail.

- *"AI tool that's NOT SaaS-cream"* → editorial-typographic (big serif, lots of whitespace).
- *"fintech that's NOT navy-and-gold"* → terminal-native dark + monospace.

**Worked example.** Same AI-writing brief. You reject cream, so you reach for a giant serif headline on white with generous margins — the predictable "we're different" move. It's still guessable, just from "AI tool, but tasteful."

**Fix:** rework until **neither** the first-order nor the second-order answer is obvious. The scene sentence is what gets you off both rails, because it commits to specifics no reflex would predict.

---

## Color-strategy commitment axis *(convention)*

A tool for breaking the reflex: deliberately choose **how far** you commit to color, rather than defaulting to "tasteful accent."

1. **Restrained** — near-neutral surface, one accent used sparingly.
2. **Committed** — a clear brand color carried through interactive and emphasis states.
3. **Full-palette** — multiple coordinated hues doing real work (categories, data, states).
4. **Drenched** — color saturates the surface itself; the background *is* the brand.

Most generated UI sits passively at level 1 because it's the safe default. Picking 2–4 on purpose, justified by the scene sentence, is one of the fastest ways out of slop.

---

## The warm-neutral default *(convention)*

The cream / sand / beige band is the **saturated AI default** — the single most over-produced neutral in generated UI.

- Roughly **OKLCH L 0.84–0.97, C < 0.06, hue 40–100**.
- Token names like `--cream`, `--sand`, `--paper` are themselves a tell.

Warm neutrals are fine *when chosen for a reason from the scene*. They are slop when they're the unexamined background. If you didn't decide on warm, don't default to it — see the tinting guidance in [context.md](./context.md) (don't default-tint warm).
