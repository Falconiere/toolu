//! Cross-round matching, dedupe, and the round recap for `validate`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::contract::Source;
use crate::memory::{Memory, Status};
use crate::model::ModelFinding;
use crate::shape::Shaped;

use super::{Candidate, Recap, SeverityChange, fingerprint};

/// Match by echoed `prior_id` first, then by fingerprint through the rename
/// map; fingerprint ties prefer an open prior, then a dismissed one.
pub(super) fn match_prior(
  finding: &ModelFinding,
  memory: &Memory,
  shaped: &Shaped,
) -> Option<usize> {
  if let Some(id) = &finding.prior_id
    && let Some(index) = memory.prior.iter().position(|prior| &prior.id == id)
  {
    return Some(index);
  }
  let wanted = fingerprint(&finding.path, &finding.category, &finding.claim);
  memory
    .prior
    .iter()
    .enumerate()
    .filter(|(_, prior)| {
      fingerprint(
        &mapped_path(&prior.path, shaped),
        &prior.category,
        &prior.claim,
      ) == wanted
    })
    .min_by_key(|(_, prior)| match prior.status {
      Status::Open => 0,
      Status::Dismissed => 1,
      Status::Resolved => 2,
    })
    .map(|(index, _)| index)
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
        let current = &kept[slot].finding;
        let wins = candidate.finding.severity > current.severity
          || (candidate.finding.severity == current.severity
            && current.source == Source::Llm
            && candidate.finding.source != Source::Llm);
        if wins {
          kept[slot] = candidate;
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
  let mut re_raised: BTreeSet<usize> = BTreeSet::new();
  for candidate in kept
    .iter()
    .filter(|item| item.finding.source == Source::Llm)
  {
    match candidate.prior {
      None => recap.new.push(candidate.finding.id.clone()),
      Some(index) => {
        let prior = &memory.prior[index];
        if prior.status == Status::Open {
          re_raised.insert(index);
          if candidate.finding.severity != prior.severity {
            recap.severity_changes.push(SeverityChange {
              id: prior.id.clone(),
              from: prior.severity,
              to: candidate.finding.severity,
            });
          }
        } else {
          recap.new.push(candidate.finding.id.clone());
        }
      }
    }
  }
  for (index, prior) in memory.prior.iter().enumerate() {
    if prior.status != Status::Open {
      continue;
    }
    let path = mapped_path(&prior.path, shaped);
    let reexamined = shaped.coverage.reviewed.contains(&path) && !lost.contains_key(&path);
    if re_raised.contains(&index) || !reexamined {
      recap.still_open.push(prior.id.clone());
    } else {
      recap.resolved.push(prior.id.clone());
    }
  }
}

#[cfg(test)]
#[path = "../tests/validate_round_test.rs"]
mod tests;
