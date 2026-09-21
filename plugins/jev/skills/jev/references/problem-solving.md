# Jev problem-solving patterns

Load Setup + the matching example only. Evidence → focused judgment → next action.
Reassess changed inputs; reuse unchanged evidence/questions/criteria across stages.
Synthetic scenarios; replace observations with actual evidence in real work.

Sources: TypeSafe [semantic search](https://docs.typesafe.ai/cookbooks/semantic_find.md)
(separate ranking/existence), [citation checking](https://docs.typesafe.ai/cookbooks/citation_check.md)
(exact quote check before context judgment), [batching](https://docs.typesafe.ai/patterns/fan-out.md)
(independent questions, shared state). Evaluation only: [record](../evals/README.md).

## Setup and failure handling

Run setup + selected example in one Bash shell. Requires `curl`, `jq`, environment
`TYPESAFE_API_KEY`; never read `.env`.

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

Failure → apply the example's rubric manually. Missing output is not a negative
judgment. Uncertain/conflicting distributions → inspect source, gather evidence,
or narrow question. No universal thresholds. Confidence is concentration, not
correctness. Retain raw model/usage for evaluation; never credentials.

## Search: rank excerpts and detect no answer

Named retrieved excerpts: access/restoration and billing. Neither states retention.

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

Interpret: sort comparable Scores in code; inspect legend/distribution. A top
rank can still be incomplete. Choice probabilities are relative, not graded
relevance. Here, access ranks first but existence should be negative.
Next: retrieve retention policy; never infer retention from billing frequency.
Uncertain → inspect surrounding source/refine retrieval. API failure → manually
confirm missing duration, then retrieve policy.

## Debugging: prioritize an experiment, then reassess

Supplied observations + hypotheses + experiments. Judgments prioritize tests;
they do not prove causes.

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

Interpret: first judgment prioritizes the cache experiment. New observations
invalidate reuse; second should select `insufficient`. Next: test a local-storage
hypothesis by removing/restoring only the saved override in a disposable profile.
Confirm through reproduction before fixing. Cache still selected → inspect
contradictory evidence, revise candidates. Uncertainty/API failure → choose the
discriminating experiment from explicit evidence, without claiming certainty.

## Planning: compare approaches against separate preferences

Named alternatives + separate preferences → independent questions, one batch.

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

Interpret: worker fits both supplied preferences. Next: verify worker capacity,
retry semantics, and export requirements; Jev cannot validate estimates or
feasibility. Conflicting preferences/`neither`/uncertainty → inspect tradeoffs or
missing requirements. Reuse unchanged results at review; changed capacity →
reassess affected questions. API failure → manually compare each preference and
retain the technical checks.

## Review: support, contradiction, or unsupported claim

Named policy source + quote + claim. Check quote presence with code, then context.

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

Interpret: `contradicts` → correct claim and cite full passage. `unsupported` →
retrieve evidence, qualify, or drop claim; unsupported does not mean false.
`supports` concerns this excerpt, not source truth/currency. Next: verify provenance
and product behavior with tools. Uncertain/API failure → read full passage and
record explicit reasoning; never fabricate a Jev verdict.

Clean up after retaining evaluation notes:

```bash
rm -rf "$JEV_EXAMPLES"
```
