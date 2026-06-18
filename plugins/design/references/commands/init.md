# init — Build

**Invocation:** `/design init [target]`   **Status:** active

## Purpose
Establish the project's design foundation so every later command has strategic context to reason from. `init` finds or creates `PRODUCT.md` (the *why* and *for whom*) and, when there is code to read, offers to also generate `DESIGN.md` (the *how it looks*). It is the prerequisite the dispatcher's PRODUCT.md gate points generative commands back to. The deprecated alias `teach` maps here — treat an invocation of `teach` as `init`.

## Flow
1. **Locate existing context.** Look for `PRODUCT.md` and `DESIGN.md` in order: project root → `.agents/context/` → `docs/`, first hit wins; honor `DESIGN_CONTEXT_DIR` as an override. Format and precedence are defined in `context.md`.
2. **If PRODUCT.md exists,** read it, summarize what it already says, and skip the interview unless the user asks to revise it.
3. **If PRODUCT.md is missing,** run a short multi-round discovery interview — a few focused questions per round, not one wall of prompts. Cover, in roughly this order:
   - **Target users** — who they are, the job they're doing, the context they work in.
   - **Brand & tone** — voice, personality, the feeling to leave behind.
   - **Anti-references** — competitors, clichés, and the obvious category look to avoid (feeds the slop-test).
   - **Strategic principles** — the few decisions everything else should serve.
   - **Register** — confirm whether the design IS the product (brand) or SERVES it (product).
4. **Write PRODUCT.md** to the chosen location using the skeleton in `context.md`, including the optional `register:` field.
5. **Offer DESIGN.md when code exists.** If the repo has real styles/components, ask whether to also capture the current visual system, and if yes, hand off to the `document` command rather than duplicating its logic.
6. **Pick the register** — brand vs product, by the `register:` field, then task cue, then surface. Default **product** when no signal is clear.
7. **Recommend next steps.** Read setup signals (stack, whether `DESIGN.md` now exists, dirty files) and surface the 2–3 highest-value commands to run next, each with a one-line reason. Never auto-run them.

## Cites
- `context.md` — PRODUCT.md / DESIGN.md skeletons, location precedence, register selection, inline OKLCH palette guidance.
- `registers/brand.md` — the brand register, for choosing register during the interview.
- `registers/product.md` — the product (default) register.

## Output
A written `PRODUCT.md` (and, when code exists and the user opts in, a `DESIGN.md` via `document`), plus a recommended next step of 2–3 commands tailored to the project's current state.
