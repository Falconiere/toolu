# Codex hook trust that sticks (#626) — brainstorm

**Date:** 2026-10-24 · **Mode:** Delivery, Full path (installer behaviour is a public interface)

## Capsule

- **Outcome:** a Codex epic worker reaches its first turn without a hook review once
  the user has trusted toolu's hooks; a worker that does sit on Codex's hook review
  is reported `needs-human` naming that screen; preflight (doctor) catches the state
  that keeps re-opening the review.
- **Material defaults / non-goals:** pin toolu's Codex marketplace to the binary's
  release tag. Do not rename the catalog, do not bypass hook trust, do not pin
  Claude Code (it has no hook trust step).
- **Repository evidence:** Muse and Cursor installers already pin a checkout to
  `v<binary version>` (`crates/plugins/src/host/checkout.rs`); Codex's adapter adds
  `Falconiere/toolu` with no ref (`crates/plugins/src/host/adapter.rs`).
- **Risk:** re-pinning an existing install must keep its plugins and hook trust
  (verified: `codex plugin marketplace remove` keeps `[plugins.*]` and
  `[hooks.state]`), and must not strand the user without a marketplace when the
  tag fetch fails.
- **Handoff:** spec.

## Cause, corrected

The issue guessed a trust-key collision between the repository's project
marketplace and the installed one. Evidence on this machine (Codex 0.164.0):

1. `codex plugin list` inside a toolu worktree lists only the installed
   marketplace root `~/.codex/.tmp/marketplaces/toolu`; Codex loads plugin hooks
   from its install cache, never from the cwd catalog.
2. `[marketplaces.toolu]` has no `ref`; its install metadata says
   `ref_name: null`, `revision: 53d24b80` (main after #627), synced 3 minutes after
   that merge.
3. Codex 0.164.0 `core-plugins/src/manager.rs` starts a
   `plugins-marketplace-auto-upgrade` thread at every session start: a git
   marketplace whose remote revision moved is re-cloned and its installed plugins
   are force-reinstalled. Plugin hook trust pins `trusted_content_hash` of the
   plugin's content (`hooks/src/engine/plugin_trust.rs`).
4. #628 changed only epic-orchestrator files; the next session asked again for
   exactly epic-orchestrator's 3 SessionStart hooks.

So every merge to toolu's main re-opens the review for the plugins it touched.
During an epic run main moves many times a day.

## Alternatives

| Option | Verdict |
|---|---|
| Pin the Codex marketplace to `v<binary version>` | **Chosen.** Content changes only on a toolu upgrade, which is one deliberate re-trust. Matches Muse/Cursor. Also stops plugins running ahead of the binary. |
| `--dangerously-bypass-hook-trust` on worker launches | Rejected: drops Codex's hook vetting for every repository a worker enters, and leaves interactive sessions broken. |
| Rename the catalog's marketplace | Rejected: not the cause; the same catalog names the installed marketplace for Codex, Muse and Cursor users. |
| Replicate Codex's hook hash to list pending reviews in preflight | Rejected: couples toolu to Codex internals; the pin check finds the cause deterministically. |

Jev (`toolu jev ask`, 2026-10-24): fix=pin 0.92; corrected cause 0.96;
needs-human by pane text 1.0; preflight by pin check 0.92.
