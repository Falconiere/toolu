//! A sub-issue's status from its tracker state, blockers, PRs and launch record.

use std::collections::BTreeSet;

use super::GraphIssue;

/// Stages whose durable ownership keeps a slot until cleanup or abandonment ends.
pub(crate) const OCCUPIED_STAGES: [&str; 7] = [
  "starting",
  "uncertain",
  "replacing",
  "running",
  "awaiting_merge",
  "cleaning",
  "cleanup-incomplete",
];

/// Whether a launch record's `stage` holds a slot.
pub(crate) fn occupies_slot(stage: Option<&str>) -> bool {
  stage.is_some_and(|stage| OCCUPIED_STAGES.contains(&stage))
}

/// `in_flight`, `done`, `external_blocked`, `blocked` or `ready`.
pub(crate) fn classify(
  issue: &GraphIssue,
  in_epic: &BTreeSet<&str>,
  stage: Option<&str>,
) -> &'static str {
  if occupies_slot(stage) {
    return "in_flight";
  }
  if issue.state == "closed" {
    return "done";
  }
  if issue.prs.iter().any(|pr| pr.state == "OPEN") {
    return "in_flight";
  }
  let open: Vec<&str> = issue
    .blockers
    .iter()
    .filter(|(_, state)| *state == "open")
    .map(|(blocker, _)| blocker.as_str())
    .collect();
  if open.iter().any(|blocker| !in_epic.contains(blocker)) {
    "external_blocked"
  } else if open.is_empty() {
    "ready"
  } else {
    "blocked"
  }
}

#[cfg(test)]
#[path = "tests/classify_test.rs"]
mod tests;
