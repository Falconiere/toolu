# code-review

Native review of the committed push diff with OpenRouter and Jev. It returns anchored findings, a compact verdict, coverage and usage, then writes gate-compatible push state.

## Install

```
/plugin install code-review@toolu
```

**Prerequisite:** the native [`toolu` binary](../../docs/install.md). Bun is not used or needed. See [docs/runtime.md](../../docs/runtime.md).

For Codex: `codex plugin add code-review@toolu`, or `toolu plugins install code-review --host codex`.

This plugin has no hook. In skills, commands and agents, write plain `toolu code-review <verb>`; never put a fixed binary path in static Markdown. With no SessionStart hook of its own, it relies on plain `toolu` resolving in the agent command shell, or on another installed toolu plugin's SessionStart supplying the native binary's absolute path for that session. See [docs/install.md](../../docs/install.md#agent-command-shell).

It needs `git`, which `toolu doctor` checks, and an exported `OPENROUTER_API_KEY`. The namespaced [review skill](skills/review/SKILL.md) is the primary agent surface; Claude and Muse also have a thin command entry. No hook starts a review automatically.

## Crate

`crates/code-review` contributes `toolu code-review run [--base REF] [--path PATH]... [--effort auto|small|medium|high|xhigh]` and `toolu code-review dismiss <finding-id> --reason TEXT`. The generated reference is in [docs/cli/](../../docs/cli/README.md). Without the per-repository opt-in (`"codeReview": {"enabled": true}` in the project `toolu.config.json`, see [docs/config.md](../../docs/config.md#native-code-review-codereview)) it sends nothing and exits 69 with `error.kind: "not-enabled"`; ask the user to opt in, never write the setting yourself. `--effort` defaults to `codeReview.effort`, else `auto`.

The native pipeline picks effort, builds a bounded prompt from the committed diff and base-ref conventions, calls OpenRouter by chunk, and judges fresh findings with Jev. It anchors and validates findings before deriving the verdict. Under `--json`, it prints the report contract (`schema_version`, `toolu_version`, `verdict`, `effort`, `findings`, `coverage`, `usage`, `error`, plus nonempty recap, Mechanical checks and notes). The schema is `$defs/codeReview` in [docs/cli/commands.schema.json](../../docs/cli/commands.schema.json).

The command writes v2 push-review state through the same `toolu-state` writer as `toolu review write-state`; the last writer wins. Medium and above count as blocking. Path-scoped, dirty or incomplete reviews cannot write passing state. A separate locked branch file holds finding memory and recent passes. The append-only usage log records model, provider, reported quantization, tokens, cost and timing without source or credentials.

## What it provides

- `toolu code-review run`: exit 0 approved, 2 blocked by a finding or incomplete coverage, 64 usage, 69 not enabled/unconfigured/outage/deadline (`error.kind` distinguishes them), and 1 local Git or state failure.
- `toolu code-review dismiss`: records an exact-ID dismissal and refreshes gate state without a model call only when the prior full review matches the clean committed diff.
- Codex needs outbound network enabled; its `workspace-write` sandbox blocks OpenRouter. Muse needs the key exported in its command environment.

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`. Validate it with `muse plugins validate plugins/code-review`.
