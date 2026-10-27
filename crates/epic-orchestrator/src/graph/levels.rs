//! Waves, downstream counts and chain lengths over open issues.

use std::collections::{BTreeMap, BTreeSet};

use super::GraphIssue;

fn open_refs(issues: &[GraphIssue]) -> BTreeSet<&str> {
  issues
    .iter()
    .filter(|issue| issue.state == "open")
    .map(|issue| issue.reference.as_str())
    .collect()
}

/// Each open issue's open blockers in the epic.
fn open_deps<'a>(
  issues: &'a [GraphIssue],
  open: &BTreeSet<&str>,
) -> BTreeMap<&'a str, Vec<&'a str>> {
  issues
    .iter()
    .filter(|issue| open.contains(issue.reference.as_str()))
    .map(|issue| {
      let blockers = issue
        .blockers
        .iter()
        .filter(|(blocker, state)| *state == "open" && open.contains(blocker.as_str()))
        .map(|(blocker, _)| blocker.as_str())
        .collect();
      (issue.reference.as_str(), blockers)
    })
    .collect()
}

/// Each open issue's wave (1 + its deepest open blocker in the epic), or,
/// when the open issues hold a cycle, no waves and the stuck references sorted.
pub(crate) fn compute_levels(issues: &[GraphIssue]) -> (BTreeMap<String, u64>, Vec<String>) {
  let open = open_refs(issues);
  let deps = open_deps(issues, &open);
  let mut levels: BTreeMap<String, u64> = BTreeMap::new();
  let mut pending: BTreeSet<&str> = open;
  while !pending.is_empty() {
    let progress: Vec<&str> = pending
      .iter()
      .copied()
      .filter(|reference| {
        deps
          .get(reference)
          .is_some_and(|blockers| blockers.iter().all(|blocker| levels.contains_key(*blocker)))
      })
      .collect();
    if progress.is_empty() {
      return (
        BTreeMap::new(),
        pending.iter().map(|r| (*r).to_owned()).collect(),
      );
    }
    let next: Vec<(String, u64)> = progress
      .iter()
      .map(|reference| {
        let deepest = deps
          .get(reference)
          .into_iter()
          .flatten()
          .filter_map(|blocker| levels.get(*blocker).copied())
          .max()
          .unwrap_or(0);
        ((*reference).to_owned(), deepest + 1)
      })
      .collect();
    for (reference, level) in next {
      pending.remove(reference.as_str());
      levels.insert(reference, level);
    }
  }
  (levels, Vec::new())
}

/// Each graph issue's wave by key, over open and closed issues alike. The
/// graph levels only open issues, so its waves renumber after every merge;
/// these keep a merged issue in its wave for the whole run. A cycle through a
/// closed issue falls back to the open issues' waves; one among open issues
/// gives none.
pub(crate) fn stable_waves(rows: &[GraphIssue]) -> BTreeMap<String, u64> {
  let all: Vec<GraphIssue> = rows
    .iter()
    .cloned()
    .map(|mut row| {
      "open".clone_into(&mut row.state);
      for state in row.blockers.values_mut() {
        "open".clone_into(state);
      }
      row
    })
    .collect();
  // A stale blocker between closed issues can close a cycle the live graph never sees.
  let (levels, cycle) = compute_levels(&all);
  let levels = if cycle.is_empty() {
    levels
  } else {
    compute_levels(rows).0
  };
  rows
    .iter()
    .filter_map(|row| Some((row.key.clone(), *levels.get(&row.reference)?)))
    .collect()
}

/// Each blocker's open dependents.
fn dependents(issues: &[GraphIssue]) -> BTreeMap<&str, BTreeSet<&str>> {
  let mut map: BTreeMap<&str, BTreeSet<&str>> = issues
    .iter()
    .map(|issue| (issue.reference.as_str(), BTreeSet::new()))
    .collect();
  for issue in issues.iter().filter(|issue| issue.state == "open") {
    for blocker in issue.blockers.keys() {
      if let Some(set) = map.get_mut(blocker.as_str()) {
        set.insert(issue.reference.as_str());
      }
    }
  }
  map
}

/// How many issues transitively wait on each issue.
pub(crate) fn downstream_counts(issues: &[GraphIssue]) -> BTreeMap<String, u64> {
  let map = dependents(issues);
  map
    .keys()
    .map(|start| {
      let seen = reachable(&map, start);
      (
        (*start).to_owned(),
        u64::try_from(seen.len()).unwrap_or(u64::MAX),
      )
    })
    .collect()
}

/// Every issue reachable from `start` through open dependents.
fn reachable<'a>(map: &BTreeMap<&'a str, BTreeSet<&'a str>>, start: &'a str) -> BTreeSet<&'a str> {
  let mut seen: BTreeSet<&str> = BTreeSet::new();
  let mut stack: Vec<&str> = vec![start];
  while let Some(reference) = stack.pop() {
    let fresh: Vec<&str> = map
      .get(reference)
      .into_iter()
      .flatten()
      .copied()
      .filter(|next| seen.insert(next))
      .collect();
    stack.extend(fresh);
  }
  seen
}

/// The longest chain of open dependents starting at each issue (itself included).
pub(crate) fn chain_lengths(issues: &[GraphIssue]) -> BTreeMap<String, u64> {
  let map = dependents(issues);
  let mut memo: BTreeMap<&str, u64> = BTreeMap::new();
  for start in map.keys() {
    depth(start, &mut BTreeSet::new(), &map, &mut memo);
  }
  memo
    .into_iter()
    .map(|(reference, depth)| (reference.to_owned(), depth))
    .collect()
}

fn depth<'a>(
  reference: &'a str,
  trail: &mut BTreeSet<&'a str>,
  map: &BTreeMap<&'a str, BTreeSet<&'a str>>,
  memo: &mut BTreeMap<&'a str, u64>,
) -> u64 {
  if let Some(known) = memo.get(reference) {
    return *known;
  }
  trail.insert(reference);
  let mut deepest = 0;
  for next in map.get(reference).into_iter().flatten() {
    if !trail.contains(next) {
      deepest = deepest.max(depth(next, trail, map, memo));
    }
  }
  trail.remove(reference);
  memo.insert(reference, deepest + 1);
  deepest + 1
}

#[cfg(test)]
#[path = "tests/levels_test.rs"]
mod tests;
