# Brainstorm: better CLI UI/UX (2026-10-27)

Mode: standalone. Path: Full (unclear goal, public interface: CLI output).

## Capsule

- **Outcome:** the native `toolu` binary reads well for a person at the terminal. Every
  human-mode verb prints text, not JSON. Lists line up and explain their marks. A bare
  namespace lists its verbs. Read-only verbs work when several hosts are detected. A dry
  run says what is already installed. `doctor` ends with a one-line summary.
- **Material defaults/non-goal:** work on `main`'s Rust CLI (`crates/cli`, `crates/plugins`,
  verb crates), on a fresh branch. No new dependencies. No color in this iteration. Not
  in scope: arrow-key pickers, spinners, a TUI, `--json` shape changes, exit-code changes,
  and the TypeScript CLI.
- **Repository evidence:** `improve-cli` is 235 commits behind `main`. Its TypeScript CLI
  is deleted on `main`, where `@toolu/plugins` prints a deprecation notice and exits `69`.
  `main` already prompts on a TTY in Rust (`crates/plugins/src/prompt.rs`). clap is built
  without color on purpose (root `Cargo.toml`). `check-cli-compat` freezes commands, flags
  and exit codes, not text layout.
- **Risk:** scripts that parse `toolu status` without `--json` would break; it needs a
  CHANGELOG note and `--json` as the documented path. Hiding `hook` from `--help` may trip
  `check-cli-compat` or the `docs-cli` drift gate. `report.rs` promises the TypeScript
  installer's text "byte for byte", so the insta snapshots and that comment change too.
- **Handoff:** `/delivery-flow:delivery-flow` on a branch cut from `main`, not `improve-cli`.

## Observed defects (toolu 8.5.0, real runs)

| # | Command | Today | Proposed |
|---|---------|-------|----------|
| 1 | `toolu status` | Raw JSON object in human mode | Short text block (branch, gate, push review, waivers); JSON only under `--json` |
| 2 | `toolu plugins list` | Marks `=`/`-` unexplained; name column fixed at 16, `epic-orchestrator` overflows | Width from the longest name; words instead of marks (`installed 8.5.0`, `not installed`, `disabled`) |
| 3 | `toolu plugins list` with several hosts, no TTY | Fails: "several hosts found … Choose one with --host." | Read-only: one section per detected host, as `install` already does |
| 4 | `toolu plugins install --dry-run` | Plans 12 installs when all 12 are at 8.5.0 | Mark each planned line `(installed 8.5.0)` or skip it, then a count summary |
| 5 | `toolu config` (no verb) | "toolu config: a verb is required" | Print the namespace's verbs (clap `arg_required_else_help`), still exit `64` |
| 6 | `toolu --help` | Internal `hook` listed first | Hide it, as plugin `hook` verbs already are |
| 7 | `toolu doctor` | ok/warn lines, no summary | Last line: `N ok, N warn, N fail` |

## Axes

- **Target codebase.** Default: Rust `toolu` on `main`. Evidence: the TS CLI is deleted
  and deprecated. Risk: none; the branch's commits are superseded by `prompt.rs`.
- **Scope.** Default: fix the observed defects inside the current contract. Jev
  (`choice`, state = evidence above) picked `polish` with confidence 1.0 over `toolkit`,
  `tui`, and `typescript`.
- **Color.** Default: none this iteration. Jev was split (no_color 0.52, inhouse_ansi
  0.48, clap_color 0.00). It is cheap to add later and easy to reverse, so defer. If added
  later: a small helper in `crates/cli` that styles marks only when the stream is a
  terminal and `NO_COLOR` is unset, with no new crate.
- **Compatibility.** Text layout is outside `check-cli-compat`, but `status` text is a
  visible behavior change: ship it as `feat(cli)` with a CHANGELOG line, not `fix`.

## Rejected alternatives

- **Toolkit** (clap color/anstream, dialoguer or inquire, indicatif): new dependency
  tree against cargo-deny and the deliberate no-color clap build. It is styling, and it
  fixes none of the observed defects.
- **Full TUI** (ratatui): large surface, fails the 300/50 bar quickly, and no request
  calls for it.
- **Continue the TypeScript CLI**: dead code on `main`.

## Open

- Whether #4 skips installed plugins or only annotates them. Skipping changes what
  `install` runs, so default to annotate.
- Whether to add color as a second, separate PR after these land.
