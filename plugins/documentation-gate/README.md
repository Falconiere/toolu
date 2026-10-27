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

`inspect` reports the committed branch diff, not uncommitted edits. Check out and review the branch being pushed. The gate supports one static, plain `git push` with an explicit remote and one source resolving to `HEAD`: a local branch resolving to `HEAD` or `HEAD:<branch>`, with ordinary no-value flags. For example, use `git push <remote> HEAD:<branch>`. Implicit pushes, multiple refs or commands, `--all`, `--mirror`, `--tags`, dynamic or wrapped commands, dynamic `-C`, and `--git-dir`, `--work-tree`, or `-c` overrides are rejected under the effective gate mode. Static `git -C PATH` is supported; `push.followTags=true` is rejected. A missing Jev credential or suitable ready route is an error and starts no worker. See [sync](skills/sync/SKILL.md).

## Muse

The Muse manifest uses generated hooks and resolvers. Validate with `muse plugins validate plugins/documentation-gate`.
