# document — Build

**Invocation:** `/design document [target]`   **Status:** active

## Purpose
Read an existing codebase and write `DESIGN.md` — a spec of the visual system actually in use, so any agent picking up the project stays on-brand instead of guessing. Where `init` captures strategy, `document` captures the implemented look: the concrete colors, type, spacing, radii, elevation, and component patterns already committed to the code. An established codebase deserves a real spec, not a reflexive palette.

## Flow
1. **Scope the stack.** Run setup `detect-stack` (`"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/detect-stack.sh" --json .`) for platform and framework context, so token extraction matches how this project expresses style (CSS vars, Tailwind config, theme file, native tokens).
2. **Scan for the system.** Open real CSS / tokens / theme files and a few representative components. Inventory:
   - **Color** — background, surface, ink, accent, muted, and state colors.
   - **Typography** — families, the size scale in play, weights, line-height.
   - **Spacing** — base unit and scale (check it against the 8pt rhythm in `principles.md`).
   - **Radii** — the radius scale and its ceiling.
   - **Elevation** — border vs shadow conventions per surface.
   - **Components** — recurring patterns and the rules they follow.
3. **Extract concrete tokens.** Record the actual values found, not idealized ones. Prefer expressing color in **OKLCH** (per `context.md`); note where the code diverges from convention rather than silently correcting it.
4. **Confirm the descriptive language.** Ask the user to confirm the words for atmosphere and color character (e.g. the mood, the color strategy on the commitment axis) — the qualitative layer the code can't state on its own.
5. **Write DESIGN.md** to the context location (root → `.agents/context/` → `docs/`, or `DESIGN_CONTEXT_DIR`) using the `# Design System` skeleton in `context.md`: Color / Typography / Spacing / Radii / Elevation / Components.

## Cites
- `context.md` — the DESIGN.md skeleton, location precedence, OKLCH guidance.
- `principles.md` — 8pt spacing rhythm, type scale, neutral + accent structure to describe the system against.

## Output
A written `DESIGN.md` capturing the project's real visual system — concrete tokens plus confirmed descriptive language — that an agent can follow to stay consistent with what's already shipped.
