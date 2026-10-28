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

use round::{bad_redaction_paths, build_recap, dedupe, generated_id, match_prior};

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

/// Text as fingerprints and quote comparisons see it: lowercased, with every
/// whitespace run collapsed to one space and the ends trimmed.
pub fn normalize_text(text: &str) -> String {
  text
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase()
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

type Anchors = HashMap<(String, bool, u32), String>;

/// Ground `model` findings in `shaped`, merge the trusted `deterministic`
/// findings, and match both against `memory` for the round recap.
/// `redactions` are #664's secret spans over the shaped lines.
pub fn validate(
  shaped: &Shaped,
  model: &[ModelFinding],
  deterministic: &[Finding],
  memory: &Memory,
  redactions: &[Redaction],
) -> Validated {
  let anchors = anchors(shaped);
  let bad = bad_redaction_paths(shaped, redactions);
  let maskable: Vec<Redaction> = redactions
    .iter()
    .filter(|redaction| !bad.contains(&redaction.path))
    .cloned()
    .collect();
  let mask = Mask::new(shaped, &maskable).ok();
  let ctx = Ctx {
    anchors: &anchors,
    mask: mask.as_ref(),
    memory,
    shaped,
    redactions,
    bad: &bad,
  };
  let (kept, dropped, lost) = collect(&ctx, model, deterministic);
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
  build_recap(&kept, shaped, memory, &lost, &mut out.recap);
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
        let reason = if anchored(finding, ctx.anchors) {
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
  anchors: &'a Anchors,
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

fn side_name(side: Side) -> &'static str {
  match side {
    Side::New => "new",
    Side::Old => "old",
  }
}

fn anchors(shaped: &Shaped) -> Anchors {
  shaped
    .files
    .iter()
    .flat_map(|file| {
      file.lines.iter().map(move |line| {
        (
          (file.path.clone(), line.side == Side::Old, line.line),
          line.text.clone(),
        )
      })
    })
    .collect()
}

fn anchored(finding: &ModelFinding, anchors: &Anchors) -> bool {
  finding.line > 0
    && finding.end_line >= finding.line
    && (finding.line..=finding.end_line)
      .all(|line| anchors.contains_key(&(finding.path.clone(), finding.side == Side::Old, line)))
}

/// The cited lines' text, or `None` when any line in the range is not a
/// changed line on the cited side.
fn cited(finding: &ModelFinding, anchors: &Anchors) -> Option<String> {
  if finding.line == 0 || finding.end_line < finding.line {
    return None;
  }
  let mut texts = Vec::new();
  for line in finding.line..=finding.end_line {
    texts.push(
      anchors
        .get(&(finding.path.clone(), finding.side == Side::Old, line))?
        .clone(),
    );
  }
  Some(texts.join("\n"))
}

fn withdrawn(finding: &ModelFinding) -> bool {
  let category = normalize_text(&finding.category);
  if matches!(category.as_str(), "no_defect" | "no-defect" | "nodefect") {
    return true;
  }
  let claim = normalize_text(&finding.claim);
  claim == "no defect"
    || claim == "abstain"
    || claim.starts_with("no defect ")
    || claim.starts_with("no defect,")
    || claim.starts_with("abstain ")
    || claim.starts_with("abstain,")
}

fn ground(ctx: &Ctx<'_>, finding: &ModelFinding) -> Option<Ground> {
  let text = cited(finding, ctx.anchors)?;
  if normalize_text(&finding.quote) != normalize_text(&text) {
    return None;
  }
  let mask = ctx.mask?;
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
  let id = matched.map_or_else(
    || {
      generated_id(&fingerprint(
        &finding.path,
        &finding.category,
        &finding.claim,
      ))
    },
    |prior| prior.id.clone(),
  );
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
      quote: mask.apply(&finding.quote),
      suggestion,
      redaction_spans: spans(ctx, finding),
    },
    prior: matched.cloned(),
  })))
}

fn spans(ctx: &Ctx<'_>, finding: &ModelFinding) -> Vec<RedactionSpan> {
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

/// A suggestion is literal replacement code, not prose: prose has no code
/// punctuation and at least three words.
fn is_code(suggestion: &str) -> bool {
  let text = suggestion.trim();
  if text.is_empty() {
    return false;
  }
  text.chars().any(|ch| "(){}[];=`<>".contains(ch)) || text.split_whitespace().count() < 3
}

#[cfg(test)]
#[path = "tests/validate_test.rs"]
mod tests;
