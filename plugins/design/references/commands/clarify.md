# clarify — Fix

**Invocation:** `/design clarify [target]`   **Status:** active

## Purpose
Repair the words a UI uses — labels, button text, helper copy, instructions, and especially error messages — so the interface explains itself. Most "confusing UI" is confusing language: a vague label, a blaming error, a jargon term the user never learned. This command edits the source text in place: it makes labels specific and action-oriented, turns cryptic or accusatory errors into "what happened + how to fix it," and tightens microcopy so every screen says what something does and what to do next. Text is a core part of the interface, not a caption on it.

## Flow
1. **Setup first.** Run the workflow in `SKILL.md` (detect-stack, load `PRODUCT.md`/`DESIGN.md`, load the command reference, load the register). clarify is generative, so the **PRODUCT.md gate applies** — if it's missing, run `init` first; brand voice and audience live there.
2. **Inventory the text.** Read the target and collect every user-facing string: labels, placeholders, button text, tooltips, empty-state copy, validation and error messages, confirmations. Note which are unclear, jargon-heavy, vague, or blaming.
3. **Rewrite labels to be specific and action-oriented.** Replace abstract nouns with the action or object ("Submit" → "Send invite"; "Settings" → name the actual setting). Recognition over recall — the label should name the choice so users don't have to remember what it does (`ux-usability.md`, heuristic #6).
4. **Fix error messages to say what happened + how to fix it.** Never blame the user, never leak a stack trace or error code alone. State the problem in plain words, then the concrete next step. Prefer prevention where the copy can stop the error up front (`ux-usability.md`, heuristic #5; slips vs. mistakes). Pair the message with visible feedback so the user knows the system registered the action (heuristic #1).
5. **Match the voice to the brand/tone.** Align word choice, formality, and rhythm to the tone and `register:` in `PRODUCT.md` (`context.md`). A playful product and a clinical one phrase the same warning differently.
6. **Keep it concise and consistent.** Cut filler, use one term per concept across the whole surface (don't alternate "delete"/"remove"/"trash"), and stay parallel across siblings. Match `principles.md` minimalism — say only what earns its place.
7. **No visual verification.** This plugin reasons over code, not rendered pixels: it can't confirm truncation, RTL, or how copy wraps in the live UI. Confirm the rendered result with the user.

## Cites
`ux-usability.md` (error prevention #5, recognition over recall #6, visibility/feedback #1, slips vs. mistakes, Nielsen heuristics), `context.md` (tone, voice, `register:`), `principles.md` (minimalism, consistency).

## Output
The edited source with the rewritten strings in place, plus a short note listing each change and why — what was unclear, jargon, or blaming, and what the new copy does better.
