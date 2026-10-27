# Brainstorm: better CLI UI/UX, round 3: look and progress (2026-10-27)

Mode: standalone. Path: Full. Builds on `2026-10-27-better-cli-ux-2.md`: the checkbox host
picker, prompt-free multi-host `update`, the Claude update fix, and the `status` table all
still stand. This round adds the visual language and the progress indicator.

## Capsule

- **Outcome:** on a terminal, `toolu plugins install|update|remove` look like the old
  @clack installer: a `┌ toolu plugins install` header, a `◆` checkbox picker on rails,
  and per host a `◇` bar that advances per plugin with a spinner line naming the current
  plugin. Results come as colored `✔`/`●`/`✘` rows with versions, closed by a `└` summary
  with counts and elapsed time. `toolu status` uses the same rails for its table.
- **Material defaults/non-goal:**
  - Styling is decided once in `toolu-cli`. A `Ctx` field is set when stdout is a
    terminal and `TERM` is not `dumb`. `NO_COLOR` (read through `Env`) drops ANSI color
    only; glyphs and rails stay.
  - Piped stdout, `--json` and `--quiet` keep today's plain reports byte for byte, so
    scripts are unaffected.
  - Live output (picker, bar, spinner) goes to `/dev/tty`, as `prompt.rs` already does,
    never stdout. It is cleared before the report prints.
  - Progress covers the `toolu plugins` verbs only. `doctor` and other commands are out.
  - No new crate. Color and progress are a small module in `toolu-runtime` that both
    `crates/plugins` and the status verb can reach. The picker uses `nix` termios.
  - The cursor is never hidden, so a Ctrl-C mid-run cannot leave it hidden.
- **Repository evidence:**
  - `rules.json` capabilities: only `toolu-cli::output` may touch stdout/stderr, only
    `toolu-runtime` reads env vars, and opening `/dev/tty` is unrestricted (the
    `prompt.rs` precedent).
  - Root `Cargo.toml` builds clap without color on purpose.
  - The previous TypeScript installer used `@clack/prompts`; the user picked that look
    from previews.
- **Risk:**
  - The spinner needs a ticker thread writing `/dev/tty` while the host command blocks
    the main thread. The thread must stop and clear its line on success, failure and
    timeout.
  - The 300-line / 50-line limits force a split: style, progress, checkbox, key decoder.
  - Terminal width under 60 columns must truncate, not wrap.
  - Glyph rendering on odd terminals: fall back to plain when `TERM=dumb`.
- **Handoff:** `/delivery-flow:delivery-flow` on a fresh branch from `main`. Order:
  1. `fix(plugins)`: the update targets (Claude `@toolu` filter, catalog intersection,
     dedupe, current detection).
  2. `feat(cli)`: style plus progress module, rails reports.
  3. `feat(plugins)`: the checkbox picker with Cursor probe, and prompt-free multi-host
     `update`.
  4. `feat(cli)`: the `status` rails table.

## Target look (chosen)

```text
┌  toolu plugins install
│
◆  Which hosts should receive the plugins?
│  ◼ claude   Claude Code
│  ◼ codex    Codex
│  ◻ muse     Muse
│  ◼ cursor   Cursor
└  ↑/↓ move · space toggle · a all · enter

◇  claude  ████████████░░░░░░  8/12
│  ⠹ installing epic-orchestrator…

◇  claude
│  ✔ toolu              8.5.0  installed
│  ✔ ast-grep           8.5.0  installed
│  ● brainstorm         8.5.0  current
│  ✘ jev                failed: timeout
└  11 installed · 1 failed  (14.2s)
```

Colors: `◆`/`◇` cyan, `✔` green, `●` dim, `✘` red, a skipped or retired step yellow `▲`,
versions dim.

## Decisions

| Axis | Default | Evidence or Jev | Rejected |
|------|---------|-----------------|----------|
| Visual style | Clack rails | User picked it from three previews | Compact modern; minimal (color only) |
| Progress and color build | In-house module: ANSI gated on terminal and `NO_COLOR`, std-thread spinner, per-plugin bar | Jev `handrolled` 0.94 | indicatif + console + cliclack (0.00): dependency surface, cargo-deny duplicates, against the deliberate no-color clap build. indicatif for the bar only (0.06) |
| Progress scope | `toolu plugins install`, `update`, `remove` | Jev `plugins_only` 0.98 | Add `doctor` (0.02); every command (0.00) |
| Where styling is decided | `toolu-cli` sets a `Ctx` style flag (it alone may test stdout); verbs render styled or plain | `rules.json` stdio capability | Verbs probing the terminal themselves: breaks the capability rule |
| Plain path | Unchanged plain reports when stdout is not a terminal | `report.rs` byte-for-byte promise, script safety | Rails in piped output |

## Testable seams

- Pure renderers: bar frame, spinner frame, rails report, status table. Each takes
  width, style on/off and a fixed tick, and is asserted on exact strings.
- Key decoder: bytes to keys (arrows `ESC [ A/B`, `j`/`k`, space, `a`, Enter,
  `q`/Esc/0x03), a pure function. The checkbox state machine runs over a key list.
- The real-terminal path is checked by hand in a live session. The `--json`, piped and
  `NO_COLOR` paths get black-box `assert_cmd` tests showing plain output and no ANSI.
