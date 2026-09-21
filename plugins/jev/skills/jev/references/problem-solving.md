# Jev throughout problem solving

Gather evidence, ask a focused typed question, interpret the result, then act.
After an experiment, retrieval, or requirement change, decide which questions need
fresh evidence. Reuse an answer when its evidence, question, and criteria remain
unchanged; do not call again just because the workflow stage changed.

These synthetic examples adapt TypeSafe's [semantic search](https://docs.typesafe.ai/cookbooks/semantic_find.md)
(separate answer existence from ranking), [citation checking](https://docs.typesafe.ai/cookbooks/citation_check.md)
(check exact quotes with code before judging context), and [batching](https://docs.typesafe.ai/patterns/fan-out.md)
(independent questions share one request). They use the existing CLI, not an SDK.
The search example uses comparable Scores for graded relevance; a Choice's
probabilities describe relative alternatives, not absolute relevance.

The [opt-in evaluation record](../evals/README.md) describes how to repeat the
agent-session checks and records observed decision impact.

## Setup and failure handling

Run setup and the desired example in the same Bash shell. Requires `curl`, `jq`,
and `TYPESAFE_API_KEY` already in the launch environment; never read `.env`.

```bash
# Codex (for Claude Code use the second line instead):
JEV="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/jev/jev.sh"
# JEV="${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/jev/jev.sh"
# Repository development, from the repository root, if not installed:
if [ ! -x "$JEV" ]; then JEV="$PWD/plugins/jev/skills/jev/scripts/jev.sh"; fi
JEV_EXAMPLES=$(mktemp -d)

judge() {
  local name="$1"
  if "$JEV" ask "$JEV_EXAMPLES/$name.questions.json" \
      -s "@$JEV_EXAMPLES/$name.state.json" --raw >"$JEV_EXAMPLES/$name.result.json"; then
    jq '.answers' "$JEV_EXAMPLES/$name.result.json"
  else
    rm -f "$JEV_EXAMPLES/$name.result.json"
    printf '%s\n' "Jev unavailable for $name; inspect the named evidence and apply the rubric manually." >&2
    return 1
  fi
}
```

On failure, use the manual next action described below. An absent result is never
a negative judgment. Keep raw model/usage when evaluating, without credentials.
Numbers and confidence are evidence to inspect, not correctness or authorization.
There are no universal thresholds here: ambiguous or conflicting distributions
mean gather evidence, narrow the question, or reason explicitly from the source.

## Search: rank excerpts and detect no answer

Evidence: two retrieved excerpts, identified by their source paths. The query
asks about retention; neither excerpt supplies a retention duration.

```bash
cat > "$JEV_EXAMPLES/search.state.json" <<'JSON'
{
  "query": "How long are deleted project backups retained?",
  "excerpts": {
    "access": {"source": "docs/access.md", "text": "Administrators may restore a deleted project from a backup."},
    "billing": {"source": "docs/billing.md", "text": "Invoices are emailed to the billing contact every month."}
  }
}
JSON
cat > "$JEV_EXAMPLES/search.questions.json" <<'JSON'
{
  "exists": {
    "type": "noul",
    "instructions": "Do the supplied `excerpts` contain an answer to `query`, including its requested retention duration?",
    "criteria": {"true": "The duration is stated or unambiguously implied in the excerpts.", "false": "The excerpts omit the duration, even if they discuss backups or restoration."}
  },
  "access_relevance": {
    "type": "score",
    "instructions": "How well does `excerpts.access.text` answer `query`? Use only this excerpt.",
    "criteria": ["Unrelated to deleted project backups.", "Discusses backups or restoration but omits the requested duration.", "Supplies the requested backup retention duration."]
  },
  "billing_relevance": {
    "type": "score",
    "instructions": "How well does `excerpts.billing.text` answer `query`? Use only this excerpt.",
    "criteria": ["Unrelated to deleted project backups.", "Discusses backups or restoration but omits the requested duration.", "Supplies the requested backup retention duration."]
  }
}
JSON
judge search
```

Interpretation: sort the comparable relevance Scores in code to choose what to
read first. Inspect the returned legend and distribution: the highest score can
still describe an incomplete answer. Here, even if access ranks first, the
existence judgment should be negative. Next action: retrieve retention policy
material; do not infer a duration from the billing cycle. If existence is unclear,
inspect source context and refine retrieval. If Jev fails, manually observe that
neither excerpt states a duration and take the same retrieval step.

## Debugging: prioritize an experiment, then reassess

Evidence: a synthetic observed failure and two supplied hypotheses. Jev compares
their fit; it neither invents observations nor proves a cause.

```bash
cat > "$JEV_EXAMPLES/debug.state.json" <<'JSON'
{
  "observations": {"cold": "A fresh profile renders the current label.", "warm": "An existing profile still renders the old label after refresh."},
  "hypotheses": {"cache": "A browser cache is serving stale assets.", "server": "The server serves an old build to every profile."},
  "experiments": {"cache": "Disable the browser cache in the affected profile and reload.", "server": "Compare the deployed asset hash against the build artifact."}
}
JSON
cat > "$JEV_EXAMPLES/debug.questions.json" <<'JSON'
{
  "next": {
    "type": "choice",
    "instructions": "Which supplied hypothesis best fits all `observations` and should have its `experiments` entry tried next? This is experiment prioritization, not proof of a cause.",
    "criteria": {"cache": "The profile-dependent behavior favors stale browser assets.", "server": "The observations favor the same old deployment affecting all profiles.", "insufficient": "Neither hypothesis explains the observations, or evidence cannot distinguish them."}
  }
}
JSON
judge debug

# Synthetic result of running the cache experiment; in real work supply actual output.
jq '.observations += {"cache_disabled": "Old label persists with browser cache disabled.", "network": "Both profiles receive identical current assets.", "storage": "Only the affected profile has a saved label override in local storage."}' \
  "$JEV_EXAMPLES/debug.state.json" > "$JEV_EXAMPLES/debug-next.state.json"
cp "$JEV_EXAMPLES/debug.questions.json" "$JEV_EXAMPLES/debug-next.questions.json"
judge debug-next
```

Interpretation: the first result can prioritize the cache experiment. After that
hypothesis fails, the changed observations invalidate reuse of the first answer.
The second result should favor `insufficient`: neither supplied hypothesis fits
all the new evidence. Next action: form a local-storage hypothesis and test it by
removing only the saved override in a disposable profile, then reproducing with
and without it. Do not ship a cache fix based on the first answer. If the model
still favors cache, inspect the contradictory observation and revise the candidate
set yourself. A close distribution or API failure calls for the discriminating
experiment and explicit reasoning, not a claim of certainty.

## Planning: compare approaches against separate preferences

Evidence: two concrete alternatives and two user preferences. These preferences
can favor different alternatives, so ask separate questions in one batch.

```bash
cat > "$JEV_EXAMPLES/plan.state.json" <<'JSON'
{
  "preferences": {"delivery": "Ship a working export this week.", "operations": "Avoid operating a new service."},
  "approaches": {"worker": "Use the existing job worker and database; estimated two days of implementation.", "service": "Build and deploy a dedicated export service; estimated two weeks plus new monitoring."}
}
JSON
cat > "$JEV_EXAMPLES/plan.questions.json" <<'JSON'
{
  "delivery": {
    "type": "choice",
    "instructions": "Which approach in `approaches` best fits `preferences.delivery`, taking the supplied estimates as assumptions rather than verified facts?",
    "criteria": {"worker": "The existing worker approach better meets the delivery preference.", "service": "The dedicated service approach better meets the delivery preference.", "neither": "Neither approach meets it, or the supplied information cannot distinguish them."}
  },
  "operations": {
    "type": "choice",
    "instructions": "Which approach in `approaches` best fits `preferences.operations`?",
    "criteria": {"worker": "The existing worker approach better meets the operations preference.", "service": "The dedicated service approach better meets the operations preference.", "neither": "Neither approach meets it, or the supplied information cannot distinguish them."}
  }
}
JSON
judge plan
```

Interpretation: worker is favored under these assumptions. Next action: inspect
the existing worker's capacity, retry semantics, and export requirements before
choosing the architecture. These judgments cannot validate estimates or technical
feasibility. Conflicting preferences, `neither`, or uncertainty require examining
tradeoffs or missing requirements. If an unchanged plan moves to review, reuse
these results. If capacity evidence changes an approach, reassess affected
questions. On failure, compare each preference against the supplied alternatives
manually and keep the same technical checks.

## Review: support, contradiction, or unsupported claim

Evidence: a synthetic policy excerpt, a quote, and a claim. Check quote presence
exactly in code, then judge its meaning in context.

```bash
cat > "$JEV_EXAMPLES/review.state.json" <<'JSON'
{
  "source": {"path": "docs/export-policy.md", "text": "Admins may request exports. Members cannot request exports; ask an admin to submit the request."},
  "quote": "Members cannot request exports",
  "claim": "Members may request exports directly."
}
JSON
cat > "$JEV_EXAMPLES/review.questions.json" <<'JSON'
{
  "relation": {
    "type": "choice",
    "instructions": "How does `source.text`, read in full context, relate to `claim`? Use only the supplied source, not outside knowledge.",
    "criteria": {"supports": "The source establishes the claim as stated, including its scope and qualifications.", "contradicts": "The source states something incompatible with the claim.", "unsupported": "The source neither establishes nor contradicts the claim, or lacks the context needed to decide."}
  }
}
JSON
if jq -e '.quote as $quote | .source.text | contains($quote)' \
    "$JEV_EXAMPLES/review.state.json" >/dev/null; then
  judge review
else
  printf '%s\n' 'Quote absent: fetch the source and repair the citation before semantic review.' >&2
fi
```

Interpretation: `contradicts` directs the reviewer to correct the claim to match
the policy and cite the full passage. `unsupported` means retrieve more evidence,
qualify, or drop the claim; it does not mean the claim is false. Even `supports`
is about the supplied excerpt, not proof that the source is current or truthful.
Next action: verify provenance and the actual product behavior with deterministic
checks. Inspect uncertain distributions manually; on API failure, read the full
passage and record the contradiction without fabricating a Jev verdict.

Clean up the scratch directory after retaining any evaluation notes:

```bash
rm -rf "$JEV_EXAMPLES"
```
