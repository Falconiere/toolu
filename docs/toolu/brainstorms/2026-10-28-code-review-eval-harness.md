# Code-review paired-comparison eval harness

**Outcome:** A rerunnable Rust harness proves review quality against committed
expectations by comparing a baseline arm (`skip_judge`) to a Jev-enhanced arm
over pinned diffs, with offline matcher self-tests in the gate and explicit
live/replay runs outside it.

**Material defaults / non-goal:** Primary entry is `toolu code-review eval`.
Fixtures and replay blobs live under `fixtures/code-review-eval/`. Quality is
judged only by expectation match rates, never by finding count or tokens.
Non-goals: GitHub posting, changing the production review path beyond harness
hooks already shipped in #666, spend ceilings, or a new JS runtime.

**Repository evidence:** #666 `Options.skip_judge` and `Options.recorder` /
`Record` are explicitly reserved for #670. Epic #660 and issue #670 fix the
expectation shape, replay rule, offline-gate rule, memory cases, and effort
calibration. Language-neutral JSON under `fixtures/` and offline
`cargo xtask bench` are the precedent patterns.

**Risk:** Live provider variance and routing can miss expectations even with
pinned model/provider constraints; replay must be the default offline path.
Matcher false negatives (claim phrasing drift) need distinctive expectation
text. Large replay blobs must stay out of the gate's critical path except as
committed offline fixtures.

**Handoff:** `spec`.

## Axes

| Axis | Default | Evidence | Risk |
| --- | --- | --- | --- |
| Intent | Paired eval over pinned diffs + expectations; report coverage/usage/cost/runtime; live only on explicit command | #670 ACs; gate must stay offline | Live cost/latency if someone runs eval without replay |
| Interface | `toolu code-review eval` + library used by black-box tests | Jev `cli_eval` 0.87; additive CLI; #666 hooks | CLI contract snapshot + docs-cli churn |
| Data | `fixtures/code-review-eval/` expectations, diffs, replay store | Jev `repo_fixtures` 0.62; `fixtures/` precedent | Blob size; prefer crate fixtures if blobs bloat CI checkout |
| Baseline | Same model path, `skip_judge=true` | #666 comment; Jev `skip_judge` 1.00 | Baseline still costs review-model tokens unless replayed |
| Replay key | SHA-256 of masked request body + stage | #670 replay store; Jev `sha256_request` 0.76 | Hash instability if request serialization drifts |
| Failure | Missing key → blocked/unverified, never green skip; unmatched findings unclassified | Epic config-vs-outage + #670 | Operators mistaking unverified for pass |
| Horizon | Ship matcher + golden offline set + eval CLI with replay; live calibration documented as optional explicit run | Effort/reasoning calibration in #670 | Full live matrix may be deferred to recorded replay after one calibration run |

## Alternatives rejected

- **xtask-only / tests-only entry:** weaker discoverability for paid runs; Jev
  preferred CLI.
- **Pre-pass-only or separate-model baseline:** contradicts #666's reserved
  `skip_judge` baseline arm.
- **case-id replay keys:** would charge again when request bytes change and
  would not prove request/response fidelity.

## Jev

- surface → `cli_eval` (0.87)
- fixture_home → `repo_fixtures` (0.62)
- baseline_arm → `skip_judge` (1.00)
- replay_key → `sha256_request` (0.76)
