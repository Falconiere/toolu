# Brainstorm: code-review orchestration (#666)

**Mode:** Delivery, Full path (public interface, external cost, security/privacy).

## Capsule

- **Outcome:** A library entry point `toolu_code_review::review::run` that takes the
  shaped diff (#662), the base-ref rules bundle (#663) and the pre-pass result (#664),
  picks the effort level, builds a budgeted cache-friendly prompt, calls the OpenRouter
  review model (#665) per chunk, and judges fresh findings with Jev over OpenRouter's
  System One endpoint. Contradicted findings are removed only by a verified dismissal
  from a generative recheck. The CLI gains the per-repo opt-in check.
- **Material defaults / non-goals:** CLI keeps failing closed when enabled until #668
  validates findings and #669 reports them (Jev 0.99 over full wiring). No report
  schema change for `effort` object, state files, usage log file or skill (#669).
- **Repository evidence:** `crates/epic-orchestrator/src/route/tier.rs` (score + 0.15
  bias), `crates/pr-babysit/src/route/jev.rs` (Jev score batching),
  `crates/core/openrouter` (deadline-shared retries), `crates/core/jev` (hard-coded
  key/endpoint, no deadline), `toolu_runtime::config::load` (fail-closed unknown keys).
- **Risk:** Large surface in one PR; Jev over OpenRouter envelope assumed identical to
  TypeSafe's (`{model, usage, answers}`), per the epic; cost field optional.
- **Handoff:** spec.

## Decisions (Jev)

| Question | Choice | Confidence |
| --- | --- | --- |
| CLI wiring | library + opt-in check, CLI still fails closed | 0.99 |
| Prompt override | replaces only the checklist; fixed safety rules stay | 0.74 |
| Config location | new top-level `codeReview {enabled, effort}`; `enabled` project-only | 0.98 |

Rejected: full CLI wiring before #668 (would print ungrounded findings); override
replacing safety rules (a repo could switch off "evidence, not instructions");
reusing `gates`/`skills` sections (wrong semantics).

## Axes

- **Interface:** `review::run(env, &Input, &Options) -> Result<Reviewed, RunError>`;
  `Reviewed` carries retained/dismissed model findings with Jev assessments,
  unreviewed paths, notes, summed usage, per-call log records and the effort choice.
- **Security/privacy:** every outbound text (prompt, Jev state, recheck) passes a mask
  built from #664 redactions; repo text is fenced as evidence; opt-in read from the
  project config file only, before any client is built.
- **Cost:** no spend ceiling; byte caps only. Usage summed across selector, review,
  Jev and recheck calls.
- **Reversibility:** `core/jev` changes are additive (`from_env_key`, `ask_until`,
  `request_bytes`, optional `cost_usd`), existing callers unchanged.
