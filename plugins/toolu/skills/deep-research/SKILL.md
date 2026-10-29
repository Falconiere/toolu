---
name: deep-research
description: "Use for deep research that needs a verified, multi-source cited report. Use research-agent for bounded lookups and deep-explore for local code questions."
---

# Deep research

Produce a cited report that answers the user's question. State the scope,
guiding questions and assumptions, then proceed without a separate approval
step. Ask only for missing information that materially changes the research.

1. **Retrieve.** Read [research tools](../../workflows/research-tools.md).
   Discover available Context7/Exa tools; prefer Context7 for library/API docs
   and Exa for broader web research, with native search/fetch fallback.
   Keep a compact claim → source passage → URL/date ledger.
2. **Divide only when useful.** Work inline for dependent or small questions.
   For independent substantial questions, use bounded `research-agent`
   assignments with explicit scope, available tools and a compact evidence
   return. Respect host concurrency and the user's budget; a report does not
   require a fixed number of agents. Use the orchestrator's model-routing
   reference only when delegating.
3. **Verify.** Read original passages behind load-bearing claims. Check date,
   version, provenance and contrary evidence; multiple copies of one source
   are not independent corroboration. An independent verifier is useful for
   disputed/high-impact claims, not mandatory for every source. Use
   [Jev](../../workflows/semantic-judgments.md) for semantic support judgments;
   tools verify quotations and provenance.
4. **Resolve gaps.** Mark claims confirmed, unsupported or source-unreachable.
   Drop unsupported conclusions or label the uncertainty. A failed worker/tool
   leaves an explicit gap; a targeted retry or alternative source must address
   its cause. With no live sources, label the report unverified.
5. **Deliver.** Synthesize the answer, disagreements and remaining questions.
   Save `docs/research/<slug>.md` unless the user requested another destination.
   On collision use a numeric suffix; overwrite only when requested.
   Return a short answer and the report path.

The report needs its date/scope, concise answer, sections for the guiding
questions, claim-level citations and reliability notes (verification gaps,
failed retrieval/workers and limits). Use as many sources as the claims need;
do not pad with a quota or paste raw pages. Stop when questions are supported
or the remaining gaps and effort limit are explicit.
