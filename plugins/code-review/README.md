# code-review

Native code review of the push diff with OpenRouter and Jev: anchored findings and a computed verdict as a local report. In development: `toolu code-review run` fails closed until the review pipeline lands. Standalone, no dependencies.

## Install

```
/plugin install code-review@toolu
```

**Prerequisite:** the native [`toolu` binary](../../docs/install.md). Bun is not used or needed. See [docs/runtime.md](../../docs/runtime.md).

For Codex: `codex plugin add code-review@toolu`, or `toolu plugins install code-review --host codex`.

This plugin has no hook. In skills, commands and agents, write plain `toolu code-review <verb>`; never put a fixed binary path in static Markdown. With no SessionStart hook of its own, it relies on plain `toolu` resolving in the agent command shell, or on another installed toolu plugin's SessionStart supplying the native binary's absolute path for that session. See [docs/install.md](../../docs/install.md#agent-command-shell).

Standalone, no dependencies. It needs `git`, which `toolu doctor` checks.

## Crate

`crates/code-review` is this plugin's Rust library. It contributes the `toolu code-review` namespace; the generated reference is in [docs/cli/](../../docs/cli/README.md). The namespace has one verb, `toolu code-review run [--base REF] [--path PATH]... [--effort auto|small|medium|high|xhigh]`. Without the per-repository opt-in (`"codeReview": {"enabled": true}` in the project `toolu.config.json`, see [docs/config.md](../../docs/config.md#native-code-review-codereview)) it sends nothing and exits 69 with `error.kind: "not-enabled"`; ask the user to opt in, never write the setting yourself. With the opt-in it still fails closed with `error.kind: "unconfigured"` until validation (#668) and the report (#669) land. `--effort` defaults to `codeReview.effort`, else `auto`.

The library's `review` module is the orchestration stage (#666): it picks the effort level, builds the cache-friendly prompt (fixed rules plus the 8-dimension checklist or the base-ref `.github/code-review-prompt.md`), calls the review model per chunk, and judges fresh findings with Jev (`typesafe/jev-1.13` over OpenRouter's System One endpoint, same `OPENROUTER_API_KEY`). A finding Jev contradicts is removed only when the review model dismisses it with a verified quote from the evidence. Jev failure, uncertainty or an expired deadline keeps the finding. Under `--json`, it prints the report contract (`schema_version`, `toolu_version`, `verdict`, `effort`, `findings`, `coverage`, `usage`, `error`). That contract is defined by `$defs/codeReview` in [docs/cli/commands.schema.json](../../docs/cli/commands.schema.json).

## What it provides

- `toolu code-review run`: the review verb. Exit 0 means approved, 2 means blocking findings (`blocker`, `high` or `medium`) or a path left unreviewed, 64 means a usage error, and 69 means not enabled, unconfigured, a total model outage, or every review call past its deadline (`error.kind` tells them apart). Exit 1 means the command itself failed, for example when a `--json` report cannot be serialised.
- The skill that runs it ships with the verdict report (#669).

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`. Validate it with `muse plugins validate plugins/code-review`.
