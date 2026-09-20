# Jev in the development workflow

The `jev` skill is **mandatory on every task**. Before acting on a request,
identify at least one bounded semantic decision it contains — the table below
names the useful one per stage — and, when Jev is available, call it. Prefer
the decision whose typed answer changes the next action; when several qualify,
batch them in one `ask` call. A task with genuinely no semantic decision (a
confirmation, a single deterministic lookup) is stated as such in one sentence,
never skipped silently.

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
If the plugin, wrapper, credentials, or service is unavailable, state that
limitation once per task and continue with explicit reasoning and repository
evidence. Do not install tools to satisfy the rule, and keep each call a real
decision over real evidence rather than a ceremonial one.

Send only the relevant excerpts, named candidates, and rubric, preferably as an
object with named fields the questions point at by backticked path. Write the
exact condition (Jev reads literally), one judgment per question, levels that
describe situations rather than degrees, and no arithmetic, counting, or date
comparison — extract parts, compute in code. Batch independent
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
