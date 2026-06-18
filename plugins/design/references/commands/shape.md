# shape — Build

**Invocation:** `/design shape [target]`   **Status:** active

## Purpose
Plan the UX and UI of a surface *before* a line of code is written. `shape` runs a short discovery pass — who the surface is for, what it must let them do, what constrains it — and turns that into a written, user-confirmed **design brief**: the goal framed, the information architecture and primary flow mapped, the key states named, and the visual direction agreed. It produces no code. Its product is intent that `craft` (or a hand build) can implement without guessing, so the build starts from a decision rather than a reflex.

## Flow
1. **Setup.** Run the dispatcher Setup in `SKILL.md`: `detect-stack` to scope platform and pick the register, load `PRODUCT.md`/`DESIGN.md` (this is a Build command, so the PRODUCT.md gate applies — if it is missing, run `init` first), and read at least one real component so the brief reuses what exists rather than inventing a parallel system.
2. **Clarify the goal and the users.** State, in one sentence each, the job this surface does and who is doing it. Pull tone, audience, and anti-references from `PRODUCT.md`; ask the user to fill any gap before continuing.
3. **Run a short discovery.** A few focused questions, not a wall: which screens/views are in scope, the single primary action, the secondary actions, the inputs and data on hand, and the hard constraints (existing components, platform, deadline). Honor platform conventions per the active platform KB.
4. **Sketch IA, flow, and states.** Map the information architecture (grouping by Gestalt proximity / common region, not boxes), the primary path through the surface, and every key state it must handle: empty, loading, error, success, and partial/edge. Missing states are where builds rot.
5. **Apply the load-and-friction lens.** Keep choices few (Hick's Law), put frequent and primary targets where they are fast to hit (Fitts's Law), and cut extraneous cognitive load — recognition over recall — per `ux-usability.md`. Note where the design law in `SKILL.md` will constrain the later build.
6. **No visual verification.** There is no browser or screenshot here; `shape` reasons over intent and code, not rendered pixels. Where a decision depends on how it looks rendered, flag it as a thing `craft` must confirm with the user.
7. **Write and confirm the brief.** Emit the brief and get explicit sign-off before any build. Run the slop-test at both altitudes on the proposed direction (`slop-test.md`) so the plan isn't the category reflex.

## Cites
- `ux-usability.md` — Hick's & Fitts's laws, cognitive load, recognition-over-recall, the state set.
- `principles.md` — hierarchy, Gestalt grouping, balance for the IA sketch.
- `context.md` — `PRODUCT.md`/`DESIGN.md` precedence and register selection. `slop-test.md` — distinctiveness of the proposed direction.

## Output
A written **design brief** — goal, users, IA, primary flow, enumerated states, visual direction, and open questions for `craft` — confirmed by the user. No source files are created or edited.
