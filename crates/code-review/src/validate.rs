//! Pipeline stage, owned by #668: grounds every finding in the diff, anchors it and drops what cannot be quoted.
//!
//! Model findings are checked against the shaped diff before anything is
//! reported: hallucinated anchors and mismatched quotes are dropped and the
//! lost coverage is returned as `unreviewed`, never as a clean pass. Withdrawn
//! findings vanish without a count, duplicates collapse to the highest
//! severity, and deterministic (`ast-grep`/`secrets`) findings pass through
//! exempt. Cross-round matching echoes `prior_id` first, then the
//! line-independent fingerprint through the rename map.

use std::collections::{BTreeMap, BTreeSet, HashMap};

mod round;

use round::{build_recap, dedupe, fresh_id, is_code, match_prior, withdrawn};

use crate::contract::{Finding, Omitted, RedactionSpan, Severity, Side, Source};
use crate::memory::{Memory, PriorFinding, Status};
use crate::model::ModelFinding;
use crate::prepass::Redaction;
use crate::review::mask::Mask;
use crate::shape::Shaped;

/// A severity change on a finding matched to an open prior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeverityChange {
  /// The finding's (prior) id.
  pub id: String,
  /// The prior round's severity.
  pub from: Severity,
  /// The severity kept this round: the highest of the two.
  pub to: Severity,
}

/// How this round's findings stand against earlier rounds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recap {
  /// Open priors re-examined this round and not re-raised.
  pub resolved: Vec<String>,
  /// Open priors re-raised, or not re-examined this round.
  pub still_open: Vec<String>,
  /// Survivors with no open prior: unmatched, resolved, or unsettled blockers.
  pub new: Vec<String>,
  /// Severity drift on matched open priors.
  pub severity_changes: Vec<SeverityChange>,
}

/// The grounded output #669 reports: nothing here is a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validated {
  /// Anchored, deduped findings (model and deterministic together).
  pub findings: Vec<Finding>,
  /// Coverage lost to grounding drops; each path blocks a clean pass.
  pub unreviewed: Vec<Omitted>,
  /// The round recap against `Memory`.
  pub recap: Recap,
  /// Grounding drops only: withdrawals and settled suppressions never count.
  pub dropped: usize,
}

/// Text as fingerprints see it: lowercased, with every whitespace run
/// collapsed to one space and the ends trimmed.
pub fn normalize_text(text: &str) -> String {
  collapse_whitespace(text).to_lowercase()
}

/// Text as quote comparisons see it: case kept, whitespace runs collapsed.
fn collapse_whitespace(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The line-independent memory fingerprint: `path`, category, normalized claim.
pub fn fingerprint(path: &str, category: &str, claim: &str) -> String {
  format!(
    "{}\u{1f}{}\u{1f}{}",
    path.trim(),
    normalize_text(category),
    normalize_text(claim)
  )
}

type Anchors<'a> = HashMap<(&'a str, bool, u32), &'a str>;

/// What one validation pass consumes: the stage outputs #669 assembles.
pub struct Input<'a> {
  /// The committed diff and its citable changed lines.
  pub shaped: &'a Shaped,
  /// Judged model findings to ground.
  pub model: &'a [ModelFinding],
  /// Trusted deterministic findings, exempt from quote checks.
  pub deterministic: &'a [Finding],
  /// Earlier rounds' findings and dismissals.
  pub memory: &'a Memory,
  /// #664's secret spans over the shaped lines.
  pub redactions: &'a [Redaction],
  /// The review stage's lost coverage: paths not re-examined this round.
  pub unreviewed: &'a [Omitted],
}

/// Ground `model` findings in `shaped`, merge the trusted `deterministic`
/// findings, and match both against `memory` for the round recap.
/// `redactions` are #664's secret spans over the shaped lines; `unreviewed`
/// is the review stage's lost coverage (failed or oversized review calls),
/// whose paths were not re-examined this round.
pub fn validate(input: &Input<'_>) -> Validated {
  let (shaped, memory) = (input.shaped, input.memory);
  let anchors = anchors(shaped);
  // A redaction `Mask` cannot locate makes its path's model findings unsendable.
  let (maskable, unmaskable): (Vec<Redaction>, Vec<Redaction>) = input
    .redactions
    .iter()
    .cloned()
    .partition(|redaction| Mask::new(shaped, std::slice::from_ref(redaction)).is_ok());
  let bad: BTreeSet<String> = unmaskable
    .into_iter()
    .map(|redaction| redaction.path)
    .collect();
  let mask = Mask::new(shaped, &maskable).ok();
  let ctx = Ctx {
    anchors: &anchors,
    mask: mask.as_ref(),
    memory,
    shaped,
    redactions: input.redactions,
    bad: &bad,
  };
  let (kept, dropped, lost) = collect(&ctx, input.model, input.deterministic);
  let mut out = Validated {
    findings: Vec::new(),
    unreviewed: lost
      .iter()
      .map(|(path, reason)| Omitted {
        path: path.clone(),
        reason: reason.clone(),
      })
      .collect(),
    recap: Recap::default(),
    dropped,
  };
  let blocked: BTreeSet<String> = lost
    .keys()
    .cloned()
    .chain(input.unreviewed.iter().map(|item| item.path.clone()))
    .collect();
  build_recap(&kept, shaped, memory, &blocked, &mut out.recap);
  out.findings = kept
    .into_iter()
    .map(|candidate| candidate.finding)
    .collect();
  out
}

fn collect(
  ctx: &Ctx<'_>,
  model: &[ModelFinding],
  deterministic: &[Finding],
) -> (Vec<Candidate>, usize, BTreeMap<String, String>) {
  let mut candidates: Vec<Candidate> = deterministic
    .iter()
    .cloned()
    .map(|finding| Candidate {
      finding,
      prior: None,
    })
    .collect();
  let mut dropped = 0;
  let mut lost: BTreeMap<String, String> = BTreeMap::new();
  for finding in model {
    if withdrawn(finding) {
      continue;
    }
    if ctx.bad.contains(&finding.path) || ctx.mask.is_none() {
      dropped += drop_lost(&mut lost, finding, "its secret spans cannot be masked");
      continue;
    }
    match ground(ctx, finding) {
      Some(Ground::Kept(candidate)) => candidates.push(*candidate),
      Some(Ground::Suppressed) => {}
      None => {
        let reason = if cited(finding, ctx.anchors).is_some() {
          "its quote does not match the cited changed lines"
        } else {
          "it cites no changed line on the cited side"
        };
        dropped += drop_lost(&mut lost, finding, reason);
      }
    }
  }
  (dedupe(candidates), dropped, lost)
}

enum Ground {
  Kept(Box<Candidate>),
  Suppressed,
}

pub(super) struct Candidate {
  pub(super) finding: Finding,
  pub(super) prior: Option<PriorFinding>,
}

/// What grounding one model finding needs.
struct Ctx<'a> {
  anchors: &'a Anchors<'a>,
  mask: Option<&'a Mask>,
  memory: &'a Memory,
  shaped: &'a Shaped,
  redactions: &'a [Redaction],
  bad: &'a BTreeSet<String>,
}

fn drop_lost(lost: &mut BTreeMap<String, String>, finding: &ModelFinding, why: &str) -> usize {
  lost.entry(finding.path.clone()).or_insert_with(|| {
    format!(
      "{}:{}..{} ({}) dropped: {why}; this coverage is unreviewed",
      finding.path,
      finding.line,
      finding.end_line,
      side_name(finding.side)
    )
  });
  1
}

pub(super) fn side_name(side: Side) -> &'static str {
  match side {
    Side::New => "new",
    Side::Old => "old",
  }
}

fn anchors(shaped: &Shaped) -> Anchors<'_> {
  shaped
    .files
    .iter()
    .flat_map(|file| {
      file.lines.iter().map(move |line| {
        (
          (file.path.as_str(), line.side == Side::Old, line.line),
          line.text.as_str(),
        )
      })
    })
    .collect()
}

/// The cited lines' text, or `None` when any line in the range is not a
/// changed line on the cited side.
fn cited(finding: &ModelFinding, anchors: &Anchors<'_>) -> Option<String> {
  if finding.line == 0 || finding.end_line < finding.line {
    return None;
  }
  let side = finding.side == Side::Old;
  (finding.line..=finding.end_line)
    .map(|line| anchors.get(&(finding.path.as_str(), side, line)).copied())
    .collect::<Option<Vec<_>>>()
    .map(|texts| texts.join("\n"))
}

fn ground(ctx: &Ctx<'_>, finding: &ModelFinding) -> Option<Ground> {
  let mask = ctx.mask?;
  // The model saw the masked patch, so both sides are compared masked.
  let text = mask.apply(&cited(finding, ctx.anchors)?);
  if collapse_whitespace(&mask.apply(&finding.quote)) != collapse_whitespace(&text) {
    return None;
  }
  if settled(ctx, finding) {
    return Some(Ground::Suppressed);
  }
  let matched = match_prior(finding, ctx.memory, ctx.shaped);
  if matched.is_some_and(|prior| {
    prior.status == Status::Dismissed
      && (!finding.severity.blocking() || finding.prior_id.as_deref() == Some(prior.id.as_str()))
  }) {
    return Some(Ground::Suppressed);
  }
  let severity = matched
    .filter(|prior| prior.status == Status::Open)
    .map_or(finding.severity, |prior| {
      finding.severity.max(prior.severity)
    });
  let id = match matched {
    Some(prior) if prior.status != Status::Dismissed => prior.id.clone(),
    _ => fresh_id(finding),
  };
  let suggestion = finding
    .suggestion
    .as_deref()
    .map(|text| mask.apply(text))
    .filter(|text| is_code(text));
  Some(Ground::Kept(Box::new(Candidate {
    finding: Finding {
      id,
      path: finding.path.clone(),
      line: finding.line,
      end_line: finding.end_line,
      side: finding.side,
      severity,
      category: finding.category.clone(),
      source: Source::Llm,
      claim: mask.apply(&finding.claim),
      quote: text,
      suggestion,
      redaction_spans: spans(ctx, finding),
    },
    prior: matched.cloned(),
  })))
}

/// A settled dismissal by exact id suppresses any severity: the echo is exact.
fn settled(ctx: &Ctx<'_>, finding: &ModelFinding) -> bool {
  finding.prior_id.as_deref().is_some_and(|id| {
    ctx
      .memory
      .dismissals
      .iter()
      .any(|dismissal| dismissal.id == id)
  })
}

fn spans(ctx: &Ctx<'_>, finding: &ModelFinding) -> Vec<RedactionSpan> {
  // Spans index one line (contract), so a multi-line finding carries none:
  // its quote is already masked, and merged spans would name no line.
  if finding.line != finding.end_line {
    return Vec::new();
  }
  ctx
    .redactions
    .iter()
    .filter(|redaction| {
      redaction.path == finding.path
        && redaction.side == finding.side
        && (finding.line..=finding.end_line).contains(&redaction.line)
    })
    .flat_map(|redaction| redaction.spans.iter().copied())
    .collect()
}

#[cfg(test)]
#[path = "tests/validate_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/validate_mask_test.rs"]
mod mask_tests;
