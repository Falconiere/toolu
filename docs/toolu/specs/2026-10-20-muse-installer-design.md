# toolu plugins targets Muse (#549) — Design

**Date:** 2026-10-20   **Status:** Approved   **Author:** Claude (epic worker)   **Topic:** `toolu plugins --host muse`: a Muse adapter with a local-dir marketplace and opt-in approval

## Problem

`toolu plugins` is the only supported path onto a host, and it drives Claude Code
and Codex only: `--host muse` exits 64 ("not an installer target"), and with only
`muse` on `PATH` the installer finds no host. Muse's CLI differs in every verb, has
no `--scope`, needs a marketplace directory rather than a GitHub source, and keeps
installed hooks inert until `muse plugins approve` trusts them. The #541 spike
settled the distribution design (D-localdir, `docs/research/muse-plugin-distribution.md`);
this issue implements it.

## Non-Goals

1. No new `--available` flag on `toolu plugins list`. The issue's "`list --available`"
   is the Muse adapter's internal available listing (the one `install` and
   `update` already read for every host); `toolu plugins list` reports
   name/version/enabled from `muse plugins list --json`. (Jev `available` → internal, 0.74.)
2. No change to Claude Code or Codex behaviour, argv, or reports.
3. No live-Muse test in CI: Muse is not on CI runners. The issue's "verified
   against the real `muse` CLI in tests" is met by stub `muse` executables that
   replay outputs captured from Muse 1.4.11 plus a recorded live run (AC-9); #552 (host-smoke) and #553 (live end-to-end) own the
   standing live checks.
4. No npm wrapper change (`tools/toolu-cli`): it forwards argv unchanged.
5. No `.muse-plugin` manifests (#547) or Muse docs sweep (#551) beyond the
   installer guide and the crate README.

## Architecture

Muse joins behind the existing host-free shape:

- `host::Target::Muse`, probed **last** (`[claude, codex, muse]`), so the prompts
  and errors Claude/Codex users see keep their order. `target_of(Host::Muse)`
  returns it; `check_scope` rejects any `--scope` for Muse naming `muse`.
- `host::adapter::Adapter::Muse` builds the argv of the spike and normalizes
  `muse plugins list [--available] --json` in a new `host::muse` module.
- A `Driver::prepare` step runs before `install` and `update` touch plugins.
  Claude/Codex: install keeps today's non-fatal `plugin marketplace add`
  hint; update does nothing. Muse: make the toolu checkout current, then
  register or refresh it as the marketplace `toolu`; every failure here is
  fatal and comes before any plugin command.
- A trust pass after `install` and `update` (`verbs::trust`): Muse only, through
  `muse plugins inspect` and, under `--approve`, `muse plugins approve`. Hosts
  without an approval step skip it, so the verbs never branch on the host.
- Foreign-source plugins: a Muse plugin whose install record's source lies
  outside the checkout is a non-failing skew, left untouched, and excluded from
  a default `update` (Jev `foreign` → skew, 1.0).

Reused: `host::run::run` (process group, deadline) for `git` and `muse`;
`step::Status`; `report`; `replay`. The git origin is a `Context` field
(`https://github.com/Falconiere/toolu.git` in production) so tests use a
`file://` origin with no network.

The decisive trade-off is the opt-in `--approve`: approving automatically would
bypass Muse's explicit-trust step, and never approving would leave toolu silently
not enforcing. Without the flag the run still exits 0 and prints the exact
approve argv per pending plugin (spike decision).

## Interfaces / Schema

**CLI.** `--approve` on `install` and `update`: "Approve each plugin's hooks after
installing or updating (Muse only)". A no-op on Claude Code and Codex. Every
other flag is unchanged; `--host muse` now resolves; `--scope … --host muse`
exits 64 with `--scope is for Claude Code; muse has no scope concept`.
`.toolu/plugins.json` accepts `"host": "muse"` (format version stays 1).

**Checkout.** `DIR = $XDG_DATA_HOME/toolu/muse-marketplace` when
`XDG_DATA_HOME` is an absolute path (a relative one is ignored, per the XDG
spec), else `$HOME/.local/share/toolu/muse-marketplace`; `TAG = v<binary
version>`. Missing parent directories are created before the clone. A plugin is
*foreign* when its install record's `source.path` starts with neither `DIR` nor
`DIR`'s canonical path.

**Muse argv** (all with `--json`):

| Purpose | argv |
|---|---|
| clone (DIR absent) | `git clone --depth 1 --branch TAG ORIGIN DIR` |
| tag present? | `git -C DIR rev-parse -q --verify refs/tags/TAG` |
| fetch (tag absent) | `git -C DIR fetch --depth 1 origin tag TAG` |
| move | `git -C DIR checkout -q --detach TAG` |
| origin check | `git -C DIR remote get-url origin` |
| marketplaces | `muse plugins marketplace list --json` |
| register | `muse plugins marketplace add toolu DIR --json` |
| refresh | `muse plugins marketplace update toolu --json` |
| install | `muse plugins install NAME@toolu --json` |
| installed | `muse plugins list --json` |
| available | `muse plugins list --available --json` |
| trust state | `muse plugins inspect NAME --json` |
| approve | `muse plugins approve NAME --json` |
| update | `muse plugins update NAME --json` |
| remove | `muse plugins remove NAME --json` |

**Parsed shapes** (captured from Muse 1.4.11, `fixtures/plugins/muse.json`):

- installed: `{"plugins": [{"record": {"id", "version", "enabled", "source": {"path"}}}]}`
  → `Installed { name: record.id, version, enabled, source: Some(path) }`.
  `record` is the lockfile entry, present even when `plugin` fails validation.
- available: `{"available": [{"name", "version", "marketplace"}]}` → entries whose
  `marketplace == "toolu"`, `enabled: true`.
- inspect: `{"runtime_capabilities": [{"status"}]}`; pending when any status is
  `review_needed` or `modified`.
- marketplaces: `{"marketplaces": [{"name", "source": {"kind", "path"}}]}`.

`Installed` gains `source: Option<String>` (`None` for Claude/Codex).

**Reports.** No new status or JSON key. The trust pass appends to a step's
`detail`: `; approved`, or `; awaiting approval: muse plugins approve NAME`. A
foreign plugin is `Status::Skew` with `installed from <path>, outside the toolu
marketplace; left untouched. Run \`muse plugins remove NAME\`, then \`toolu
plugins install NAME --host muse\`.`

## Failure modes and edge cases

| Case | Behaviour |
|---|---|
| Neither `XDG_DATA_HOME` nor `HOME` set | exit 1 naming both, before any command |
| Clone fails (offline, no tag `v<version>`, e.g. a development build) | exit 1 with git's first stderr line; no `muse` command ran |
| `DIR` exists without `.agents/plugins/marketplace.json`, or origin ≠ ORIGIN | exit 1 naming DIR; DIR untouched |
| Fetch or checkout fails | exit 1 with git's line; no `muse` command ran |
| `toolu` marketplace registered with another source | exit 1 naming that source and `muse plugins marketplace remove toolu`; nothing installed |
| `marketplace list` malformed or failing | exit 1 naming the payload problem |
| `muse plugins list --json` malformed (`toolu plugins list`) | exit 1 naming the shape problem, never an empty list |
| Installed/available listing malformed during install/update | as today for every host: treated as empty, so each plugin is attempted |
| Plugin already installed from the checkout | `already`/`skew` as today; the trust pass still runs, so re-running `install --approve` approves a pending plugin and an approved one stays approved without a new prompt |
| Plugin installed from another source | non-failing skew with remove + reinstall commands; no trust pass; skipped by a default `update` |
| `inspect` fails or is malformed after a good install | the step becomes `failed` ("installed, but …"), exit 1 — trust state unknown is not success |
| `approve` fails | the step becomes `failed` with Muse's line, exit 1 |
| Two runs at once on one `DIR` | git's lock refuses the second; exit 1 with git's line |
| Tag without Muse manifests (v8.0.4 and earlier) | Muse skips those plugins, so `install NAME@toolu` fails per plugin with Muse's `not available in marketplace` line; exit 1. Fixed by the first release carrying #547 |
| `--dry-run` | no git, no marketplace change, no trust pass; plugin argv only, as for other hosts |
| `--approve` on Claude/Codex | accepted, does nothing |
| `remove` of an absent plugin | Muse exits 1 (`plugin \`x\` is not installed`); a failed step, exit 1, as on Claude |
| Several hosts on `PATH`, no `--host`, no terminal | exit 64 naming `claude, codex, muse` (unchanged rule) |

## Acceptance criteria

- **AC-1:** With only a `muse` executable on `PATH`, `toolu plugins list` and `install` target Muse without asking; with `claude`, `codex` and `muse` on `PATH`, `--host muse` selects Muse alone, a terminal prompt offers all three, and no terminal exits 64 naming them.
- **AC-2:** `toolu plugins list --host muse` reports name, version and enabled parsed from the captured Muse 1.4.11 `muse plugins list --json` (a disabled plugin reads disabled); the Muse adapter parses the captured `list --available --json` into the toolu marketplace's entries only.
- **AC-3:** A malformed `muse plugins list --json` payload (wrong top-level shape, an entry without `record.id`) makes `toolu plugins list --host muse` exit 1 naming the problem, never print an empty list.
- **AC-4:** `toolu plugins install NAME --host muse` on a clean home clones `v<version>` into the checkout, registers it as marketplace `toolu`, installs `NAME@toolu` in dependency order; with an existing checkout it moves the checkout and refreshes the marketplace instead; a foreign `toolu` marketplace or a non-toolu `DIR` fails before any install.
- **AC-5:** After install, a plugin whose `inspect` reports `review_needed` or `modified` is approved under `--approve` (detail `approved`) or reported with `muse plugins approve NAME` without it (exit 0); a `trusted_enabled` plugin is left alone. Re-running `install --approve` over an approved plugin exits 0 without another approve call.
- **AC-6:** `--scope user --host muse` exits 64 with a message naming Muse.
- **AC-7:** `toolu plugins update --host muse` moves the checkout, refreshes the marketplace, updates each installed toolu plugin behind the offered version, runs the trust pass, and skips plugins installed from another source; `remove NAME --host muse --yes` runs `muse plugins remove NAME --json`.
- **AC-8:** `install --config` with `"host": "muse"` replays on Muse.
- **AC-9:** Against the real Muse CLI (1.4.11, isolated home), `toolu plugins install toolu --host muse --approve` leaves `muse plugins inspect toolu` with every runtime capability `trusted_enabled`, a second run exits 0 without re-approval, and `remove` makes `muse plugins list` no longer show it.
- **AC-10:** `cargo xtask gate` passes, with `docs/cli/` regenerated and the commands snapshot accepted.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | stub `muse`/`claude`/`codex` executables in a sandbox `PATH` | `host::resolve` → `[Muse]`; all three → prompt order / 64 | no terminal, `--no-input` | `cargo test -p toolu-plugins host`, `cargo test -p toolu-cli --test plugins_muse` |
| AC-2 | `fixtures/plugins/muse.json` cases `list`, `available` (captured Muse 1.4.11 output) | rows `toolu 8.0.4`, `hooky … (disabled)`; available keeps `marketplace == "toolu"` | other-marketplace entries dropped | `cargo test -p toolu-plugins muse`; CLI test via stub serving the capture |
| AC-3 | captured `list` mutated to `{"plugins":{}}` and an entry without `record` | `Err` naming the shape; CLI exit 1 | empty `{"plugins":[]}` is a valid empty list | `cargo test -p toolu-plugins muse`, `--test plugins_muse` |
| AC-4 | real `git` with a `file://` origin repo tagged `v<version>` + stub `muse` serving captures | argv log: clone → marketplace list → add → install; second run: checkout → update | foreign marketplace capture, `DIR` without catalog | `cargo test -p toolu-plugins prepare` |
| AC-5 | captures `inspect-review-needed`, `inspect-trusted`, `inspect-modified` | approve argv logged only for pending; detail text | `inspect` failure → failed step | `cargo test -p toolu-plugins trust` |
| AC-6 | `--scope user --host muse` on the real binary | exit 64, stderr names muse | — | `cargo test -p toolu-plugins host`, `--test plugins_muse` |
| AC-7 | stub `muse` + real git checkout | update/remove argv logged; foreign record skipped as skew | named foreign plugin → skew | `cargo test -p toolu-plugins update`, `remove` |
| AC-8 | a `.toolu/plugins.json` with `"host": "muse"` | `Selection.host == Some(Muse)` and install runs on Muse | `"host": "cursor"` still 64 | `cargo test -p toolu-plugins replay` |
| AC-9 | real `muse` 1.4.11 with `XDG_*` in a scratch home, checkout pre-seeded from this branch with local tag `v8.0.4` | inspect statuses all `trusted_enabled`; second run 0; list empty after remove | re-run idempotent | transcript in `docs/toolu/evidence/2026-10-20-muse-installer.md` |
| AC-10 | the repository | gate green | — | `toolu epic job -- cargo xtask gate` |

## Documentation impact

- `docs/cli/installer.md`: Muse as a host, `--approve`, the checkout and its failure modes, `"host": "muse"`.
- `docs/cli/` regenerated (`cargo xtask docs-cli`) for `--approve` and the new help text.
- `crates/plugins/README.md` and the `AGENTS.md` key-files row for `crates/plugins/src/lib.rs`.
- `fixtures/README.md`: the new `plugins/muse.json` capture file.

## Open Questions

None blocking. Recorded decisions: probe order last; no `list --available` flag;
flag name `--approve`; foreign sources are skew; `record` (not `plugin`) is the
installed identity.

## Spec review (2026-10-20)

Jev: coverage 0.63 → "every issue item covered or explicitly deviated"; live-test deviation justified 0.93.

- Non-Goals: 🟡 should-fix: the issue's "verified against the real muse CLI in tests" was not tied to the deviation. Fixed: Non-Goal 3 names it.
- Interfaces: 🟡 should-fix: "foreign" and a relative `XDG_DATA_HOME` were undefined. Fixed in **Checkout**.
- Failure modes: 🟡 should-fix: concurrent runs and released tags without Muse manifests were missing. Fixed: two rows added.

**Status:** Approved
