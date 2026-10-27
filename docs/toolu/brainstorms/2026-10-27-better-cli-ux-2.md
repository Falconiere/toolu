# Brainstorm: better CLI UI/UX, round 2 (2026-10-27)

Mode: standalone. Path: Full (public interface: prompts, the `update` JSON document, and
`status` output). Supersedes the scope of `2026-10-27-better-cli-ux.md` for `install`,
`update` and `status`. Its other findings (list legend and width, dry-run marks, bare
namespace, hidden `hook`, doctor summary) remain candidates.

## Capsule

- **Outcome:**
  1. On a terminal, `toolu plugins install` probes every supported host and shows the ones
     it finds as a checkbox list: arrows or j/k move, space toggles, `a` toggles all,
     Enter confirms, q/Esc/Ctrl-C cancels, all preselected. Hosts are Claude Code, Codex
     and Muse, plus Cursor through `cursor-agent`. The same widget replaces the numbered
     plugin picker.
  2. `toolu plugins update` never asks. It acts on every detected host that reports at
     least one toolu plugin and prints a section per host. It updates only toolu's own
     plugins that the binary's catalog still lists. Already-current plugins say so.
  3. `toolu status` prints a table in human mode. `--json` is unchanged.
- **Material defaults/non-goal:**
  - No new crate. The checkbox list uses termios from the existing `nix` dependency
    (`term` feature) and falls back to today's numbered prompt when raw mode fails.
  - Without a terminal, or with `--no-input`, `install` still exits `64` when several
    hosts are found. Never resolving silently stays the rule for install.
  - Out of scope: refreshing per-project `toolu@toolu` pins (they are only deduplicated),
    color, and spinners.
- **Repository evidence:** `crates/plugins/src/host/claude.rs` `parse()` drops the
  `@<marketplace>` and keeps every marketplace's plugins. `codex.rs` already keeps only
  `marketplaceName == "toolu"`. `update` with no names targets every installed entry, so
  `cloudflare@cloudflare` becomes `claude plugin update cloudflare@toolu` and fails "not
  found". `claude plugin list --available --json` returns `{available, installed}`, not an
  array, so the offered list is always empty. Every plugin then runs and prints "updated"
  with no version. Host resolve: `crates/plugins/src/host.rs` `resolve()`
  (Mode::Many/One, Cursor never probed). Prompt: `crates/plugins/src/prompt.rs`.
- **Risk:**
  - Probing `cursor-agent` turns a non-TTY install on a machine with Claude plus
    cursor-agent from silent into exit `64`.
  - `update` without `--host` changes its JSON shape.
  - Raw mode must restore the terminal on every exit path, including Ctrl-C (ISIG is off
    in raw mode, so 0x03 is read as cancel) and a panic (Drop guard).
  - `rules.json` may restrict which crates link `nix`; check before choosing the feature
    route.
  - `crates/cli/tests/golden_statusline.rs` pins the `toolu status` JSON snapshot.
  - How to read the offered version on Claude is not settled (see Open).
- **Handoff:** `/delivery-flow:delivery-flow` on a fresh branch from `main`. Land the
  update fix first as `fix(plugins)`, then `feat(plugins)` for the picker and multi-host
  update, then `feat(cli)` for the status table.

## Decisions

| Axis | Default | Jev | Rejected |
|------|---------|-----|----------|
| Picker | Hand-written checkbox over `nix` termios, numbered fallback | `raw_checkbox` 0.98 | Keep numbered prompt (0.02): does not read as a multiselector. New crate such as inquire, dialoguer or cliclack (0.00): cargo-deny surface, against the no-new-dependency preference |
| `update` JSON | `--host` keeps `{ "host", "steps" }`; without it `{ "dryRun", "hosts": [{ "host", "steps" }] }`, same as `install` | `hosts_without_flag` 0.88 | Always hosts (0.02): breaks `--host` scripts. Shape by host count (0.10): shape depends on the machine |
| `update` targets | Installed, marketplace `toolu`, and in the catalog; deduplicated by name; retired plugins get a skipped step naming `toolu plugins remove` | `marketplace_and_catalog` 0.95 | Marketplace only (0.04): retired `context7`/`exa-search`/`jira` would fail. Catalog only (0.01): a same-named plugin from another marketplace would be hit |
| Where the filter lives | `claude::parse` keeps only `@toolu` ids, matching `codex::parse`; the catalog intersection lives in `verbs/update.rs` | n/a (code evidence) | Filter only in `update`: `list`/`remove` would keep the same latent bug |
| Hosts for `update` | Every probed host on `PATH` (plus Cursor) whose listing has toolu plugins. A host with none: one line, not a failure | n/a | Prompt for one host: the user asked for no prompt |
| `status` | Human mode: a two-column table (branch, ahead/behind, host, gate, push review, waivers, working tree). `--json`: today's document | n/a | Keep JSON in human mode: the user asked for a table |

## Open (for the spec)

- Claude's offered version: does `.installed[]` in `claude plugin list --available --json`
  carry the marketplace version? If not, compare against the toolu marketplace's
  `plugin.json` in Claude's marketplace clone, or report Claude's own `plugin update`
  message. Probe the real output first; don't guess.
- `claude plugin update` with project-scope pins present: which scope does it change?
  This decides whether "updated" is true for the active project.
