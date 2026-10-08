# Fixed GitHub checks for the epic engine

## Outcome

The resident engine checks registered epics and watched pull requests every 180 seconds, notices verdicts, CI, reviews, base movement, and graph changes without an agent turn, and reports separate REST and GraphQL costs.

## Decisions and evidence

- Keep one GitHub detector interface feeding state-machine inputs. The existing `toolu-github` client supplies conditional REST GETs, a scheduled one-attempt policy, and GraphQL cost. The epic engine already persists `watch.json` and has a state-thread timer.
- Persist an absolute scheduled deadline for each watch. Advance it from the previous scheduled deadline after a check. Worker reports and engine merges may request immediate checks without moving it. A virtual-clock test will check six hours of 180-second gaps. Jev ranked this integration above counting the engine's 30-second maintenance ticks (0.99 vs 0.01), consistent with the issue's restart and immediate-check requirements.
- Every babysit check executes the full GraphQL tick because review-thread resolution cannot be detected with the REST endpoints. Conditional REST probes remain useful for detecting other inputs and measuring idle cost. Keep REST and GraphQL budgets separate.
- A `retry-after` response suppresses requests until its deadline. The cadence remains anchored to 180-second slots; the next eligible slot resumes checking.
- Only registered epics are in scope. No webhook, tunnel, App, or public listener is introduced.

## Alternatives and risks

Counting every sixth maintenance tick would let work and restarts drift the schedule. Skipping GraphQL on unchanged REST would miss resolved review threads. The principal risks are incomplete watch recovery, API cost growth across several PRs, and an HTTP failure interfering with the local engine. Tests will use a loopback HTTPS GitHub fixture and a virtual clock, plus a live sandbox watch required by #447.

## Handoff

Specify persisted watches, detector inputs, failure handling, cost accounting, and observable acceptance tests before writing code.
