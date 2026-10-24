# Codex hook trust that sticks for epic workers (#626) — Design

**Date:** 2026-10-24   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** pin toolu's Codex marketplace to the release tag; report a hook-review screen as needs-human

## Problem

A Codex epic worker launched into a toolu worktree stops at Codex's hook review
(`26 hooks need review before they can run.`). Nothing is written to Codex's
session store, so the launch times out and the engine raises
`failed … cannot attest codex session identity`. Trusting does not stick: the
next session asks again. Both #605 and #615 had to be moved to Claude by hand.

The issue guessed a trust-key collision with the repository's project
marketplace. The brainstorm
(`docs/toolu/brainstorms/2026-10-24-codex-hook-trust-pin.md`) verified a
different cause on Codex 0.164.0:

- toolu's installer registers the Codex marketplace with
  `codex plugin marketplace add Falconiere/toolu` and no ref, so it follows
  toolu's default branch (`ref_name: null`, revision = main HEAD).
- At every session start Codex runs `plugins-marketplace-auto-upgrade`: a git
  marketplace whose remote revision moved is re-cloned and its installed
  plugins are force-reinstalled (`core-plugins/src/manager.rs`, tag
  `rust-v0.164.0`).
- Plugin hook trust pins `trusted_content_hash`, a hash of the plugin's content
  (`hooks/src/engine/plugin_trust.rs`). Any merge that touches a plugin
  directory re-opens the review for that plugin's hooks. #628 touched only
  epic-orchestrator; the next session asked for exactly its 3 hooks.
- `codex plugin list` in a toolu worktree lists only the installed marketplace;
  the repository catalog is not loaded, so no rename is needed.

## Non-Goals

1. Renaming `.agents/plugins/marketplace.json` or its `name` (not the cause, and
   the same catalog names the installed marketplace for Codex, Muse and Cursor).
2. Passing `--dangerously-bypass-hook-trust` to worker launches.
3. Pinning Claude Code's marketplace (Claude has no hook trust step).
4. Replicating Codex's hook hash to enumerate untrusted hooks.
5. Changing the record stage machine: a launch that started an agent but could
   not attest its session keeps stage `uncertain`, so `--reprompt`/`--replace`
   recovery is unchanged.

## Architecture

Three parts, each in the crate that owns it.

**1. Installer pins Codex (`crates/plugins`).** Muse and Cursor already follow a
checkout at `v<binary version>`; Codex now does the same through its own CLI:
`codex plugin marketplace add Falconiere/toolu --ref v<version>`. The
`Driver` gets `pin: Option<String>` (`Some("v<version>")` for Codex, `None`
otherwise) built in `dispatch::driver`. `prepare::run` makes the Codex
marketplace pinned on both `install` and `update`, and returns
`Prepared { hint, repinned }`.

Pinning an existing unpinned or differently pinned marketplace: Codex refuses
the add with `marketplace 'toolu' is already added from a different source`
(verified). The installer then reads `codex plugin marketplace list --json`;
only when `toolu` is a git source at `https://github.com/Falconiere/toolu.git`
does it run `codex plugin marketplace remove toolu` and the pinned add.
Verified on 0.164.0 in a private `CODEX_HOME`: remove + add keeps
`[plugins."<name>@toolu"]` and every `[hooks.state]` entry. A marketplace from
any other source is left untouched, with the hint naming it.

After a re-pin the installed toolu plugins are re-added (`codex plugin add
<name>@toolu`, the existing update argv) so their cache holds the tag's
content: `update` re-adds every installed toolu plugin even when versions are
equal; `install` re-adds an already installed plugin instead of reporting
`already`. Both report `updated` with detail `re-pinned to v<version>`.

With the tag pinned, the auto-upgrade sees no new revision, the plugin content
stops changing, and one trust lasts until the next toolu upgrade.

**2. Doctor reports an unpinned Codex marketplace (`crates/toolu/src/doctor`).**
On the Codex host the plugins check reads `$CODEX_HOME/config.toml` (else
`~/.codex/config.toml`) with the workspace `toml` crate. When
`[marketplaces.toolu]` is a git source at `Falconiere/toolu` and its `ref` is
not `v<binary version>`, the check warns with the fix. The epic preflight
already treats any `warn plugins` line as not ready
(`plugins/epic-orchestrator/skills/epic-orchestrator/references/preflight.md`
step 3), so the preflight catches it before a launch.

**3. Launch names an approval screen (`crates/epic-orchestrator`).** When an
attempt fails after reservation (start, prompt or session capture) and the
failure is neither `not_started` nor host-limited, `launch()` reads the agent's
pane once (`Call::Read`, the existing 15-line recent tail). A new
`launch/screen.rs` matches known approval screens; today Codex's hook review,
whose text is `1 hook needs review before it can run.` or
`<n> hooks need review before they can run.` (Codex `tui/src/bottom_pane/hooks_browser_view.rs`).
A match replaces the failure with code `approval_pending` and a message naming
the host, the screen line and the fix. The record gets
`lifecycle_outcome: "needs-human"`; the engine's launch effect returns
`{"outcome":"needs-human",…}` and `commit_launch` raises a `needs-human`
attention item instead of `failed`. A read error or no match keeps the original
failure.

## Interfaces / Schema

- `crates/plugins/src/host/adapter.rs`
  - `Adapter::add_marketplace(self, pin: Option<&str>) -> Vec<String>`;
    Codex with a pin: `codex plugin marketplace add Falconiere/toolu --ref <pin>`.
  - `Adapter::list_marketplaces(self) -> Vec<String>`: `codex plugin marketplace list --json`.
  - `Adapter::remove_marketplace(self) -> Vec<String>`: `codex plugin marketplace remove toolu`.
- `crates/plugins/src/host/codex.rs`: `marketplace_source(stdout, name) -> Result<Option<(String, String)>, String>`
  parsing `{"marketplaces":[{"name","marketplaceSource":{"sourceType","source"}}]}`.
- `crates/plugins/src/verbs/pin.rs` (new): `ensure(driver, pin) -> Res<Pin>` with
  `enum Pin { Pinned, Repinned, Unpinned(String /* hint */) }`.
- `Driver.pin: Option<String>`; `prepare::run -> Res<Prepared>`;
  `Prepared { hint: String, repinned: bool }`.
- Doctor notices (part of `warn plugins`):
  - no ref: `the Codex marketplace toolu follows its default branch, so Codex asks to review toolu's hooks again after every change to it; toolu plugins update --host codex`
  - other ref: `the Codex marketplace toolu is pinned to <ref>, not v<version>; toolu plugins update --host codex`
- `crates/epic-orchestrator/src/launch/screen.rs` (new):
  `pub(crate) fn approval_screen(kind: HostKind, tail: &str) -> Option<String>`
  returning the matched screen line.
- `failure.rs`: `pub(crate) const APPROVAL: &str = "approval_pending";`
  `Failure::awaiting_approval(&self) -> bool`; `details()` maps it to
  `lifecycle_outcome: "needs-human"`.
- Failure message:
  `codex is waiting on its hook review in pane <pane>: "26 hooks need review before they can run."; review and trust the hooks there, then relaunch with --replace`
- `effects_launch.rs`: `{"outcome":"needs-human","note":<message>,"stage":<stage>}`.
- `commit.rs`: `"needs-human"` → same bookkeeping as `failed`, attention kind `needs-human`.

## Decisions

- **Host stubs in unit tests.** `crates/plugins` tests drive a scripted host
  (`crate::support::Sandbox`), the crate's existing pattern, replaying Codex
  0.164.0's verbatim output captured in the brainstorm. Each behaviour they
  cover also runs live against real Codex 0.164.0 in a private `CODEX_HOME`
  (AC-1 to AC-3, AC-7); only the failure-after-remove path (AC-4) cannot be
  provoked live without breaking the network or the tag.
- **Record stage stays `uncertain`.** The agent exists in its pane and its
  session is not attested, which is exactly what `uncertain` guards:
  `--reprompt` and `--replace` keep working. What changes is everything the
  issue saw: the error no longer says `cannot attest`, `lifecycle_outcome` is
  `needs-human`, and the engine raises a `needs-human` attention item naming the
  screen instead of `failed`. A new stage would touch recovery, dispatch and
  the record schema for no behavioural gain.
- **Epic launches on the user's own Codex are post-merge.** Epic workers start
  through herdr panes that inherit herdr's environment, so a private
  `CODEX_HOME` cannot be injected into `toolu epic launch`. AC-7 runs the same
  Codex TUI start the launch performs, in herdr panes, with a private home. The
  two-launch check on the user's real `~/.codex` needs the released binary and
  `toolu plugins update --host codex`; it is the epic's "repro against the
  merged build" step (#591), owner: the epic orchestrator.

## Failure modes and edge cases

- **Tag missing or network down during re-pin:** the pinned add fails after the
  remove. The installer re-adds the unpinned source to restore the previous
  state and fails the verb with `cannot pin the Codex marketplace toolu to
  v<version>: <stderr>; restored it unpinned`. Nothing is left without a
  marketplace unless the restore also fails, which is reported too.
- **Fresh machine, no marketplace:** the pinned add creates it (`install`); on
  failure the hint is attached to plugin failures as today.
- **Already pinned to this tag:** Codex answers `already added` with exit 0; no
  remove, no re-add.
- **Foreign `toolu` marketplace** (a local path or a fork): left untouched; the
  hint names its source. Doctor stays silent for a foreign source.
- **`marketplace list --json` fails or is malformed:** the verb fails naming
  the tag and the listing error; no add, no remove (review round, Jev 0.87:
  an add could replace a local source the listing would have shown).
- **Pinned add of toolu's own marketplace fails for another reason than the
  different-source refusal** (network, auth): the verb fails naming the tag;
  no remove (Jev 0.92: a silent update would loop doctor's fix).
- **Dry run:** no marketplace command runs (unchanged).
- **Doctor:** config absent, unreadable, unparsable, or with no
  `[marketplaces.toolu]`: no pin notice (the inventory already reports missing
  plugins).
- **Launch read fails or the agent is not yet named:** original failure kept.
- **Provider limit text and a review screen together:** limit wins (existing
  `host_limited` order), so the engine still moves the issue to another host.
- **Non-Codex hosts:** `approval_screen` returns `None`; behaviour unchanged.

## Acceptance criteria

- **AC-1:** Given a Codex host whose `toolu` marketplace is the unpinned
  `Falconiere/toolu` git source with plugins installed and `[hooks.state]`
  entries, `toolu plugins update --host codex` leaves `[marketplaces.toolu]`
  with `ref = "v<version>"`, keeps every `[plugins.*@toolu]` and
  `[hooks.state]` entry, and re-adds each installed toolu plugin.
- **AC-2:** Given a Codex host already pinned to `v<version>`, `install` and
  `update` run no `marketplace remove`, and `install` on a fresh host adds the
  marketplace with `--ref v<version>`.
- **AC-3:** Given a `toolu` marketplace from a foreign source, neither verb
  removes it, and the report names the source.
- **AC-4:** Given a pinned add that fails after the remove, the unpinned source
  is added back and the verb fails naming the tag and the restore.
- **AC-5:** `toolu --host codex doctor` warns `plugins` with the default-branch
  notice for a `config.toml` whose `[marketplaces.toolu]` has no `ref`, the
  pinned-to notice for another ref, and no pin notice for `ref = "v<version>"`.
- **AC-6:** Given the pane text Codex shows on its hook review, a failed launch
  attempt becomes code `approval_pending`, its record has
  `lifecycle_outcome: "needs-human"`, the engine's launch effect returns outcome
  `needs-human`, and `commit_launch` raises a `needs-human` attention item whose
  note names the screen line; text without the screen keeps `uncertain` and
  `failed`.
- **AC-7:** With a private `CODEX_HOME` (auth linked, as `host-smoke` does)
  whose `toolu` marketplace is pinned by the new installer, after one trust two
  consecutive Codex TUI sessions in two fresh toolu worktrees start with no
  hook review; and a fresh private home with untrusted hooks shows the review
  screen whose captured pane text matches `approval_screen`.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Sandbox `codex` stub replaying Codex 0.164's exact add-refusal and list JSON; plus live private `CODEX_HOME` run of `target/debug/toolu plugins update --host codex` | argv log: add `--ref` → list → remove → add `--ref` → `plugin add` per plugin; live `config.toml` diff shows `ref` added, plugins and hooks.state intact | equal versions still re-added | `cargo test -p toolu-plugins pin`; live transcript in `docs/toolu/evidence/` |
| AC-2 | stub answering add with exit 0; live: a second `update` on the AC-1 private home, and `install` on an empty private home | no remove in argv log; live `config.toml` has `ref` after the fresh install | fresh host | `cargo test -p toolu-plugins pin`; live transcript |
| AC-3 | list JSON with `sourceType: local`; live: private home whose `toolu` is a local-path copy of the catalog | no remove; hint names source; live `config.toml` unchanged | — | `cargo test -p toolu-plugins pin`; live transcript |
| AC-4 | stub replaying Codex 0.164's verbatim refusal, then failing the second add | restore add without `--ref`; verb error text | restore failure also reported | `cargo test -p toolu-plugins pin` |
| AC-5 | `config.toml` fixtures copied from this machine's real `[marketplaces.toolu]` (no ref), with `ref = "v8.1.4"`, and with the binary's tag | warn texts above / ok | absent config, foreign source | `cargo test -p toolu-hub codex_pin` |
| AC-6 | pane text captured from a real Codex 0.164 hook review (AC-7) and the TUI's exact strings | approval code, `needs-human` outcome and attention | 1-hook singular; text without screen; limit text | `cargo test -p toolu-epic-orchestrator screen`, `commit`, `effects_launch` tests |
| AC-7 | live Codex 0.164.0 in herdr panes, private `CODEX_HOME`, two `git worktree add` worktrees | second and third sessions show no review | first session with untrusted hooks shows the review | manual run; transcript in `docs/toolu/evidence/2026-10-24-codex-hook-trust.md` |

## Documentation impact

- `docs/cli/installer.md`: Codex marketplace is pinned to `v<version>`; re-pin
  behaviour and its failure restore.
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/preflight.md`:
  name the Codex pin notice as a `warn plugins` example.
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md`:
  a `needs-human` launch naming an approval screen: trust it in the pane, then
  `--replace`.
- `plugins/epic-orchestrator/README.md` if it documents launch outcomes.

## Open Questions

- None blocking. Whether Claude Code should also pin its marketplace (plugins
  ahead of the binary) is a separate decision, owner: epic orchestrator; it
  does not change this design.

## Spec review (2026-10-24)

Round 1 (Needs changes), Jev `covers` 0.47, `evidence_real` 0.05, `stage_ok` 0.49:

- Acceptance evidence: 🔴 blocker: installer ACs proved only by scripted host stubs. Add live private-`CODEX_HOME` runs for AC-1 to AC-3. Fixed.
- Architecture: 🔴 blocker: keeping stage `uncertain` unexplained against "not left uncertain". Added the Decisions entry. Fixed.
- Acceptance criteria: 🟡 should-fix: issue's two-epic-launch acceptance not mapped. AC-7 plus the post-merge owner in Decisions. Fixed.

Round 2 (Approved), Jev `covers` 0.58, `stage_ok` 0.66, `evidence_real` 0.06. The
remaining doubt is AC-4, which only a scripted host can provoke; the script replays
Codex 0.164.0's verbatim output, the crate's established pattern. Non-blocking.
