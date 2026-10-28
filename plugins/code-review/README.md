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

The dirty-worktree check excludes only this plugin's untracked review state and usage files. Other untracked host files still count as uncommitted changes; ignore them in the repository if they are intentionally local.

## What it provides

- `toolu code-review run`: exit 0 approved, 2 blocked by a finding or incomplete coverage, 64 usage, 69 not enabled/unconfigured/outage/deadline or a state write that failed after the review (`error.kind` distinguishes them), and 1 when Git, the branch or branch memory cannot be read before the review.
- `toolu code-review dismiss`: records an exact-ID dismissal and refreshes gate state without a model call only when the prior full review matches the clean committed diff.
- Codex needs outbound network enabled; its `workspace-write` sandbox blocks OpenRouter. Muse needs the key exported in its command environment.

## Eval

`toolu code-review eval --cases DIR [--arm baseline|enhanced|both] [--effort LEVEL]... [--live] [--replay-dir DIR] [--report PATH]` runs a paired comparison of the review with the Jev judge off (baseline) and on (enhanced) against committed expectations. `--cases` points at a set such as `fixtures/code-review-eval` (`expectations.json` and `cases/<id>/files/{base,tip}`). `--arm` defaults to `both`.

`--effort` repeats for calibration and accepts `none`, `auto`, `small`, `medium`, `high` and `xhigh`; the default is `auto`. `none` is the known-weak baseline: the lowest level, never judged.

Replay is the default. Without `--live` every model call is answered from recorded responses in `--replay-dir` (default `<cases>/replay`), nothing is sent and no key is needed; a request with no recording fails that case closed and the run is unverified. `--live` calls OpenRouter and Jev for real, records every exchange into the replay directory, and needs the repository opt-in and `OPENROUTER_API_KEY`. Without them it exits 69, unverified, before any request. Exit 0 means expectations were satisfied, 2 means they were missed, and 69 means unverified, which is not a pass.

### Quality bar

Fewer findings or fewer tokens alone is not better. A review is judged by expectation outcomes: defects caught, false positives raised and unclassified findings. A finding that matches no expectation is reported as unclassified; it is neither credited nor silently dropped. Compare arms and effort levels on those outcomes first, and read tokens per finding only as a cost beside them.

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`. Validate it with `muse plugins validate plugins/code-review`.
