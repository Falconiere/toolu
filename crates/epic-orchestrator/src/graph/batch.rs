//! The ready issues that fit the free slots: the longest chain first, then the
//! most unblocked work, then the least-loaded repository, repository, number.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::GraphIssue;

/// Up to `slots` of `ready`, ordered as `pickBatch` orders them.
pub(crate) fn pick_batch(
  ready: &[String],
  issues: &[GraphIssue],
  in_flight: &[String],
  slots: usize,
) -> Vec<String> {
  let by_ref: BTreeMap<&str, &GraphIssue> = issues
    .iter()
    .map(|issue| (issue.reference.as_str(), issue))
    .collect();
  let mut load: BTreeMap<String, u64> = BTreeMap::new();
  for reference in in_flight {
    if let Some(issue) = by_ref.get(reference.as_str()) {
      *load.entry(issue.repo.clone()).or_default() += 1;
    }
  }
  let mut pool: Vec<&String> = ready.iter().collect();
  let mut batch = Vec::new();
  while !pool.is_empty() && batch.len() < slots {
    let key = |reference: &str| {
      by_ref
        .get(reference)
        .map_or((Reverse(0), Reverse(0), 0, String::new(), 0), |issue| {
          (
            Reverse(issue.chain),
            Reverse(issue.unblocks),
            load.get(&issue.repo).copied().unwrap_or(0),
            issue.repo.clone(),
            issue.number.unwrap_or(0),
          )
        })
    };
    let Some((best_index, _)) =
      pool
        .iter()
        .enumerate()
        .min_by(|(left_index, left), (right_index, right)| {
          key(left).cmp(&key(right)).then(left_index.cmp(right_index))
        })
    else {
      break;
    };
    let best = pool.remove(best_index);
    if let Some(issue) = by_ref.get(best.as_str()) {
      *load.entry(issue.repo.clone()).or_default() += 1;
    }
    batch.push(best.clone());
  }
  batch
}

#[cfg(test)]
#[path = "tests/batch_test.rs"]
mod tests;
