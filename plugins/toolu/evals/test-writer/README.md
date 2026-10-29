# test-writer evals

Live `claude plugin eval` cases for the `test-writer` skill and the `test-author` agent. Each case is a directory with a `prompt.md` (frontmatter plus the prompt) and `graders/*.md`. Runs are billed to your Claude account and are not part of `cargo xtask gate`. The trial and grading rules are in `docs/agent-text-evals.md`.

| Kind | Cases | What it checks |
|---|---|---|
| Trigger (`trigger` tag) | 5 positive (`trigger-*`), 3 negative (`neg-*`) | The skill loads for test-writing prompts, including small ones, and stays out of debugging, renames and reviews. |
| Behavior (`behavior` tag) | `tdd-python-duration` | Tests come first and run red before green. Few, mock-free tests and few test runs. |
| | `python-planted-bug` | Expected values come from the spec: a boundary bug surfaces as a failing test and a report, and the code stays unedited. |
| | `js-lean-suite` | At most 8 distinct, behavior-level tests for correct code, with no mocks. |
| | `playwright-login-style` | Role and label locators, web-first assertions, no fixed waits, no CSS selectors. |
| | `appium-login-style` | Accessibility-id locators, no XPath, no fixed sleeps. |
| | `agent-receipt` | Delegation to `test-author`, and a receipt of 15 lines or fewer instead of raw test output. |

## Run

From the repository root. Iterate with one run and no baseline:

```text
claude plugin eval plugins/toolu --trust-plugin --tag trigger --runs 1 --ablation none
claude plugin eval plugins/toolu --trust-plugin --tag behavior --runs 1 --ablation none --allow-tools Bash Write Edit
```

Before merging a change to the skill or the agent, run every case three times with the no-plugin baseline arm, under a cost ceiling:

```text
claude plugin eval plugins/toolu --trust-plugin --allow-tools Bash Write Edit -j 4 --max-cost-usd 20
```

Results and the HTML report go to `plugins/toolu/evals/results/`, which git ignores. `claude plugin eval` refuses an eval directory inside `skills/`, so this suite lives under `plugins/toolu/evals/` rather than beside the skill. A case passes when its with-plugin score is at least `--threshold` (default 1.0). `Δ` is the with-plugin score minus the baseline score; on behavior cases a Δ near zero means the skill no longer adds anything. Each run in the `--json` output records `turns`, `costUsd` and `durationSeconds`.

## Recorded result (2026-10-29)

Claude Code 2.1.312, one host:

- Trigger: all 8 cases scored 1.0 over 3 runs each, after the description became a "Load BEFORE …" directive. Before that change, `trigger-rust-unit` scored 0 of 3.
- Behavior: all 6 cases scored 1.0 in a single run each, with no baseline arm.

A trigger run cost about $0.10; a behavior run cost $0.15–0.25.

## Limits

- Plugin eval has no script graders, and the gate bans tracked shell and TypeScript files, so there are no scaffold scripts. Each case embeds its code in `prompt.md` and is graded with regexes, tool-use counts and LLM judges. No case runs mutation tests.
- The eval sandbox gives each run a temporary `HOME` and no network, so `cargo` cannot run. Behavior cases use Python and Node; Rust is covered by a trigger case only.
- `agent-receipt` matches the Agent tool's `subagent_type` input. That field name is inferred; the plugin-eval docs do not state it.
- Only Claude Code runs this harness. Codex, Muse, Cursor and OpenCode are not covered.
- The toolu hooks load from this checkout and call the first `toolu` on `PATH`. Put `target/debug` first on `PATH` when the installed binary is older than the plugin.
