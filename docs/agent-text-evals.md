# Agent text evaluations

These suites test shipped skills, command routers, agent roles and SessionStart
guidance. They complement the native plugin tests; JSON validity or a passing
quality gate is not evidence that an agent follows the text correctly.

## Suites

| Suite | Coverage |
| --- | --- |
| `plugins/code-review/skills/review/evals/evals.json` | Native/fallback review, real source fixtures, scope, approval and Ponytail |
| `plugins/toolu/skills/orchestrator/evals/behavior-evals.json` | Five agent roles, research tools, orchestration and language guidance |
| `plugins/toolu/skills/orchestrator/evals/workflow-evals.json` | Workflow/command routing, SessionStart, installed/missing/disabled Ponytail |
| `plugins/toolu/skills/orchestrator/evals/routing-evals.json` | Primary-skill selection with adjacent negative controls |
| `plugins/epic-orchestrator/skills/epic-orchestrator/evals/evals.json` | Plan/run/status/pause and recovery boundaries |
| `plugins/pr-babysit/skills/babysit/evals/evals.json` | Host continuation, strict clearance, uncertain outcomes and cancellation |
| `plugins/toolu/skills/debug/evals/cases.json` | Description-only positive and negative routing |
| `plugins/toolu/evals/test-writer/` | Live `claude plugin eval` cases: routing, spec-derived tests, red before green, lean mock-free suites, E2E locator style, the `test-author` receipt |
| `plugins/jev/skills/jev/evals/README.md` | Optional live judgments, evidence changes and unavailable-service fallback |

## Run a trial

Use the installed host's normal skill/command/agent invocation in an isolated
temporary checkout. Record host/model/version, source revision and dirty diff,
available tools, prompt, loaded paths, output, tool trace, artifacts, exit status,
elapsed time and usage when exposed. Never publish credentials. Keep trials
independent; prior answers and grades must not enter the next actor's context.

Give the actor only `prompt`, `targets` (the entrypoint text to load) and `files`
(input evidence), resolving paths from the repository root. For older suites,
use their documented owning skill when `targets` is absent. The grader alone
receives `assertions` and `expected_output`; do not let the actor open the suite.
References load through the entrypoint's normal routing, not an upfront dump.

Cases marked simulated evaluate decisions about supplied observations. The actor
must state proposed actions without executing external mutations. They do not
establish that GitHub, MCP, herdr or a host integration works. For real-tool
trials, use actual isolated resources and assert resulting state; never mock an
integration and report it as verified. Repo fixtures support read-only reviews.
Live epic/babysit delivery needs a dedicated authorized disposable project.

## Grade behavior

- **Outcome:** the requested answer/artifact/state is correct. Compile or test
  produced code, inspect Git/artifacts, and verify citations where applicable.
- **Instruction adherence:** scope, authorization, tool choice, failure handling
  and coverage are preserved. Exact order matters only for a real contract
  (for example, resolving a Context7 library before querying it).
- **Efficiency:** compare loaded bytes/words, tool calls, retries, elapsed time
  and actual token usage when available. Words/4 is not measured token usage.

Grade deterministic facts from artifacts/traces first. Use a separate judge for
qualitative evidence support, supplying the rubric and evidence; retain its
reason and uncertainty. Any approval/scope/secret-handling failure fails the
case regardless of average score. Unknown or unrun is not pass.

Routing trials expose only catalog names/descriptions plus a prompt, with both
positive and adjacent negative controls. Execution trials explicitly invoke the
entrypoint. Keep those scores separate: choosing a skill does not prove execution.
Start with a small smoke set, then run 3–5 isolated trials per case on supported
hosts. Compare the previous text, revised text and skill-unloaded baseline on
the same prompts and environment. Do not retire a workflow contract solely
because a base model passes a capability case. Add held-out cases from observed
failures rather than teaching the actor the grader's wording.

Store results separately from runtime instructions. Each result needs case ID,
trial/mode, checks with pass/fail/unrun and evidence locations, measured costs
and limitations. Report the denominator and missing hosts/cases; never turn a
single simulated pass into an integration or reliability claim.

Method informed by [Philipp Schmid's testing guide](https://www.philschmid.de/testing-skills)
and [Anthropic's agent evaluation guide](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents).
Compression follows [progressive context retrieval](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents).

## Recorded smoke run

[2026-10-29 results](../plugins/toolu/skills/orchestrator/evals/results-2026-10-29.json)
record a description-only routing run (21/22 before a trigger correction, 22/22
after) and four isolated behavioral trials. These are single-trial smoke
observations; the limitations and actual evidence are in the result file.
