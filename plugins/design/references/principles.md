# Visual Design Principles

The foundational visual rules this plugin applies and enforces when evaluating or generating UI.

> Provenance: seeded from adversarially-verified deep research (3-vote). Primary sources cited. Mark anything not independently verified.

## The five principles (verified — NN/g)

Per [NN/g](https://www.nngroup.com/articles/principles-visual-design/), five principles — **scale, visual hierarchy, balance, contrast, Gestalt** — "not only create beautiful designs, but also increase usability when applied correctly."

### Scale
- Relative size signals **importance and rank**.
- Use **no more than 3 sizes** in a composition.
- Make the **most important element the biggest**.

### Visual hierarchy
- Guide the eye **in order of importance**.
- Levers: variations in **scale, value, color, spacing, and placement**.

### Balance
- **Visual weight equally distributed about an axis** — equally distributed, *not* necessarily symmetrical.
- Three kinds: **symmetrical, asymmetrical, radial**.

### Contrast
- One of the five named principles ([NN/g](https://www.nngroup.com/articles/principles-visual-design/)). Used to separate elements and draw attention.
- Specific contrast *ratios* for legibility/accessibility live in **[accessibility.md](./accessibility.md)** — cross-link, don't duplicate.

### Gestalt
- See the dedicated section below.

## Gestalt grouping (verified — NN/g)

Timeless perceptual psychology, applied pervasively in UI. Two cues are load-bearing for this plugin:

### Proximity
- Elements **close together are perceived as one group** (shared function/traits); elements **spaced apart read as separate groups**. ([NN/g — Proximity](https://www.nngroup.com/articles/gestalt-proximity/))
- One of the **strongest grouping cues** — it **can overpower** similarity of color/shape.
- **Not absolute:** strong similarity can override proximity under some conditions (Quinlan & Wilton, cited via NN/g).
- Practical rule: **communicate relationships by varying whitespace**, not just by adding lines or boxes.

### Common Region
- Items **inside a shared boundary** (a border or a background fill) are perceived as a **group sharing characteristics/function**; it helps users grasp structure. ([NN/g — Common Region](https://www.nngroup.com/articles/common-region/))
- **Caveat:** don't over-use borders — too many boundaries create clutter. Prefer background/whitespace where it suffices.

---

## Spacing, type, and color conventions

> The items below are **established design convention**, included for completeness. They did **not** come from the verified NN/g set above. Each is tagged **(convention — not independently verified here)** and cited where a source was supplied.

### Spacing & rhythm — the 8pt grid
- Space and size in **multiples of 8** (use **4** for fine adjustments) for consistent rhythm. Aligns with Material's **4dp–8dp baseline grid**. *(convention — not independently verified here; per Material spacing guidance.)*
- Benefit: predictable rhythm, fewer arbitrary gaps, easier handoff to dev.

### Typographic scale
- Use a **modular scale** — a constant ratio (e.g. **1.2 / 1.25 / 1.333**) — rather than arbitrary sizes. *(convention — not independently verified here.)*
- **Limit the number of sizes** in play (echoes the verified "no more than 3 sizes" for scale).
- Target **line length ~45–75 characters** for readability. *(convention — not independently verified here.)*

### Color
- **Limit the palette;** use a **neutral + accent** structure. *(convention — not independently verified here.)*
- **Do not rely on color alone** to convey meaning — pair it with text, icon, or shape (ties directly to accessibility). *(convention — not independently verified here.)*
- Contrast **thresholds** (e.g. WCAG ratios) are defined in **[accessibility.md](./accessibility.md)** — see there, not here.

## Sources

- [NN/g — The Principles of Visual Design](https://www.nngroup.com/articles/principles-visual-design/) (verified)
- [NN/g — Proximity Principle](https://www.nngroup.com/articles/gestalt-proximity/) (verified)
- [NN/g — Common Region](https://www.nngroup.com/articles/common-region/) (verified)
- Quinlan & Wilton — cited within NN/g Proximity article re: similarity overriding proximity
- Material Design — spacing / baseline grid guidance *(convention)*
- See **[accessibility.md](./accessibility.md)** for contrast ratios and color-independence thresholds.
