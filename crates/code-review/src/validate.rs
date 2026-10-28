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

use round::{build_recap, dedupe, generated_id, match_prior};

use crate::contract::{Finding, Omitted, Severity, Side, Source};
use crate::memory::{Memory, Status};
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
  let mut out = Validated {
    findings: Vec::new(),
    unreviewed: Vec::new(),
    recap: Recap::default(),
    dropped: 0,
  };
  let mut lost: BTreeMap<String, String> = BTreeMap::new();
  let mut candidates: Vec<Candidate> = deterministic
    .iter()
    .cloned()
    .map(|finding| Candidate {
      finding,
      prior: None,
    })
    .collect();
  for finding in model {
    if withdrawn(finding) {
      continue;
    }
    if bad.contains(&finding.path) || mask.is_none() {
      drop_lost(
        &mut out,
        &mut lost,
        finding,
        "its secret spans cannot be masked",
      );
      continue;
    }
    let Some(candidate) = ground(finding, &anchors, mask.as_ref(), memory, shaped, redactions)
    else {
      let reason = if anchored(finding, &anchors) {
        "its quote does not match the cited changed lines"
      } else {
        "it cites no changed line on the cited side"
      };
      drop_lost(&mut out, &mut lost, finding, reason);
      continue;
    };
    if let Ground::Suppressed = candidate {
      continue;
    }
    let Ground::Kept(candidate) = candidate else {
      continue;
    };
    candidates.push(candidate);
  }
  let kept = dedupe(candidates);
  build_recap(&kept, shaped, memory, &lost, &mut out.recap);
  out.findings = kept
    .into_iter()
    .map(|candidate| candidate.finding)
    .collect();
  out.unreviewed = lost
    .into_iter()
    .map(|(path, reason)| Omitted { path, reason })
    .collect();
  out
}

enum Ground {
  Kept(Candidate),
  Suppressed,
}

pub(super) struct Candidate {
  pub(super) finding: Finding,
  pub(super) prior: Option<usize>,
}

fn drop_lost(
  out: &mut Validated,
  lost: &mut BTreeMap<String, String>,
  finding: &ModelFinding,
  why: &str,
) {
  out.dropped += 1;
  lost.entry(finding.path.clone()).or_insert_with(|| {
    format!(
      "{}:{}..{} ({}) dropped: {why}; this coverage is unreviewed",
      finding.path,
      finding.line,
      finding.end_line,
      side_name(finding.side)
    )
  });
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

fn ground(
  finding: &ModelFinding,
  anchors: &Anchors,
  mask: Option<&Mask>,
  memory: &Memory,
  shaped: &Shaped,
  redactions: &[Redaction],
) -> Option<Ground> {
  let text = cited(finding, anchors)?;
  if normalize_text(&finding.quote) != normalize_text(&text) {
    return None;
  }
  let mask = mask?;
  let matched = match_prior(finding, memory, shaped);
  if let Some(index) = matched
    && memory.prior[index].status == Status::Dismissed
    && (!finding.severity.blocking()
      || finding.prior_id.as_deref() == Some(memory.prior[index].id.as_str()))
  {
    return Some(Ground::Suppressed);
  }
  let mut severity = finding.severity;
  if let Some(index) = matched
    && memory.prior[index].status == Status::Open
  {
    severity = severity.max(memory.prior[index].severity);
  }
  let id = matched.map_or_else(
    || {
      generated_id(&fingerprint(
        &finding.path,
        &finding.category,
        &finding.claim,
      ))
    },
    |index| memory.prior[index].id.clone(),
  );
  let suggestion = finding
    .suggestion
    .as_deref()
    .map(|text| mask.apply(text))
    .filter(|text| is_code(text));
  Some(Ground::Kept(Candidate {
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
      redaction_spans: redactions
        .iter()
        .filter(|redaction| {
          redaction.path == finding.path
            && redaction.side == finding.side
            && (finding.line..=finding.end_line).contains(&redaction.line)
        })
        .flat_map(|redaction| redaction.spans.iter().copied())
        .collect(),
    },
    prior: matched,
  }))
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

/// Paths whose secret spans cannot be located in the shaped lines: their
/// model findings may not be sent or stored unmasked.
fn bad_redaction_paths(shaped: &Shaped, redactions: &[Redaction]) -> BTreeSet<String> {
  redactions
    .iter()
    .filter(|redaction| {
      let line = shaped
        .files
        .iter()
        .filter(|file| file.path == redaction.path)
        .flat_map(|file| &file.lines)
        .find(|line| line.side == redaction.side && line.line == redaction.line);
      line.is_none_or(|line| {
        redaction.spans.iter().any(|span| {
          line
            .text
            .get(span.start..span.end)
            .is_none_or(str::is_empty)
        })
      })
    })
    .map(|redaction| redaction.path.clone())
    .collect()
}

#[cfg(test)]
#[path = "tests/validate_test.rs"]
mod tests;
