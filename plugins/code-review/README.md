# code-review

Native review of branch and local changes through Jev-selected host CLIs. It returns anchored findings, a compact verdict, coverage and usage. A complete review of a clean committed diff can approve a push.

## Install

```
/plugin install code-review@toolu
```

**Prerequisite:** the native [`toolu` binary](../../docs/install.md). Bun is not used or needed. See [docs/runtime.md](../../docs/runtime.md).

For Codex: `codex plugin add code-review@toolu`, or `toolu plugins install code-review --host codex`.

This plugin has no hook. In skills, commands and agents, write plain `toolu code-review <verb>`; never put a fixed binary path in static Markdown. With no SessionStart hook of its own, it relies on plain `toolu` resolving in the agent command shell, or on another installed toolu plugin's SessionStart supplying the native binary's absolute path for that session. See [docs/install.md](../../docs/install.md#agent-command-shell).

It needs `git`, which `toolu doctor` checks, an exported `TYPESAFE_API_KEY`, and an authenticated supported host CLI on `PATH`. The namespaced [review skill](skills/review/SKILL.md) is the primary agent surface; Claude and Muse also have a thin command entry. No hook starts a review automatically.

## Crate

`crates/code-review` contributes `toolu code-review run [--base REF] [--path PATH]... [--effort auto|small|medium|high|xhigh]`, `toolu code-review dismiss <finding-id> --reason TEXT` and `toolu code-review digest [--since REF] [--path PATH]... [--json]`. The generated reference is in [docs/cli/](../../docs/cli/README.md). Review is enabled by default; set `"codeReview": {"enabled": false}` in user or project `toolu.config.json` to opt out (project settings win; see [configuration](../../docs/config.md#native-code-review-codereview)). Disabled or invalid configuration sends nothing and exits 69 with `error.kind: "not-enabled"` and a reason. The agent skill automatically invokes `toolu-review:review` when native review cannot complete, including missing credentials, operational failures and incomplete coverage (local-only push ineligibility does not trigger fallback); it preserves scope and real findings and never approves failed or partial reviews. Standalone CLI exits remain unchanged. `--effort` defaults to `codeReview.effort`, else `auto`.

The native pipeline lets Jev pick the host CLI, the model, and the effort, builds a bounded prompt from the reviewed diff and base-ref conventions, calls the selected host CLI by chunk, and judges fresh findings with Jev. It anchors and validates findings before deriving the verdict. Under `--json`, it prints the report contract (`schema_version`, `toolu_version`, `verdict`, `effort`, `findings`, `coverage`, `usage`, `error`, plus nonempty recap, Mechanical checks and notes). The schema is `$defs/codeReview` in [docs/cli/commands.schema.json](../../docs/cli/commands.schema.json).

The command writes v2 push-review state through the same `toolu-state` writer as `toolu review write-state`; the last writer wins. Medium and above count as blocking. Path-scoped, dirty or incomplete reviews cannot write passing state. A separate locked branch file holds finding memory and recent passes. The append-only usage log records model, provider, reported quantization, tokens, cost and timing without source or credentials. Its active file is `<project-state>/code-review-usage.jsonl`; each file is limited to 1 MiB, with up to four complete archives in `<project-state>/code-review-usage/`, named by increasing sequence. Read archived files in sequence order, then the active file, while reviews are idle. The oldest archives are reclaimed after a successful append; an oversized legacy active file is reclaimed at its first rotation. A failed usage write, lock, rotation or cleanup blocks passing review state.

Staged, unstaged and non-ignored untracked files are included alongside committed branch changes against the selected merge-base. One temporary Git index captures the final working-tree contents; patches, structural scanning, surrounding source and caller context all read that snapshot. HEAD and the user's index are preserved. Resolve merge conflicts before reviewing.

Local reviews return findings and exit 2 because they cannot approve a push; the report includes a note to commit and re-run. They replace earlier approval with blocked state, even when the committed diff is empty or the files are committed during the review. Persistence failures also block approval.

The dirty-worktree check excludes only this plugin's review state and valid untracked usage archives. Other untracked host files still count as uncommitted changes; ignore them in the repository if they are intentionally local.

## What it provides

- `toolu code-review run`: exit 0 approved, 2 blocked by a finding or incomplete coverage, 64 usage, 69 not enabled/unconfigured/outage/deadline or a state write that failed after the review (`error.kind` distinguishes them), and 1 when Git, the branch or branch memory cannot be read before the review.
- `toolu code-review dismiss`: records an exact-ID dismissal and refreshes gate state without a model call only when the prior full review matches the clean committed diff.
- `toolu code-review digest`: a model-free summary of branch and uncommitted changes by file and item, from the same snapshot as `run`. `--since REF` digests from the merge-base of REF and HEAD (default: the push-review base) and `--path` limits it to paths; it needs neither a key nor network, writes no review state, and exits 0, or 1 when Git or the base cannot be read.
- Jev and the selected host CLI need network access. Muse needs the key exported in its command environment.

## Eval

`toolu code-review eval --cases DIR [--arm baseline|enhanced|both] [--effort LEVEL]... [--live] [--replay-dir DIR] [--report PATH]` runs a paired comparison of the review with the Jev judge off (baseline) and on (enhanced) against committed expectations. `--cases` points at a set such as `fixtures/code-review-eval` (`expectations.json` and `cases/<id>/files/{base,tip}`). `--arm` defaults to `both`.

`--effort` repeats for calibration and accepts `none`, `auto`, `small`, `medium`, `high` and `xhigh`; the default is `auto`. `none` is the known-weak baseline: the lowest level, never judged.

Replay is the default. Without `--live` every model call is answered from recorded responses in `--replay-dir` (default `<cases>/replay`), nothing is sent and no key is needed; a request with no recording fails that case closed and the run is unverified. `--live` calls the Jev-selected host CLI and Jev for real, records every exchange into the replay directory, and needs enabled review configuration (the default) plus `TYPESAFE_API_KEY`. When disabled, invalid or missing the key it exits 69, unverified, before any request. Recordings are not committed: `fixtures/code-review-eval/replay/` is gitignored, so run `--live` once in a fresh checkout before replaying. Exit 0 means expectations were satisfied, 2 means they were missed, and 69 means unverified, which is not a pass.

To replay older recordings made with a custom `OPENROUTER_MODEL`, set that same value in the replay environment. It is used to find the historical recording, not for live review calls.

### Quality bar

Fewer findings or fewer tokens alone is not better. A review is judged by expectation outcomes: defects caught, false positives raised and unclassified findings. A finding that matches no expectation is reported as unclassified; it is neither credited nor silently dropped. Eval suite success requires catching every defect row **and** raising no known false-positive row on non-`none` arms; unclassified findings stay reported for adjudication and do not alone fail the suite. Compare arms and effort levels on those outcomes first, and read tokens per finding only as a cost beside them.

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`. Validate it with `muse plugins validate plugins/code-review`.
