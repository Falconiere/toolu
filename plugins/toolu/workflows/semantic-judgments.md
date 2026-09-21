# Jev in the development workflow

The `jev` skill is **mandatory on every task containing semantic decisions**.
Explore first to gather relevant evidence, identify useful judgments, and call
Jev before the decisions they inform. Reassess opportunities throughout the task
after new evidence, failed hypotheses, or changed requirements. Batch independent
questions over shared state; reuse answers while evidence and question meanings
are unchanged. Neither a ceremonial first call nor a call quota satisfies this
workflow. A task with no semantic decision (a confirmation or a deterministic
lookup) is stated as such in one sentence.

The installed Jev skill links to `references/problem-solving.md`, with runnable
search, debugging, planning, and review examples. The table gives opportunities,
not a requirement to ask every question at every stage.

| Stage | Useful judgment | Keep with the agent or deterministic tools |
| --- | --- | --- |
| Brainstorm | Compare concrete candidates against one stated user preference per question. | Generate alternatives, resolve goals, and choose the architecture. |
| Spec | Check whether a requirement expresses one observable outcome or has ambiguous wording. | Design interfaces and establish technical feasibility. |
| Spec review | Check a supplied requirement/evidence pair for a semantic mismatch. | Verify completeness, executable contracts, and the approval verdict. |
| Plan | Classify supplied work by a stated rubric when that classification changes its handling. | Resolve dependencies, paths, ordering, and estimates. |
| Plan review | Check whether a step's described outcome addresses its linked requirement. | Validate AC ids, ledger structure, commands, and dependency graphs. |
| Search and research | Rank retrieved excerpts and check whether any answers the question; assess claim/source alignment. | Retrieve sources, verify quotes and provenance, and identify gaps. |
| Debugging | Compare supplied hypotheses against observations to prioritize the next experiment; reassess after it. | Reproduce, instrument, falsify hypotheses, and confirm the cause. |
| Execution and review | Triage a supplied finding or compare expected and observed text against a narrow rubric. | Reproduce bugs, inspect code, run tests, and decide correctness. |
| Test design | Identify semantic boundary cases in supplied examples. | Compute expected outputs and assert test results. |

Read the installed `jev` skill for the host's published wrapper path and CLI.
If the plugin, wrapper, credentials, or service is unavailable, state that
limitation once per task and continue with explicit reasoning and repository
evidence. Do not install tools to satisfy the rule, and keep each call a real
decision over real evidence rather than a ceremonial one.

Send only the relevant excerpts, named candidates, and rubric, preferably as an
object with named fields the questions point at by backticked path. Write the
exact condition (Jev reads literally), split independently useful judgments while
allowing a coherent contextual judgment. Use descriptive levels. Keep arithmetic,
counting, and date comparison in code. Batch independent
questions over the same state into one `ask` request. A Choice picks one option;
use comparable per-candidate Scores when graded ranking matters. A Noul near
0.5 expresses uncertainty. For Choice/Score, confidence measures distribution
concentration, not correctness. Gather evidence or reason through uncertain
answers instead of treating them as permission to proceed.

Record a useful result beside the decision: evidence, question, answer, and how
it affected the next action. Reuse it across stages while that evidence and
question remain unchanged. Keep model/usage with `--raw` when evaluating cost
or reproducibility; pin a model when thresholds depend on its behavior. Jev
does not approve a spec, waive a check, or establish that code is correct.
