---
name: review
description: Review the committed push diff with native OpenRouter and Jev, automatically falling back to toolu-review:review if native review cannot complete. Use when asked for a code review or before pushing a branch.
---

# code-review:review

Run `toolu --json code-review run` from the reviewed repository root. Pass `--base REF` or repeated `--path PATH` only when the user or calling workflow supplied that scope. Pass `--effort small|medium|high|xhigh` only when requested; otherwise the setting or Jev selects it. Return the verdict, validated findings, coverage, recap and usage from the JSON. The native command reads the diff; read it yourself only when fallback needs it.

Review is enabled by default. User or project `codeReview.enabled: false` disables native requests; project settings take precedence. Invalid configuration also disables requests and reports why. Never change configuration or credentials to force a native review. Native review needs `OPENROUTER_API_KEY` exported in the command environment and outbound network access; Muse must forward the key to the command shell.

Use the report as well as the exit code:

- A complete native review with blocking findings requires fixes and re-review. Do not invoke fallback merely to bypass those findings.
- If native review cannot complete, automatically invoke the existing `toolu-review:review` skill in the active host's invocation syntax. This includes disabled (`not-enabled`), unavailable command or skill, missing credentials (`unconfigured`), outages, deadlines, state errors, non-JSON failures (Git or branch memory), and incomplete coverage even when exit 2 also reports findings. Report the fallback reason once; do not ask the user to opt in or choose a reviewer. If this native skill itself is unavailable, the calling workflow invokes fallback directly.
- Preserve the requested repository/worktree, base, paths and review intent through fallback. Retain and assess every real finding already reported by native review, including Mechanical checks and prior open findings; fallback cannot erase unresolved findings. An operational failure alone is not a code defect. Fix or dismiss real findings only with evidence, and re-review changed code.
- Read and follow `toolu-review:review` completely. Run its review directly; never route fallback back into `code-review:review` or retry reviewers recursively. If fallback is unavailable or fails, report the blocker and stop without writing approval.
- Record passing push-review state only after a complete, clean review of the committed diff in a clean worktree with all real findings resolved. Uncommitted fixes must be committed and re-reviewed before approval. Do not fabricate approval or count unreviewed files as reviewed. A path-scoped review cannot approve the whole branch; report that limitation and do not run the fallback's whole-diff state writer for partial coverage. Preserve the supplied base when recording state; if the writer cannot represent that scope, report the limitation without approval. In a detached worktree fallback must use the calling workflow's target branch and repository for state.

Native CLI exit codes and JSON remain authoritative for standalone invocations: 2 blocks for findings or incomplete coverage; 69 identifies `error.kind`; operational errors may exit 1. Automatic fallback belongs to this skill and its callers.

Enabled review sends masked zero-context patch, bounded surrounding source (line windows around the changes when a file is too big to send whole), at `high` and `xhigh` the bounded `HEAD` lines elsewhere in the repository that name the changed definitions, base-ref conventions, pre-pass findings, and open prior findings with dismissals. No source, quote, or credential goes into the append-only usage log. To settle a finding by exact ID without another model call, run `toolu code-review dismiss <finding-id> --reason <text>`; it refreshes push state only when the previous complete review still matches the clean committed diff.
