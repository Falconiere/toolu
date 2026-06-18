# colorize — Enhance

**Invocation:** `/design colorize [target]`   **Status:** active

## Purpose
Add strategic color to a monochromatic or under-colored UI so color earns its place as a tool for state and hierarchy, not decoration. This command picks a color strategy appropriate to the register, introduces accents where they guide attention or signal meaning, and verifies every new pairing against accessible contrast thresholds. It works in OKLCH so lightness stays perceptually even across hues, and it deliberately avoids the warm-neutral cream/sand default that reads as machine-made. The pass edits source in place and keeps the surface legible.

## Flow
1. **Setup.** Run the SKILL.md setup workflow (detect-stack, load `PRODUCT.md`/`DESIGN.md`, command reference, register). This is an Enhance command, so the `PRODUCT.md` gate applies — run `init` first if it is missing. Open the existing CSS/tokens to reuse what works and preserve brand identity.
2. **Pick a color strategy.** Choose one strategy before any colors — restrained, committed, full-palette, or drenched (`slop-test.md`) — matched to the register: a product surface usually wants restraint and a clear accent; a brand surface can commit harder. Decide the strategy first, then derive colors from it.
3. **Place color where it carries meaning.** Introduce color to signal state (success, warning, danger, active) and to reinforce hierarchy (primary action, emphasis), not to fill space. Don't rely on color alone — pair it with text, icon, or shape (`principles.md`, color).
4. **Work in OKLCH.** Compose accents in OKLCH so lightness and chroma stay perceptually consistent across hues (`context.md`). Avoid the cream/sand/beige warm-neutral default — it is the first training-data reflex (`slop-test.md`).
5. **Verify every new pairing.** Check each text/background and UI-component pair against thresholds: body ≥4.5:1, large text ≥3:1, UI/graphical ≥3:1, placeholders ≥4.5:1 (`accessibility.md`, SC 1.4.3 / 1.4.11). Gray on a colored background washes out — use a darker shade of the background's own hue; for body text, push toward the ink end.
6. **Confirm the result.** No live-browser/screenshot is available — reason over the code and compute contrast, then ask the user to confirm the rendered palette reads as intended on real screens.

## Cites
`principles.md` (color: limit palette, neutral + accent, don't rely on color alone), `accessibility.md` (contrast thresholds, SC 1.4.3 / 1.4.11), `context.md` (OKLCH palette composition), `slop-test.md` (color strategy, avoid the cream/sand reflex), the active register (`registers/brand.md` or `registers/product.md`).

## Output
The edited source with the new color tokens and applied accents, plus a short note: the chosen strategy, where color was placed and what it signals, and the measured contrast of each new pairing.
