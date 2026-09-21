# Ongoing problem-solving evaluation

Manual, opt-in evaluation; never part of hermetic CI. Requires an existing
`TYPESAFE_API_KEY` in the launch environment. Do not read `.env` or record keys.

## Repeat

1. Run setup and the search, debugging (both evidence sets), planning, and review
   blocks in [problem-solving.md](../references/problem-solving.md), using the
   existing wrapper. Retain the scratch results until session evaluation ends.
2. In a fresh agent session, load the updated skill and reference. Ask it to
   select and record its next action, not just report the model's answers:
   - Search: replace the query with “What telescope aperture is recommended to
     resolve the rings of Saturn?” Keep the same two excerpts. Change the Noul
     criteria to require an aperture recommendation. Score each excerpt on the
     same levels: “Unrelated to telescopes or observing Saturn”, “Discusses
     observing Saturn but omits an aperture recommendation”, “Supplies the
     requested telescope aperture recommendation”. Neither excerpt matches.
   - Debugging: supply the first observations, record the chosen experiment,
     then supply the explicitly synthetic new observations. Record whether the
     agent abandons the first hypothesis and chooses a discriminating experiment.
   - Reuse: move the unchanged planning evidence and questions to review with
     the saved answer. Check that the agent reuses it without another call.
   - Unavailable: invoke the wrapper with `env -u TYPESAFE_API_KEY`, then ask
     the agent to continue from the supplied evidence and record its fallback.
3. Inspect the raw model, usage, choices, uncertainty, and decision impact. Do
   not assert exact probabilities. A plausible classification alone does not
   pass a case: it must lead to an appropriate next action or explicit fallback.

No application is included in these examples. Actual debugging experiments and
technical feasibility checks need a real target; do not report them as executed.
Installed-host hook lifecycle behavior is separately covered by the hook tests.

## Observed run — 2026-09-21

The parent Codex session ran the documented shell blocks through the repository
wrapper with real API calls. `jev-latest` resolved to `jev-1.13.0`. Five requests
used 2,641 input and 254 output tokens; results below are observations, not
thresholds or guarantees.

| Case | Observed answer | Decision impact |
| --- | --- | --- |
| Search, retention query | Existence 0.03; access relevance 1, billing 0 on a 0–2 rubric | Read access first but seek retention policy; neither excerpt supplies a duration. |
| Debug, initial evidence | `cache`, confidence 0.97 | Prioritize disabling cache in the affected profile. |
| Debug, changed evidence | `insufficient`, confidence 0.92 | Drop the supplied hypotheses as complete explanations; propose an override removal/restoration experiment. |
| Planning | `worker` for both preferences | Prefer the existing worker under stated assumptions; technical checks still required. |
| Review | `contradicts` | Correct the claim against the full policy passage; verify provenance separately. |

A separate fresh Codex agent session then read the updated guidance and performed
three more real calls (1,660 input / 137 output tokens, same model):

| Session case | Observed behavior and next action |
| --- | --- |
| No relevant search match | Existence 0.01 and both Scores 0. Rejected both excerpts and ran a targeted local search for telescope/Saturn/astronomy; no matches. Reported the corpus gap rather than inventing an answer. |
| Debug with new evidence | Selected cache first, then `insufficient` (confidence 0.88). Changed the proposed experiment to isolating the saved label override; did not claim the synthetic bug was reproduced. |
| Unchanged planning evidence | Read saved state, questions, and answers. Reused both worker judgments with zero new planning API calls; retained capacity/retry/requirements checks as next steps. |
| Missing credentials | Actual wrapper exit 1, `jev: TYPESAFE_API_KEY unset`, empty stdout. Compared observations manually and chose the override experiment with explicit reasons, without inventing a result. |

One earlier design-comparison call chose preserving the old initial-call demand
with confidence 0.56, contrary to the user's explicit plan. The implementing
agent rejected that recommendation after inspecting the conflict. This is also
evidence for keeping user requirements and technical verification authoritative.

Limits: these were real agent/API sessions over controlled synthetic evidence,
not installed-host end-to-end sessions or production accuracy measurements. The
browser experiment and worker capacity checks were proposed, not executed.

## Repository verification

The 25 Jev hook tests passed locally, including host paths, prerequisite failures,
confirmation suppression, and JSON output. The full `bun run test` passed in an
Ubuntu 24.04 ARM container with GNU awk: 1,841 passed and 99 skipped
(including missing Cargo, platform-specific checks, and opt-in live checks). All Jev hook
and offline wrapper tests ran; live scenarios were evaluated separately above. Shellcheck and context-budget checks passed. A fresh-context review
reported no blocking findings.

The native macOS full run was stopped after script-launch delays reproduced on
a trivial temporary script. A minimal Debian container lacked CI tools and
failed loopback transport checks; an Ubuntu run using mawk hit a regex compiler
panic in unchanged debug-log tests. Matching the test tools and using GNU awk
resolved those environment failures without production-code changes.

## Prompt-size audit — 2026-09-21

Compared with commit `7c5915e`, the skill + shared workflow decreased from
1,908 to 762 whitespace-delimited words (13,063 to 5,841 UTF-8 bytes).
These are size measurements, not tokenizer counts. Stage prompts link to the
shared rules; transport details and evaluation records load only on demand.
Examples explicitly load Setup + the relevant section. All six executable
Bash blocks remain byte-identical to the previously evaluated examples.

A fresh agent loaded only the compact skill and Setup + Debugging. Live calls
through the byte-identical wrapper in the sibling checkout selected `cache`,
then `insufficient` after changed observations; next experiment shifted to the
saved override. The isolated checkout call did not complete and was terminated;
no answer was inferred. Missing-key invocation returned exit 1. The agent
recognized unchanged-input reuse and retained explicit manual fallback. This
checks prompt usability, not installed-host behavior or production accuracy.
