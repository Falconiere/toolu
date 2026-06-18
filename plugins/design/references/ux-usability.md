# UX & Usability Foundations

The interaction and usability laws this plugin reasons with when evaluating flows and components.

> Provenance: seeded from adversarially-verified deep research (3-vote). Primary sources cited. Mark anything not independently verified.

## Nielsen's 10 usability heuristics (verified — NN/g)

Per [NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/), these are **"broad rules of thumb... not specific usability guidelines."** Treat them as evaluation lenses, not checklists.

The ten, by name:
1. **Visibility of system status**
2. Match between system and the real world
3. User control and freedom
4. Consistency and standards
5. **Error prevention**
6. **Recognition rather than recall**
7. Flexibility and efficiency of use
8. Aesthetic and minimalist design
9. Help users recognize, diagnose, and recover from errors
10. Help and documentation

### Anchor heuristics (the ones this plugin leans on)
- **#1 Visibility of system status** — keep users informed through **timely, appropriate feedback**.
- **#5 Error prevention** — *"the best designs carefully prevent problems from occurring in the first place."* Prefer prevention over good error messages.
- **#6 Recognition rather than recall** — *"minimize the user's memory load by making elements, actions, and options visible."*

## Affordances vs. signifiers (verified — Norman)

Per [Don Norman — Signifiers, Not Affordances](https://jnd.org/signifiers-not-affordances/):

- An **affordance** (following Gibson) is a **relationship** — *"the actions possible by a specific agent on a specific environment"* — and it **exists independent of perception**.
- The **perceivable part** of an affordance **is a signifier**.
- If a signifier is **deliberately placed by a designer**, it is a **social signifier**.
- **Design directive:** *"Forget affordances, provide signifiers"* — users search for **perceivable clues** to what an element is for.

> **Caveat (a narrower claim was refuted 0-3):** Do **NOT** define "social signifier" as *only* deliberately-placed designer cues. Norman's social signifiers also include **accidental/incidental** cues (e.g., a worn path, a crowd). The deliberate kind is a subset, not the definition.

## Error taxonomy: slips vs. mistakes (verified — NN/g)

Per [NN/g — Slips](https://www.nngroup.com/articles/slips/):

- **Slips** — *unconscious* errors made on autopilot: you **intend one action but perform another similar one**. Common with skilled, routine tasks.
- **Mistakes** — *conscious* errors from **goals that are inappropriate for the task** (a flawed plan, even if executed correctly).
- **Priority:** **prevent errors before they happen** > recover after the fact (reinforces heuristic #5).

> **Nuance:** the full Norman/Reason taxonomy adds a third category, **lapses** (memory-based failures), which the NN/g article omits.

---

## Supporting laws and notes

> The items below were fetched as sources but did **not** surface as distinct verified findings. Tagged **(secondary source)** and cited.

### Hick's Law
- Decision time **increases with the number/complexity of choices** (logarithmically). *(secondary source)*
- Apply: **reduce or segment choices** to speed decisions; chunk long menus. ([Laws of UX — Hick's Law](https://lawsofux.com/hicks-law/))

### Fitts's Law
- Time to acquire a target is a function of **distance to** and **size of** the target. *(secondary source)*
- Apply: make **important/frequent targets bigger and closer**. Touch-target *sizes* live in **[responsive.md](./responsive.md)** — cross-link, don't duplicate. ([Laws of UX — Fitts's Law](https://lawsofux.com/fittss-law/))

### Cognitive load & feedback
- **Cognitive load:** minimize **extraneous** load — strip work that isn't core to the task (ties to heuristic #6, recognition over recall). *(secondary source)*
- **Feedback:** **every action needs a perceptible response** (ties to heuristic #1, visibility of system status). *(secondary source)*

## Sources

- [NN/g — 10 Usability Heuristics for User Interface Design](https://www.nngroup.com/articles/ten-usability-heuristics/) (verified)
- [Don Norman — Signifiers, Not Affordances](https://jnd.org/signifiers-not-affordances/) (verified)
- [NN/g — Slips: When Users Do What They Didn't Mean To Do](https://www.nngroup.com/articles/slips/) (verified)
- [Laws of UX — Hick's Law](https://lawsofux.com/hicks-law/) *(secondary source)*
- [Laws of UX — Fitts's Law](https://lawsofux.com/fittss-law/) *(secondary source)*
- See **[responsive.md](./responsive.md)** for touch-target sizing.
