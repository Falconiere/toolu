# toolu CLI look, progress and plugin update targets — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** Clack-style terminal output, a progress bar and checkbox host picker for `toolu plugins`, prompt-free multi-host `update` that touches only toolu's plugins, and a `toolu status` table.

Brainstorms: `docs/toolu/brainstorms/2026-10-27-better-cli-ux{,-2,-3}.md`. Revision 2 answers
spec review 1 (one blocker, fourteen should-fix, five consider).

## Problem

1. `toolu plugins update --host claude` runs `claude plugin update <name>@toolu` for other
   marketplaces' plugins (`cloudflare@cloudflare`, `exa@claude-plugins-official`), and each
   fails with "Plugin not found". `crates/plugins/src/host/claude.rs` `parse()` drops the
   marketplace and keeps every entry; `codex.rs` keeps only `toolu`. Claude's real listing
   here has 30 `toolu@toolu` rows (user scope plus project pins at 8.1.3, 8.1.4 and 8.2.0)
   and retired `context7`, `exa-search` and `jira` rows the catalog no longer has.
2. Every Claude update prints "updated" with no version, even when nothing changed. Claude's
   `plugin list --available --json` is `{available, installed}` with no offered versions.
3. `update` asks for a host (or exits 64) when several are installed. The user wants every
   host with toolu plugins updated without asking.
4. The installer UI is numbered prompts and bare `+ = x -` marks: no color, no progress
   during multi-minute runs, and a 16-column name field that `epic-orchestrator` overflows.
   `toolu status` prints raw JSON at a terminal. The user chose the @clack look of the
   former TypeScript installer from previews.

## Non-Goals

1. No `--json` change except `update` without `--host` (Interfaces). User decision: ship
   as `feat` with a compat note in `docs/cli/installer.md` and the PR, not `feat!`.
2. When stdout is not a terminal, plain renderers stay as they are. The one plain text
   change is Claude's update argv gaining `--json`.
3. No new crate. `crates/plugins` adds the workspace `nix` with the `term` feature.
4. Progress only for `toolu plugins install|update|remove`.
5. Per-project `toolu@toolu` pins are deduplicated, not refreshed per scope. The update
   argv has no `--scope`; Claude auto-detects it (`claude plugin update --help`: "default:
   auto-detect", 2.1.311).
6. `list` and `remove` keep single-host resolution; only their widget and rendering change.
7. No terminal-width probing (no `ioctl`; `unsafe_code` is forbidden). Progress lines are
   capped at 64 columns and picker rows at 76, cut with `…`.
8. SIGTERM or SIGHUP during the picker leaves raw mode set (no signal handler). This is
   a documented limit. Ctrl-C arrives as a byte in raw mode and cancels cleanly.

## Architecture

**Style and rails primitives** live in `toolu_runtime::term` (new
`crates/core/runtime/src/term.rs`), shared by `crates/plugins` and `crates/toolu` (a
plugin crate may not depend on another, and jscpd runs at 0):

- `Style { Plain, Mono, Color }` and `Tone { Accent, Good, Warn, Bad, Dim }`.
- `style_of(env, is_terminal)`: not a terminal or `TERM=dumb` gives `Plain`, a non-empty
  `NO_COLOR` gives `Mono`, otherwise `Color`.
- `paint`: ANSI SGR only for `Color`; `Mono` and `Plain` text has no `\x1b` at all.
- Builders: `intro(style, title)` gives `┌  title\n│\n`; `section(style, title)` gives
  `◇  title\n`; `row(style, glyph, tone, name, width, detail)` gives
  `│  <glyph> <name padded to width> <detail>\n`; `label_row(style, label, width, value)`
  gives `│  <label padded to width><value>\n` (no glyph slot); `outro(style, text)` gives
  `└  text\n` (`└\n` when `text` is empty); `truncate(text, cols)` cuts to `cols`
  characters with `…`.
- `Ctx` gains `style: Style` (default `Plain`; `Style` derives Debug, Clone, Copy,
  Default, PartialEq, Eq). `crates/cli` `output::style()`, the only stdout user under
  rules.json `stdio`, sets it in `dispatch::ctx_of` from `std::io::stdout().is_terminal()`
  and `Env::process()`. `--json` forces `Plain`. `crates/statusline/src/lib.rs:35`
  gains `..Ctx::default()`.

**Rails reports** (`crates/plugins/src/report/rails.rs`, new) map `Status` to glyph and
tone:

| Status | Glyph | Tone |
|--------|-------|------|
| installed, updated, removed | `✔` | Good |
| already, current | `●` | Dim |
| skew | `▲` | Warn |
| failed | `✘` | Bad |
| skipped | `○` | Dim |

A dry run's skipped row detail is `would run: <argv joined>`. `list` rows use `✔` Good
with the version when installed and enabled, `▲` Warn with `<version> disabled`, and `○`
Dim with `not installed`. Each host is a section, names are padded to the longest name,
and the footer is `└  <n> <status>` joined by ` · ` in Status order, zero counts omitted,
then `  (<secs with one decimal>s)`. The intro `┌  toolu plugins <verb>` goes to
`/dev/tty` at run start when the style is not `Plain` and `/dev/tty` opens. If it could
not be written, the stdout report starts with it.

**Progress** (`crates/plugins/src/progress.rs`, new):

- `Progress::open(style, quiet)` opens `/dev/tty`. It returns `None` for `Plain`
  (which `--json` forces), `quiet` or an open error. `Progress::to_writer(Box<dyn Write + Send>, Style)`
  is the test seam.
- `begin(host, total)`, `task(label)`, `advance()` and `end()` draw synchronously. A std
  thread only re-draws the spinner frame every 80 ms; `end()` and `Drop` stop and join
  it.
- Two lines: `◇  <host>  <20-cell bar> n/total` and `│  <braille frame> <task>…`.
  `total == 0` draws the spinner line only. The lines are capped at 64 columns.
- `end()` writes `\x1b[1A\r\x1b[2K` cursor-up/clear sequences so both lines vanish. The
  cursor is never hidden. Write errors are dropped; progress never fails a run.
- `frame(state, tick, style)` is pure.
- `Context` gains `style` and `progress: Option<Arc<Progress>>`; `Driver` gains `progress`.
  `install`, `update` and `remove` call it per plugin. Cursor and dry runs use `total == 0`.

**Checkbox picker** (`crates/plugins/src/prompt.rs` keeps `Ask`, `Choice` and `terminal()`;
new siblings `prompt/numbered.rs` (today's `Prompter`), `prompt/keys.rs`, `prompt/raw.rs`,
`prompt/checkbox.rs`):

- `Checkbox::new(tty: File, style)` implements `Ask`. `raw.rs` saves the termios
  attributes, clears ICANON, ECHO and ISIG with VMIN 1 and VTIME 0, and restores them
  on drop.
- `read_key(&mut impl KeyInput)` reads bytes. `KeyInput::byte()` blocks;
  `KeyInput::byte_soon()` waits about 100 ms by switching to VMIN 0 and VTIME 1.
- After `\x1b`, a `[` within the wait starts a CSI: `A` is Up, `B` is Down, anything
  else is Other. No byte in time means a lone Esc, which cancels. `k`/`j` are Up/Down,
  space toggles, `a` toggles all (all on becomes all off, otherwise all on), `\r` or
  `\n` is Enter, and `q`, Esc, `\x03` and EOF cancel.
- Drawing:
  - The list is `◆  message`, then rows `│  ◼|◻ label  hint`, with `>` and Accent on
    the cursor row and each row cut at 76 columns.
  - The last line is `└  ↑/↓ move · space toggle · a all · enter`.
  - A redraw moves up by the line count it last drew.
  - Enter with nothing chosen re-draws with `└  choose at least one`.
  - Confirm collapses the list to `◇  message  <chosen labels, comma-joined>`.
  - `one()` is the same list with `●`/`○` and the cursor row as the choice.
- `terminal(allowed, style)` returns the checkbox when the style is not `Plain`, stdin
  is a terminal and `tcgetattr` works on `/dev/tty`; otherwise the numbered prompt, as
  today. It only probes; it never leaves raw mode on.
- Raw mode is scoped to one prompt: each `many()` and `one()` call creates the raw guard
  on entry and drops it before returning. Between prompts, during progress and while
  host commands run, the terminal is cooked, so Ctrl-C stays a signal there. If raw mode
  then fails inside a call, the call falls back to the numbered prompt.

**Hosts** (`host.rs`): `detected(env)` is the PATH hosts (claude, codex, muse) plus Cursor
when its config root (`Roots` for `Host::Cursor`) is a directory or `cursor-agent` is on
PATH.

- `resolve()` with an `Ask` offers `detected` for `install`. For `list` and `remove` it
  offers the PATH hosts plus Cursor only when `Store` holds a toolu-marked plugin, so a
  stale `~/.cursor` never adds a "Which host?" question there. Without an `Ask` it
  counts PATH hosts only, so non-interactive runs are unchanged.
- One host is used without asking, as today.
- Labels: claude "Claude Code", codex "Codex", muse "Muse", cursor "Cursor".

**Update targets**:

- `claude::parse` keeps only ids whose marketplace is `toolu`; an id with no `@` is kept,
  as in Codex. Duplicate names keep the `scope == "user"` row, else the first.
- `update::run` (no names) targets the installed toolu entries. A name absent from the
  catalog gets a `skew` step: "retired from the toolu marketplace; remove it with
  `<hint>`".
  - Claude, Codex and Cursor hint: `toolu plugins remove <name> --host <host> --yes`.
  - Muse hint: `muse plugins remove <name>` (as `doctor`'s `skew_check::removal`).
- Claude's update argv is `claude plugin update <name>@toolu --json`. The stdout JSON line
  (captured, Interfaces) maps:
  - `outcome == "failed"`: `failed`, detail `message`.
  - `updateOutcome == "up_to_date"` with `refreshFailed`: `skew`, "could not refresh the
    marketplace; still at <newVersion>".
  - `up_to_date`: `current`, "current at <newVersion>".
  - `updated`: `updated`, "updated <oldVersion> → <newVersion>". A missing or `"unknown"`
    version gives "updated".
  - `skipped`, or any other `updateOutcome` value: `skew`, the `message`.
  - Exit 0 with no parsable line: `updated`, "updated".
  - Exit ≠ 0 with no parsable line: `failed`, stderr's first line.
- `Adapter::list_available()` returns `Option<Vec<String>>`, `None` for Claude. Claude's
  real `--available` output carries no offered versions, so `install` and `update` stop
  calling it there and skip the offered-version comparison for Claude. A Claude dry run
  lists every installed toolu plugin as would-run. Codex and Muse argv and mapping are
  unchanged. Test fixtures that feed Claude an array-shaped `claude-available.json`
  (`tests/dispatch_test.rs`) are removed or replaced by the real object shape.

**Prompt-free update** (`crates/plugins/src/dispatch/update.rs`, new; `dispatch.rs` keeps
the `--host` path):

- Without `--host`, `update` never calls `Ask`. For each PATH host it lists the installed
  toolu plugins first, before any prepare, pin, sync or clone. Claude and Codex use the
  filtered listing; Muse uses non-`foreign` entries. Cursor is a candidate only when
  `Store` holds a toolu-marked plugin.
- A failed or malformed listing counts as none; its note names the cause
  (`<host>: could not list its plugins: <first stderr line>; skipped`), and the
  nothing-to-update error lists the hosts that could not be read. The listing a
  host was read with is the one its update uses.
- Hosts with none get the text note `<host>: no toolu plugins installed; skipped` (rails:
  `◇  <host>` then `│  ○ no toolu plugins installed`) and are left out of the JSON `hosts`
  array.
- With names: each host updates only the names it has. A host that has toolu plugins
  but none of the names gets the note `<host>: <names, comma-joined> not installed;
  skipped` and is left out of JSON. A name installed on no detected host fails the run:
  exit 1, "<name> is not installed from toolu on any detected host".
- `--scope` is checked for Claude Code and ignored for the other hosts.
- A host whose verb returns `Err` (prepare, pin, sync) gets one `failed` step named after
  the host, with the error message and empty argv. The other hosts continue; exit 1.
- With `--host`, the behavior is today's.

**Status table** (`crates/toolu/src/status.rs` plus new `status/table.rs`): for
`Mono`/`Color` it prints:

```text
┌  toolu status
│
│  repository    <repo_root | not a git repository>
│  branch        <branch | HEAD (detached) | —>  ↑<ahead> ↓<behind>
│  host          <host>
│  working tree  <s> staged · <u> unstaged · <n> untracked
│  gate          ✔ passing | ✘ failing: <reason> (<k> files) | ▲ invalid: <reason> | ○ missing
│  push review   ✔ recorded | ▲ invalid: <reason> | ○ missing
│  waivers       none | waiver active | waiver pending | waiver active, waiver pending
└
```

The `↑/↓` part is shown only inside a repository, and the label column is 14 wide.
`Plain` keeps today's pretty JSON, and `--json` is unchanged.

Reused: `prompt::Ask`/`Choice`, `report.rs` plain renderers, `Step`/`Status`,
`host::available`, `Driver::versions`, `catalog::Catalog`, `cursor::store::Store`,
`toolu_runtime::env::Env`, `toolu_runtime::host::roots::Roots`. Size: the update fan-out
lives in `dispatch/update.rs`, and `verbs::install::run`'s per-plugin body moves to a helper.
Style and progress travel in `Context`/`Driver`, not new parameters.

## Interfaces / Schema

```rust
// toolu_runtime::term
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Style { #[default] Plain, Mono, Color }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone { Accent, Good, Warn, Bad, Dim }
pub fn style_of(env: &Env, is_terminal: bool) -> Style;
pub fn paint(style: Style, tone: Tone, text: &str) -> String;
pub fn intro(style: Style, title: &str) -> String;
pub fn section(style: Style, title: &str) -> String;
pub fn row(style: Style, (glyph, tone): (&str, Tone), name: &str, width: usize, detail: &str) -> String;
pub fn label_row(style: Style, label: &str, width: usize, value: &str) -> String;
pub fn outro(style: Style, text: &str) -> String;
pub fn truncate(text: &str, cols: usize) -> String;
// crates/plugins host::adapter
pub(crate) fn list_available(self) -> Option<Vec<String>>; // None for Claude
// toolu_runtime::cli::Ctx
pub style: Style,
// crates/plugins
pub(crate) struct Progress; // open(style, quiet) -> Option<Self> (`--json` already forces Plain); to_writer(Box<dyn Write + Send>, Style) -> Self
pub(crate) fn frame(state: &State, tick: usize, style: Style) -> String;
pub(crate) trait KeyInput { fn byte(&mut self) -> io::Result<Option<u8>>; fn byte_soon(&mut self) -> io::Result<Option<u8>>; }
pub(crate) enum Key { Up, Down, Toggle, All, Enter, Cancel, Other }
pub(crate) fn read_key(input: &mut impl KeyInput) -> io::Result<Key>;
pub(crate) struct Checkbox; // new(tty: File, style: Style) -> Self; impl Ask
pub(crate) fn detected(env: &Env) -> Vec<Target>;
```

`toolu plugins update --json` without `--host` prints `{ "dryRun": bool, "hosts": [{
"host", "steps": [Step...] }] }`, the `pluginsInstall` shape. With `--host` it prints `{
"host", "steps" }` as today. In `crates/cli/src/commands.schema.json` (source of the
generated docs copy), `pluginsHost.host` widens to `claude|codex|muse|cursor`, and the
`pluginsInstall` and `pluginsHost` descriptions name `update`.

Claude result lines, captured from 2.1.311 into `fixtures/plugins/claude.json` (format of
`fixtures/plugins/muse.json`: `version`, `claude`, `captured`, `note`, `cases[{name, argv,
exit, stdout, stderr}]`):

- `list`: real `plugin list --json` rows, trimmed to toolu, foreign, duplicate-pin and
  retired cases, with home paths rewritten to `/home/user` as `muse.json` does.
- `update-up-to-date`: `{"command":"update","outcome":"ok",...,"updateOutcome":
  "up_to_date","oldVersion":"8.5.0","newVersion":"8.5.0"}`.
- `update-not-found`: `{"outcome":"failed","message":"Plugin \"cloudflare\" not found",
  "failureCode":"not_found"}`, exit 1.
- `update-updated`, `update-skipped` and `update-refresh-failed`: marked `"derived":
  true`. Each is built from the captured line plus the field names in the 2.1.311 binary
  (`updateOutcome` `updated`/`skipped`, `skipReason`, `refreshFailed`), because a real
  update cannot be forced here.

`support_test.rs` gains `Sandbox::replay(name, cases)`: a stub that prints each case's
stdout and stderr and exits with its code when argv matches. It also gains
`claude_case(name)` next to `muse_output`.

## Failure modes and edge cases

- No PATH host and no Cursor plugins, update without `--host`: exit 64, today's "no host
  found on PATH (probed claude, codex, muse). Choose one with --host."
- Hosts found, none with toolu plugins: exit 1, "no detected host has toolu plugins
  installed, so there is nothing to update".
- One host errors or a step fails: shown in its section; others continue; exit 1.
- Raw mode cannot start (`tcgetattr` fails, stdin not a terminal): numbered prompt.
- Picker cancel by `q`, Esc, Ctrl-C or EOF: exit 1 "cancelled" (today's text). Termios is
  restored on return, cancel, error and panic (Drop).
- Unknown escape sequence: ignored (`Other`), not a key.
- Progress write error: dropped. `Drop` stops the thread if `end()` never ran.
- Ctrl-C during progress (not raw): `GroupGuard` kills the host command and `toolu`; the
  two lines stay on screen and the cursor is visible.
- Empty `NO_COLOR`: unset (`Env::get`).
- Claude without `plugin update --json` (older than the verified 2.1.311): each update is
  `failed` with Claude's message. installer.md states the requirement.
- Duplicate `toolu@toolu` rows: one step per name, user scope first.
- Claude listing rows from other marketplaces under a catalog name (`jev@other`): ignored
  by `list`, `install`, `update`, `remove`.

## Acceptance criteria

- **AC-1:** Replaying `fixtures/plugins/claude.json` `list` (toolu `ast-grep`/`jev`, two
  `toolu@toolu` rows, `cloudflare@cloudflare`, `jev@other`, retired `context7@toolu`),
  `update --host claude` logs `plugin update toolu@toolu --json`, `ast-grep@toolu --json`
  and `jev@toolu --json` once each. It logs nothing for `cloudflare`. `context7` is `skew`
  with `toolu plugins remove context7 --host claude --yes`.
- **AC-1b:** On Muse, a retired installed plugin gets the hint `muse plugins remove <name>`.
  On Cursor, a toolu-marked directory whose name the catalog lacks gets `skew` with
  `toolu plugins remove <name> --host cursor --yes` instead of a copy failure.
- **AC-2:** Replaying `update-up-to-date` gives `current` "current at 8.5.0".
  `update-updated` gives `updated` "updated 8.5.0 → 8.6.0", `update-skipped` gives `skew`
  with its message, `update-refresh-failed` gives `skew` "could not refresh the marketplace;
  still at 8.5.0", `update-not-found` gives `failed` `Plugin "cloudflare" not found`, and
  exit 0 with empty stdout gives `updated`.
- **AC-3:** `list --host claude` over a listing holding only `jev@other` reports `jev` not
  installed.
- **AC-4:** With stub `claude` and `codex` holding toolu plugins, `muse` holding none, and
  a scripted `Ask` that fails the test if called, `update` without `--host`:
  - logs no prepare or listing beyond `plugins list --json` for Muse;
  - updates claude and codex;
  - its plain text contains `muse: no toolu plugins installed; skipped`;
  - its `--json` is `{dryRun:false, hosts:[claude, codex]}`.
  With `--host claude`, `--json` is `{host, steps}`.
- **AC-4b:** `update jev` without `--host`, with `jev` only on claude and codex holding
  other toolu plugins, runs nothing on codex, and its text has `codex: jev not installed;
  skipped`. `update ast-grep` where the name is in the catalog but on no host exits 1 with
  the not-installed message.
- **AC-4c:** A codex host whose prepare fails (stub fails `plugin marketplace add`) yields
  a `failed` step named `codex`, claude's steps still run, and the exit is 1.
- **AC-5:** `update` without `--host`, where every listing is empty, exits 1 with the
  nothing-to-update message.
- **AC-6:** `style_of` maps not-terminal to `Plain`, `TERM=dumb` to `Plain`,
  `NO_COLOR=1` to `Mono`, and `NO_COLOR=` to `Color`. `paint` adds `\x1b[` only for
  `Color`. The built `toolu` binary with piped stdout (`assert_cmd`) prints no `\x1b` for
  `plugins list` (a sandbox stub host) or `status`.
- **AC-7:** The rails install report, over fixed sections and `Duration` 1.25 s, equals
  the expected text exactly:
  - claude rows: installed, already, skew, failed;
  - a dry-run section: `○ … would run: claude plugin install x@toolu`;
  - the footer `└  1 installed · 1 already · 1 skew · 1 failed  (1.2s)`.
  The `Mono` render has no `\x1b`. The rails list shows the three list glyphs.
- **AC-8:** `Progress::to_writer` over a shared buffer: after `begin("claude", 3)`,
  `task("installing jev")` and `advance()`, the bytes hold a frame with the bar, `1/3` and
  `installing jev…`. After `end()` the buffer ends with the clear sequence and grows no more
  within 200 ms. `open` returns `None` for `Plain` (which `--json` forces) and for quiet.
- **AC-9:** On a real pty (`nix::pty::openpty`; master writes keys, `Checkbox::new` gets
  the slave), over [claude, codex, muse] all preselected:
  - `\x1b[B`, space, `\r` returns [0, 2];
  - `a`, `\r` shows "choose at least one", then `a`, `\r` returns [0, 1, 2];
  - `q`, `\x03` and a lone `\x1b` cancel;
  - a 200-character hint renders cut to 76 columns;
  - `tcgetattr` after each `many()`/`one()` call, on success and on cancel, equals the
    attributes before the call;
  - constructing the checkbox leaves the attributes unchanged.
- **AC-10:** `read_key` over byte input maps `\x1b[A`, `\x1b[B`, `k`, `j`, space, `a`, `\r`,
  `\n`, `q`, `\x03`, lone `\x1b` (no follow-up byte), EOF and `\x1b[Z` (Other).
- **AC-11:** With stub `claude` and `codex` and `home/.cursor`:
  - the scripted `Ask` for the install host choice receives claude, codex, cursor with
    their labels;
  - without an `Ask`, install exits 64 naming only claude and codex;
  - with only `claude` on PATH plus `home/.cursor`, install without an `Ask` uses claude;
  - `list` with claude on PATH and an empty `home/.cursor` uses claude without asking;
  - with a toolu-marked plugin in `home/.cursor/plugins/local`, `list`'s `Ask` receives
    claude and cursor.
- **AC-12:** Status `Color`/`Mono` over a real temp git repository (one commit, one
  untracked file, no state) equals the expected table lines exactly. Outside a repository
  it shows `not a git repository` and `—`. `Plain` equals today's pretty JSON. Status
  documents built from real gate, push-review and waiver files render the rows below.
  The failing gate, recorded review and active waiver are written as in
  `status/tests/gate_test.rs`. The malformed gate, invalid review, pending waiver and
  waiver-plus-pending files are new fixtures written the same way.
  - `✘ failing: <reason> (2 files)`;
  - `▲ invalid: <reason>` for a malformed gate file;
  - `✔ recorded`, and `▲ invalid: not JSON` for a review file that is not JSON;
  - `waiver active`, `waiver pending` and both.
- **AC-13:** `docs/cli/installer.md` has the sections "Choosing hosts", "Updating every
  host" and "Terminal output", and names `NO_COLOR`, the picker keys, the update `--json`
  shapes with the compat note, and the Claude version requirement.
  - `crates/plugins` `docs_test` asserts those strings.
  - After `cargo xtask docs-cli`, `git diff --exit-code docs/cli` shows nothing.
  - `cargo xtask check-markdown-cli` passes.
- **AC-14:** `cargo xtask gate` passes on the branch.

## Acceptance evidence

| AC | Real input / fixture | Expected observable | Boundary / failure | Check |
|----|----------------------|---------------------|--------------------|-------|
| 1, 1b | `fixtures/plugins/claude.json` `list` replayed by `Sandbox::replay`; Muse fixture listing plus a retired name | stub log; step detail | duplicates, foreign, retired | `cargo test -p toolu-plugins update` |
| 2 | `claude.json` update cases | status and detail | refresh failed, skipped, failed, empty | `cargo test -p toolu-plugins update` |
| 3 | stub listing `jev@other` | `installed: false` | foreign same name | `cargo test -p toolu-plugins list` |
| 4, 4b, 4c, 5 | sandbox stubs, panicking scripted `Ask` | logs, text, JSON | names, prepare failure, all empty | `cargo test -p toolu-plugins dispatch` |
| 6 | `Env::from_pairs`; built binary via `assert_cmd` | `Style` values; no `\x1b` | empty `NO_COLOR`, dumb | `cargo test -p toolu-runtime term`; `cargo test -p toolu-cli --test plain_output` |
| 7 | fixed `Step` sections | exact text | dry run, Mono | `cargo test -p toolu-plugins rails` |
| 8 | shared buffer | frame bytes, clear, quiet after end | `None` cases | `cargo test -p toolu-plugins progress` |
| 9 | `openpty` pair | chosen indices, restored termios | cancel, empty, long hint | `cargo test -p toolu-plugins checkbox` |
| 10 | byte slices | `Key` values | unknown CSI, EOF | `cargo test -p toolu-plugins keys` |
| 11 | sandbox `bin/` + `home/.cursor` | `Ask` options; exit 64 text | single PATH host | `cargo test -p toolu-plugins host` |
| 12 | `git init` temp repo; temp non-repo dir | exact lines; JSON equality | not a repo, Plain | `cargo test -p toolu-hub status` |
| 13 | `docs/cli/installer.md` | strings present; no regen diff | — | `cargo test -p toolu-plugins docs`; `cargo xtask docs-cli && git diff --exit-code docs/cli`; `cargo xtask check-markdown-cli` |
| 14 | branch | exit 0 | — | `cargo xtask gate` |

## Documentation impact

- `docs/cli/installer.md`:
  - new sections "Choosing hosts" (detection, Cursor, picker keys), "Updating every
    host" (prompt-free, names, `--scope`, JSON shapes, compat note) and "Terminal
    output" (rails, progress on `/dev/tty`, `NO_COLOR`, `TERM=dumb`, piped output
    unchanged);
  - the Claude 2.1.311 `update --json` requirement.
- `crates/cli/src/commands.schema.json` and the regenerated `docs/cli/commands.schema.json`.
- `docs/cli/plugins.md` and `docs/cli/status.md`: regenerated if `--help` text changes.
- `README.md` install section: one line saying `toolu plugins update` covers every host.
- CHANGELOG comes from release-please; no manual edit.

## Open Questions

- None. The release question is decided (`feat` plus compat note, user, 2026-10-27).
