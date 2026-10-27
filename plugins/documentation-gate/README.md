# documentation-gate

Keep documentation in sync with changed behavior through a Jev-routed editing worker and committed-diff review.

## Install

```text
/plugin install documentation-gate@toolu
```

**Prerequisites:** the native [`toolu` binary](../../docs/install.md), the `toolu` plugin, exported Jev credentials, and a ready supported agent profile. See [runtime](../../docs/runtime.md).

## Crate

`crates/documentation-gate` provides `toolu docs inspect`, `toolu docs route`, and `toolu docs write-state`; see the [CLI reference](../../docs/cli/README.md).

## What it provides

- **`documentation-gate:sync`** audits changed behavior, updates or adds documentation with a separate routed worker, and records a committed review.
- **SessionStart hook** registers a native pre-push rule in the toolu dispatcher. The rule and ledger use the same review assessment and configured `docsSync` mode; no model is called by the hook.

`inspect` reports the committed branch diff, not uncommitted edits, against the base branch `origin/HEAD` names (`main` when unset). If that branch does not exist, run `git remote set-head origin <default-branch>`, or export `DOCS_SYNC_BASE` before starting the host. A push with nothing to review passes in any form. Otherwise the gate supports one static, plain `git push` with an explicit remote and one source resolving to `HEAD`: a local branch resolving to `HEAD` or `HEAD:<branch>`, with ordinary no-value flags. For example, use `git push <remote> HEAD:<branch>`. Implicit pushes, multiple refs or commands, `--all`, `--mirror`, `--tags`, dynamic `-C`, and `--git-dir`, `--work-tree`, or `-c` overrides are rejected under the effective gate mode. So are dynamic commands, subcommands or `sh -c`/`eval` scripts and Git alias definitions in a command line that mentions `push`, and configured Git aliases that expand to `push`. Static `git -C PATH` is supported; `push.followTags=true` is rejected. Record a review with `toolu docs write-state --host <host>`, naming the agent host whose gate reads it. A missing Jev credential or suitable ready route is an error and starts no worker. See [sync](skills/sync/SKILL.md).

## Muse

The Muse manifest uses generated hooks and resolvers. Validate with `muse plugins validate plugins/documentation-gate`.
