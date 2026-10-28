//! Cross-round matching, dedupe, and the round recap for `validate`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::contract::Source;
use crate::memory::{Memory, PriorFinding, Status};
use crate::model::ModelFinding;
use crate::prepass::Redaction;
use crate::shape::Shaped;

use super::{Candidate, Recap, SeverityChange, fingerprint};

/// Match by echoed `prior_id` first, then by fingerprint through the rename
/// map; fingerprint ties prefer an open prior, then a dismissed one.
pub(super) fn match_prior<'a>(
  finding: &ModelFinding,
  memory: &'a Memory,
  shaped: &Shaped,
) -> Option<&'a PriorFinding> {
  if let Some(id) = &finding.prior_id
    && let Some(prior) = memory.prior.iter().find(|prior| &prior.id == id)
  {
    return Some(prior);
  }
  let wanted = fingerprint(&finding.path, &finding.category, &finding.claim);
  memory
    .prior
    .iter()
    .filter(|prior| {
      fingerprint(
        &mapped_path(&prior.path, shaped),
        &prior.category,
        &prior.claim,
      ) == wanted
    })
    .min_by_key(|prior| match prior.status {
      Status::Open => 0,
      Status::Dismissed => 1,
      Status::Resolved => 2,
    })
}

pub(super) fn mapped_path(path: &str, shaped: &Shaped) -> String {
  shaped
    .renames
    .iter()
    .find(|rename| rename.old_path == path)
    .map_or_else(|| path.to_owned(), |rename| rename.path.clone())
}

pub(super) fn generated_id(fingerprint: &str) -> String {
  let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
  for byte in fingerprint.bytes() {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
  }
  format!("cr-{hash:016x}")
}

/// Collapse `(path, line, end_line, text-fingerprint)` duplicates: highest
/// severity survives, deterministic findings win ties.
pub(super) fn dedupe(candidates: Vec<Candidate>) -> Vec<Candidate> {
  let mut kept: Vec<Candidate> = Vec::new();
  let mut slots: HashMap<(String, u32, u32, String), usize> = HashMap::new();
  for candidate in candidates {
    let finding = &candidate.finding;
    let key = (
      finding.path.clone(),
      finding.line,
      finding.end_line,
      fingerprint(&finding.path, &finding.category, &finding.claim),
    );
    match slots.get(&key) {
      None => {
        slots.insert(key, kept.len());
        kept.push(candidate);
      }
      Some(&slot) => {
        let wins = kept.get(slot).is_some_and(|current| {
          candidate.finding.severity > current.finding.severity
            || (candidate.finding.severity == current.finding.severity
              && current.finding.source == Source::Llm
              && candidate.finding.source != Source::Llm)
        });
        if wins && let Some(current) = kept.get_mut(slot) {
          *current = candidate;
        }
      }
    }
  }
  kept
}

/// Fill `recap` from the surviving candidates and the priors they did not
/// re-raise: an open prior is resolved only when its path was re-examined
/// this round (reviewed, and not lost again to a grounding drop).
pub(super) fn build_recap(
  kept: &[Candidate],
  shaped: &Shaped,
  memory: &Memory,
  lost: &BTreeMap<String, String>,
  recap: &mut Recap,
) {
  let mut re_raised: BTreeSet<String> = BTreeSet::new();
  for candidate in kept
    .iter()
    .filter(|item| item.finding.source == Source::Llm)
  {
    note_survivor(candidate, recap, &mut re_raised);
  }
  for prior in memory
    .prior
    .iter()
    .filter(|prior| prior.status == Status::Open)
  {
    let path = mapped_path(&prior.path, shaped);
    let reexamined = shaped.coverage.reviewed.contains(&path) && !lost.contains_key(&path);
    if re_raised.contains(&prior.id) || !reexamined {
      recap.still_open.push(prior.id.clone());
    } else {
      recap.resolved.push(prior.id.clone());
    }
  }
}

fn note_survivor(candidate: &Candidate, recap: &mut Recap, re_raised: &mut BTreeSet<String>) {
  let Some(prior) = &candidate.prior else {
    recap.new.push(candidate.finding.id.clone());
    return;
  };
  if prior.status != Status::Open {
    recap.new.push(candidate.finding.id.clone());
    return;
  }
  re_raised.insert(prior.id.clone());
  if candidate.finding.severity != prior.severity {
    recap.severity_changes.push(SeverityChange {
      id: prior.id.clone(),
      from: prior.severity,
      to: candidate.finding.severity,
    });
  }
}

/// Paths whose secret spans cannot be located in the shaped lines: their
/// model findings may not be sent or stored unmasked.
pub(super) fn bad_redaction_paths(shaped: &Shaped, redactions: &[Redaction]) -> BTreeSet<String> {
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
#[path = "tests/round_test.rs"]
mod tests;
