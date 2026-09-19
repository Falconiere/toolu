# Jev in the development workflow

Use the optional `jev` skill **if and only if** the current stage has a bounded
semantic decision, the relevant evidence is available, and its typed answer
would change the next action. When those conditions hold and Jev is available,
calling it is mandatory. Installation alone does not justify a request.

| Stage | Useful judgment | Keep with the agent or deterministic tools |
| --- | --- | --- |
| Brainstorm | Compare concrete candidates against one stated user preference per question. | Generate alternatives, resolve goals, and choose the architecture. |
| Spec | Check whether a requirement expresses one observable outcome or has ambiguous wording. | Design interfaces and establish technical feasibility. |
| Spec review | Check a supplied requirement/evidence pair for a semantic mismatch. | Verify completeness, executable contracts, and the approval verdict. |
| Plan | Classify supplied work by a stated rubric when that classification changes its handling. | Resolve dependencies, paths, ordering, and estimates. |
| Plan review | Check whether a step's described outcome addresses its linked requirement. | Validate AC ids, ledger structure, commands, and dependency graphs. |
| Execution and review | Triage a supplied finding or compare expected and observed text against a narrow rubric. | Reproduce bugs, inspect code, run tests, and decide correctness. |
| Test design | Identify semantic boundary cases in supplied examples. | Compute expected outputs and assert test results. |

Read the installed `jev` skill for the host's published wrapper path and CLI.
If the plugin, wrapper, credentials, or service is unavailable, state that limitation once and
continue with explicit reasoning and repository evidence. Do not install tools
or make ceremonial calls simply to satisfy a workflow stage.

Send only the relevant excerpts, named candidates, and rubric. Batch independent
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
