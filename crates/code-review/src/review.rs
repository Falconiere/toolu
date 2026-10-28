//! Pipeline stage, owned by #666: wires the shaped diff (#662), the base-ref rules
//! (#663) and the pre-pass (#664) into the structured review call (#665), then
//! judges fresh findings with Jev over `OpenRouter`'s System One endpoint. Nothing is
//! sent when review is disabled or a key is absent; every outbound text is
//! masked. The output is judged model findings for #668 to ground and anchor.

mod calls;
mod chunks;
mod effort;
mod judge;
pub(crate) mod mask;
mod options;
mod prompt;
mod recheck;
mod replayed;
mod source;

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_jev_client::{Answer, Jev, Question, Questions, State};
use toolu_openrouter::Client;
use toolu_runtime::env::Env;

pub use calls::{Call, Record, Stage};
pub use effort::{EffortChoice, EffortSource};
pub use options::{Input, Options};
pub(crate) use replayed::REPLAYED;
pub use replayed::Replay;

use crate::budget::{Budget, Effort, Judge};
use crate::contract::{ErrorKind, Omitted, Report, Usage};
use crate::memory::{Memory, Status};
use crate::model::ModelFinding;
use crate::settings::ENABLE_HINT;
use calls::Ledger;
use mask::Mask;
use source::Package;

/// The key both the review model and Jev use.
const KEY_VAR: &str = "OPENROUTER_API_KEY";
/// The effort selector's own wall deadline.
const SELECTOR_DEADLINE: Duration = Duration::from_secs(60);

/// How Jev assessed a model finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assessment {
  /// The evidence shows the defect.
  Supported,
  /// Jev found the claim contradicted; no valid dismissal followed, so it stays.
  Contradicted,
  /// Neither, or Jev was uncertain.
  Insufficient,
  /// Jev could not judge it: failure, deadline, or evidence over the cap.
  Unavailable,
  /// Not sent to Jev: a re-raised prior finding, below the level's judge depth,
  /// or the Jev stage was switched off.
  NotJudged,
}

/// A retained model finding and its assessment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
  /// The model's finding, before #668 grounds it.
  pub finding: ModelFinding,
  /// Jev's assessment.
  pub assessment: Assessment,
}

/// A finding removed by a verified dismissal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dismissed {
  /// The model's finding.
  pub finding: ModelFinding,
  /// Why the recheck dismissed it.
  pub explanation: String,
  /// The evidence lines that prove it.
  pub quotes: Vec<String>,
}

/// The result of a run that reviewed at least one chunk, or had none to review.
#[derive(Debug, Clone, PartialEq)]
pub struct Reviewed {
  /// The level and why.
  pub effort: EffortChoice,
  /// Retained model findings.
  pub findings: Vec<Judged>,
  /// Findings removed by verified dismissals.
  pub dismissed: Vec<Dismissed>,
  /// Paths whose review call failed or did not fit; never a pass.
  pub unreviewed: Vec<Omitted>,
  /// What was degraded or left out, one line each.
  pub notes: Vec<String>,
  /// Summed over every call.
  pub usage: Usage,
  /// Every call, for the usage log.
  pub calls: Vec<Call>,
}

/// Why a run produced no review.
#[derive(Debug, Clone, PartialEq)]
pub enum RunError {
  /// Review is disabled by configuration; nothing was sent.
  NotEnabled,
  /// No usable key, or an unusable configuration; nothing was sent.
  Unconfigured(String),
  /// Every review call failed, or the secrets could not be masked.
  Outage {
    /// What failed.
    message: String,
    /// What the failed calls cost.
    usage: Usage,
  },
  /// Every review call ran out of its deadline.
  Deadline {
    /// What the calls cost.
    usage: Usage,
  },
}

impl RunError {
  /// The report a failed run prints: verdict `error`, exit 69.
  pub fn report(&self, effort: Effort) -> Report {
    let (kind, message, usage) = match self {
      RunError::NotEnabled => (ErrorKind::NotEnabled, ENABLE_HINT.to_owned(), None),
      RunError::Unconfigured(why) => (
        ErrorKind::Unconfigured,
        format!("{why}; nothing was sent"),
        None,
      ),
      RunError::Outage { message, usage } => (ErrorKind::Outage, message.clone(), Some(usage)),
      RunError::Deadline { usage } => (
        ErrorKind::Deadline,
        "every review call ran out of its deadline; not retried at a lower level".to_owned(),
        Some(usage),
      ),
    };
    let mut report = Report::failed(effort, kind, &message);
    report.usage = usage.cloned().unwrap_or_default();
    report
  }
}

/// What the judge and the recheck share: one wall deadline for every call.
pub(crate) struct Ctx<'a, 'r> {
  pub(crate) client: &'a Client,
  pub(crate) jev: &'a Jev,
  pub(crate) ledger: Ledger<'r>,
  pub(crate) deadline: Instant,
  pub(crate) notes: Vec<String>,
  pub(crate) budget: &'static Budget,
  pub(crate) mask: &'a Mask,
  pub(crate) memory: &'a Memory,
}

/// Review `input`. Enablement and the key are checked before anything is sent.
///
/// # Errors
/// `NotEnabled` or `Unconfigured` before any request; `Outage` or `Deadline`
/// when no review call succeeded.
pub fn run(env: &Env, input: &Input<'_>, options: &Options<'_>) -> Result<Reviewed, RunError> {
  if !input.settings.enabled {
    return Err(RunError::NotEnabled);
  }
  let (client, jev) = clients(env, options)?;
  let (mask, schema) = local(input)?;
  let mut ledger = Ledger::new(options.recorder)
    .replaying(options.replay)
    .live_on_miss(options.live_on_miss);
  let effort = select(input, options, &jev, &mut ledger);
  let budget = effort.level.budget();
  let (packages, mut notes) = source::packages(env, input.root, input.shaped, budget, &mask);
  notes.extend(input.settings.notes.iter().cloned());
  let deadline = options
    .review_deadline
    .unwrap_or(Duration::from_secs(budget.deadline_secs));
  let call = chunks::Call {
    client: &client,
    schema: &schema,
    level: effort.level,
    deadline,
  };
  let reviews = chunks::review(input, &packages, &mask, &call, &mut ledger);
  if let Some(err) = reviews.error(input.shaped.chunks.len(), &ledger, client.model()) {
    return Err(err);
  }
  notes.extend(reviews.notes);
  let judge_deadline = options
    .judge_deadline
    .unwrap_or(Duration::from_secs(budget.judge_deadline_secs));
  let ctx = Ctx {
    client: &client,
    jev: &jev,
    ledger,
    deadline: Instant::now() + judge_deadline,
    notes,
    budget,
    mask: &mask,
    memory: input.memory,
  };
  let partial = (effort, reviews.unreviewed);
  Ok(finish(
    partial,
    reviews.findings,
    &packages,
    !options.skip_judge,
    ctx,
  ))
}

/// The mask and the review schema; failing either sends nothing.
fn local(input: &Input<'_>) -> Result<(Mask, Value), RunError> {
  let outage = |message: String| RunError::Outage {
    message,
    usage: Usage::default(),
  };
  let mask = Mask::new(input.shaped, &input.prepass.redactions).map_err(outage)?;
  let schema = serde_json::from_str(crate::model::SCHEMA_JSON)
    .map_err(|err| outage(format!("review schema: {err}")))?;
  Ok((mask, schema))
}

/// Judge the findings unless the stage is off, then account for every call.
fn finish(
  (effort, unreviewed): (EffortChoice, Vec<Omitted>),
  findings: Vec<ModelFinding>,
  packages: &[Package],
  judge: bool,
  mut ctx: Ctx<'_, '_>,
) -> Reviewed {
  let mut reviewed = Reviewed {
    effort,
    findings: Vec::new(),
    dismissed: Vec::new(),
    unreviewed,
    notes: Vec::new(),
    usage: Usage::default(),
    calls: Vec::new(),
  };
  reviewed.findings = findings
    .into_iter()
    .map(|finding| Judged {
      finding,
      assessment: Assessment::NotJudged,
    })
    .collect();
  if judge {
    let memory = ctx.memory;
    reviewed.dismissed = judge_and_recheck(&mut reviewed.findings, packages, memory, &mut ctx);
  }
  reviewed.usage = ctx.ledger.usage(Some(ctx.client.model().to_owned()));
  reviewed.notes = ctx.notes;
  reviewed.calls = ctx.ledger.calls;
  reviewed
}

/// Both clients, from the one `OpenRouter` key; refused before any request.
fn clients(env: &Env, options: &Options<'_>) -> Result<(Client, Jev), RunError> {
  let unconfigured = |err: &dyn std::fmt::Display| RunError::Unconfigured(err.to_string());
  let client =
    Client::from_env(env, options.openrouter.clone()).map_err(|err| unconfigured(&err))?;
  let config = options
    .jev
    .clone()
    .unwrap_or_else(|| toolu_jev_client::Config {
      endpoint: toolu_jev_client::OPENROUTER_ENDPOINT.to_owned(),
      ..toolu_jev_client::Config::default()
    });
  let jev = Jev::from_env_key(env, KEY_VAR, config).map_err(|err| unconfigured(&err))?;
  Ok((client, jev))
}

/// Judge the fresh findings Jev may see at this level, then recheck the
/// contradicted ones unless the level judges blocking findings only.
fn judge_and_recheck(
  judged: &mut Vec<Judged>,
  packages: &[Package],
  memory: &Memory,
  ctx: &mut Ctx<'_, '_>,
) -> Vec<Dismissed> {
  let depth = ctx.budget.judge;
  let open = |id: &str| {
    memory
      .prior
      .iter()
      .any(|prior| prior.id == id && prior.status == Status::Open)
  };
  let targets: Vec<usize> = (0..judged.len())
    .filter(|index| {
      judged.get(*index).is_some_and(|item| {
        let fresh = item.finding.prior_id.as_deref().is_none_or(|id| !open(id));
        fresh && (depth != Judge::BlockingOnly || item.finding.severity.blocking())
      })
    })
    .collect();
  judge::assess(judged, &targets, packages, ctx);
  if depth == Judge::BlockingOnly {
    return Vec::new();
  }
  recheck::recheck(judged, packages, ctx)
}

/// The run's effort: flag, then config, else Jev under `auto` with its floors.
fn select(
  input: &Input<'_>,
  options: &Options<'_>,
  jev: &Jev,
  ledger: &mut Ledger<'_>,
) -> EffortChoice {
  let explicit = input.flag.map(|level| (level, EffortSource::Flag)).or(
    input
      .settings
      .effort
      .map(|level| (level, EffortSource::Config)),
  );
  let floor = effort::floor(input.shaped, input.prepass);
  effort::choose(explicit, floor, || {
    if options.skip_judge {
      return Err("the Jev stage is off".to_owned());
    }
    if input.shaped.chunks.is_empty() {
      return Err("nothing to review".to_owned());
    }
    let state = State::structured(&effort::metadata(input.shaped, input.prepass).to_string());
    let question =
      Question::score(effort::QUESTION, &effort::LEVELS).map_err(|err| err.to_string())?;
    let reply = ledger
      .jev(
        Stage::Selector,
        jev,
        (&state, &Questions::single("effort", question)),
        Instant::now() + SELECTOR_DEADLINE,
      )
      .map_err(|err| calls::jev_failure(&err))?;
    match reply.answers.first() {
      Some((_, Answer::Score { score, .. })) => Ok(*score),
      Some(_) | None => Err("invalid answer".to_owned()),
    }
  })
}

/// A finding as the judge and the recheck show it.
pub(crate) fn shown(finding: &ModelFinding) -> Value {
  json!({"path": finding.path, "line": finding.line, "end_line": finding.end_line,
    "side": finding.side, "severity": finding.severity, "category": finding.category,
    "claim": finding.claim, "quote": finding.quote})
}

#[cfg(test)]
#[path = "tests/review_test.rs"]
mod tests;
