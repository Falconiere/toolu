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

`inspect` reports the committed branch diff of the current directory's repository (or `--repo`), not uncommitted edits. The base is `DOCS_SYNC_BASE`, else the branch `origin/HEAD` names (`main` when unset), then `master` and `main`, each locally or on `origin`. If none exists, run `git remote set-head origin <default-branch>`, or export `DOCS_SYNC_BASE` before starting the host. A lone plain push whose commits are all already in the base passes in any form. Otherwise the gate supports one static, plain `git push` with an explicit remote and one source resolving to `HEAD`: a local branch resolving to `HEAD` or `HEAD:<branch>`, with ordinary no-value flags. For example, use `git push <remote> HEAD:<branch>`. Implicit pushes, multiple refs or commands, `--all`, `--mirror`, `--tags`, dynamic `-C`, and `--git-dir`, `--work-tree`, or `-c` overrides are rejected under the effective gate mode. So are dynamic git subcommands, `send-pack`, `http-push`, `subtree push`, Git aliases set in the line or configured to push, and dynamic commands, `sh -c`/`eval` scripts or unparseable lines that mention `push`. Static `git -C PATH` is supported. When `push.followTags=true`, add `--no-follow-tags`. `toolu docs write-state` records the review for the detected host (or `--host`); every host's gate and the ledger read it. A missing Jev credential or suitable ready route is an error and starts no worker. See [sync](skills/sync/SKILL.md).

## Muse

The Muse manifest uses generated hooks and resolvers. Validate with `muse plugins validate plugins/documentation-gate`.
