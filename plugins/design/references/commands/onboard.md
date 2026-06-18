# onboard — Refine

**Invocation:** `/design onboard [target]`   **Status:** active

## Purpose
Design the first-run experience so new users reach value fast. It identifies the aha-moment and the shortest path to first value, then designs welcome and empty states that teach by showing, progressive disclosure that reveals depth as it's needed, and contextual hints placed where the question arises — not a front-loaded tour dump. It treats the blank screen as the first teaching surface and leans on recognition over recall so the user learns by doing, not by reading a manual.

## Flow
1. **Setup.** Run the `SKILL.md` workflow: detect-stack, load `PRODUCT.md`/`DESIGN.md` (run `init` first if `PRODUCT.md` is absent — `PRODUCT.md` defines who the user is and what "value" means here), pick the register, and open the existing surface so the first-run flow matches the product.
2. **Find the aha-moment.** Name the single moment a new user first feels the product's value, and the minimum path to get there. Cut anything between the start and that moment that isn't required.
3. **Design empty/welcome states that teach by showing.** Replace blank screens with a state that demonstrates the next action — an example, a sample, a primed call-to-action — so the user sees what to do rather than being told (recognition over recall, `ux-usability.md`).
4. **Use progressive disclosure.** Reveal advanced options and depth only as the user needs them; don't expose the full surface area on first contact (reduces extraneous load).
5. **Place contextual hints, not a tour.** Put guidance where the relevant control lives, triggered by where the user is — avoid the up-front modal tour that's dismissed and forgotten. Make affordances and signifiers clear so the path is obvious without narration (`ux-usability.md`).
6. **Run the slop-test (`SKILL.md` / `slop-test.md`).** Confirm the onboarding isn't the generic checklist/confetti/empty-illustration cliché; make the first-run feel specific to this product, not template onboarding.
7. **Confirm the result.** No live-browser or screenshot — reason over the code and describe the first-run path, then ask the user to confirm it lands the aha-moment.

## Cites
`ux-usability.md` (recognition over recall, affordances/signifiers, progressive disclosure / cognitive load), `principles.md` (hierarchy and focal point for the primed action), `slop-test.md` (avoid generic onboarding cliché).

## Output
The refined code with first-run welcome/empty states, progressive disclosure, and contextual hints implemented along the path to first value, plus a short note naming the aha-moment, the chosen path, and the slop-test result — and a request to confirm the rendered first-run experience.
