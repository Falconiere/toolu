---
name: review
description: Review the committed push diff with native OpenRouter and Jev, then return the compact verdict and usage report. Use when asked for a code review or before pushing a branch.
---

# code-review:review

Run `toolu --json code-review run` from the repository root. Pass `--base REF` or repeated `--path PATH` only when the user supplied that scope. Pass `--effort small|medium|high|xhigh` only when the user requested a level; otherwise the project setting or Jev selects it. Return the verdict, validated findings, coverage, recap and usage from the JSON. Do not read the full diff or send it to subagents; the native command does the review.

An exit of 2 means findings or incomplete coverage block the push. Exit 69 names `error.kind`: a total model outage fails closed and still reports Mechanical checks. On `not-enabled`, ask the user to opt in through the project's `toolu.config.json`; never write the opt-in yourself. The user must export `OPENROUTER_API_KEY` in this command's environment. Codex must run with outbound network enabled; its `workspace-write` sandbox blocks the provider call. Muse must forward the exported key to the command shell.

The per-repository opt-in permits outbound masked zero-context patch, bounded surrounding source, base-ref conventions, pre-pass findings, and open prior findings with dismissals. No source, quote, or credential goes into the append-only usage log. To settle a finding by exact ID without another model call, run `toolu code-review dismiss <finding-id> --reason <text>`; the command refreshes push state only when the previous complete review still matches the clean committed diff.
