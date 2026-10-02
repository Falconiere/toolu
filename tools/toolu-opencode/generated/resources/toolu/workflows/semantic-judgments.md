# Jev workflow

**Mandatory for semantic decisions when available.** Explore → gather evidence →
call Jev before the informed decision → act. Reassess after new evidence, failed
hypotheses, or changed requirements. Batch independent questions over shared
state; reuse results while evidence, questions, and criteria are unchanged.
Dependent questions wait for their inputs. No quotas or ceremonial calls.

No semantic decision → say so once. Unavailable → state limitation once/task;
continue from explicit evidence/reasoning. Do not install tools to satisfy this
rule or invent results.

## Judgment opportunities

| Stage | Ask about supplied evidence |
| --- | --- |
| Brainstorm | Compare concrete alternatives per user preference. |
| Spec | Requirement ambiguity; one observable outcome. |
| Spec review | Requirement/evidence alignment. |
| Plan | Classification that changes a step's handling. |
| Plan review | Step outcome vs. linked requirement. |
| Search/research | Excerpt relevance, answer existence, claim/source support. |
| Debug | Hypothesis fit; next experiment; reassess after results. |
| Execution/review | Finding triage; expected vs. observed behavior. |
| Test design | Semantic boundaries in supplied examples. |

Use relevant rows, not a stage checklist. Agent/tools own architecture,
feasibility, exact checks, arithmetic, reproduction, tests, correctness, and
approval. Jev never waives verification or authorization.

Read the installed `jev` skill for host path, CLI, question contract, and
uncertainty handling. Load only the needed section of its linked
`references/problem-solving.md`. Keep state to named, relevant excerpts and
candidates. Record useful evidence → question → answer → next action; retain
model/usage with `--raw` during evaluation.
