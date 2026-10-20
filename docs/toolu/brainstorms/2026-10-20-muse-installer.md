# Brainstorm — `toolu plugins` targets Muse (#549)

## Capsule

- **Outcome:** `toolu plugins <install|list|remove|update> --host muse` manages
  toolu plugins through the `muse` CLI, including the approve-to-activate trust
  step, with the D-localdir design the #541 spike settled
  (`docs/research/muse-plugin-distribution.md`).
- **Material defaults / non-goals:** `muse` is probed after `claude` and `codex`;
  approval is opt-in through a new `--approve` flag on `install` and `update`;
  no new `--available` flag on `list`; no live-Muse test in CI (#552/#553 own
  host-smoke and the live run); no npm wrapper change.
- **Repository evidence:** `crates/plugins` already hides each host behind
  `host::adapter::Adapter` (argv builders + `parse_list`), and the verbs are
  host-free. The spike fixed the argv and the checkout location. Live probes of
  Muse 1.4.11 (2026-10-20, isolated `XDG_*` home) reproduced the spike: hooks and
  agent definitions are `review_needed` after install, `trusted_enabled` after
  `approve`, `modified` after a changed definition; a repeated `install` of the
  same source exits 0; `marketplace add` of an existing name exits 1.
- **Risk:** the checkout clones `v<binary version>` from GitHub, so a development
  build with no matching tag cannot install on Muse (fails before any `muse`
  call). Tests cannot reach GitHub, so the clone origin is a `Context` field.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Probe order | `[claude, codex, muse]` | Epic: Claude/Codex behaviour unchanged at every merge; Jev `probe` → `last` (1.0) | A machine with all three lists muse third |
| `list --available` | No new flag; the Muse adapter's internal available listing parses `.available[]` | The flag does not exist for any host today; Jev `available` → `internal` (0.74) | Reviewer may read the issue literally |
| Approval | `--approve` on `install`/`update`; without it, exit 0 and name `muse plugins approve <name>` per pending plugin | Spike decision ("#549 picks the flag name") | A user who skips the flag runs with inert hooks, but is told so per plugin |
| Foreign source | Non-failing skew with remove + reinstall commands; excluded from default `update` | Spike edge case; Jev `foreign` → `skew` (1.0) | None beyond the report |
| Installed identity | `record.{id,version,enabled}` of `muse plugins list --json` | `record` is the lockfile entry, present even when `plugin` fails validation | — |

## Alternatives rejected

- **muse first in probe order:** changes the prompt order Claude/Codex users see.
- **Automatic approval:** bypasses Muse's explicit-trust step (spike).
- **A Muse-only verb module set:** duplicates install/update; the adapter plus a
  `Driver` preparation hook keeps verbs host-free.
- **Adding `--available` to `toolu plugins list`:** a new public surface on every
  host for no stated need beyond the issue's wording.
