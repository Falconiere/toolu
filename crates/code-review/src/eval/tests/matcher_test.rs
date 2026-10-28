use super::{Status, match_findings};
use crate::contract::{Finding, Severity, Side, Source};
use crate::eval::{Expectation, Kind};

fn finding(id: &str, path: &str, claim: &str) -> Finding {
  Finding {
    id: id.to_owned(),
    path: path.to_owned(),
    line: 3,
    end_line: 3,
    side: Side::New,
    severity: Severity::High,
    category: "correctness".to_owned(),
    source: Source::Llm,
    claim: claim.to_owned(),
    quote: "quote".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
    assessment: None,
  }
}

fn expectation(id: &str, path: &str, claim: &str, kind: Kind) -> Expectation {
  Expectation {
    id: id.to_owned(),
    path: path.to_owned(),
    claim: claim.to_owned(),
    kind,
  }
}

fn statuses(matches: &[super::Match]) -> Vec<Status> {
  matches.iter().map(|entry| entry.status).collect()
}

#[test]
fn defect_expectation_matches_a_finding_on_the_same_path_and_claim() {
  let rows = [expectation(
    "defect-underflow",
    "src/lib.rs",
    "slice underflows",
    Kind::Defect,
  )];
  let findings = [finding(
    "f1",
    "src/lib.rs",
    "The slice underflows when the input is empty",
  )];
  let outcome = match_findings(&rows, &findings);
  assert_eq!(statuses(&outcome.matches), [Status::Caught]);
  assert_eq!(outcome.matches[0].finding_id.as_deref(), Some("f1"));
  assert_eq!(outcome.unclassified.len(), 0);
  assert!(outcome.defects_satisfied());
}

#[test]
fn claim_substring_ignores_case_in_both_directions() {
  let rows = [expectation(
    "owner",
    "src/own.rs",
    "Ownership Check Is Inverted",
    Kind::Defect,
  )];
  let findings = [finding(
    "f1",
    "src/own.rs",
    "the OWNERSHIP check is INVERTED here",
  )];
  let outcome = match_findings(&rows, &findings);
  assert_eq!(statuses(&outcome.matches), [Status::Caught]);
}

#[test]
fn path_mismatch_is_a_miss_and_leaves_the_finding_unclassified() {
  let rows = [expectation(
    "defect-underflow",
    "src/lib.rs",
    "slice underflows",
    Kind::Defect,
  )];
  let findings = [finding("f1", "src/other.rs", "slice underflows")];
  let outcome = match_findings(&rows, &findings);
  assert_eq!(statuses(&outcome.matches), [Status::Missed]);
  assert_eq!(outcome.matches[0].finding_id, None);
  assert_eq!(outcome.unclassified.len(), 1);
  assert!(!outcome.defects_satisfied());
}

#[test]
fn path_must_match_exactly_not_by_prefix_or_case() {
  let rows = [expectation("d", "src/lib.rs", "bug", Kind::Defect)];
  let findings = [
    finding("f1", "src/lib.rs.bak", "bug"),
    finding("f2", "SRC/LIB.RS", "bug"),
  ];
  let outcome = match_findings(&rows, &findings);
  assert_eq!(statuses(&outcome.matches), [Status::Missed]);
  assert_eq!(outcome.unclassified.len(), 2);
}

#[test]
fn false_positive_expectation_is_flagged_when_matched_and_clear_when_not() {
  let rows = [
    expectation(
      "fp-rewrite",
      "src/lib.rs",
      "prefer rewrite",
      Kind::FalsePositive,
    ),
    expectation(
      "fp-style",
      "src/lib.rs",
      "rename variable",
      Kind::FalsePositive,
    ),
  ];
  let findings = [finding(
    "f1",
    "src/lib.rs",
    "Prefer rewrite in iterator style",
  )];
  let outcome = match_findings(&rows, &findings);
  assert_eq!(statuses(&outcome.matches), [Status::Flagged, Status::Clear]);
  let tally = outcome.tally();
  assert_eq!((tally.flagged, tally.clear, tally.missed), (1, 1, 0));
  assert!(outcome.defects_satisfied());
}

#[test]
fn leftover_findings_are_unclassified() {
  let rows = [expectation(
    "d",
    "src/lib.rs",
    "slice underflows",
    Kind::Defect,
  )];
  let findings = [
    finding("f1", "src/lib.rs", "slice underflows"),
    finding("f2", "src/lib.rs", "an unrelated observation"),
    finding("f3", "src/new.rs", "something else entirely"),
  ];
  let outcome = match_findings(&rows, &findings);
  let ids: Vec<&str> = outcome.unclassified.iter().map(|f| f.id.as_str()).collect();
  assert_eq!(ids, ["f2", "f3"]);
  assert_eq!(outcome.tally().unclassified, 2);
}

#[test]
fn first_unused_matching_finding_wins_and_each_finding_serves_one_row() {
  let rows = [
    expectation("a", "src/lib.rs", "overflow", Kind::Defect),
    expectation("b", "src/lib.rs", "overflow", Kind::Defect),
    expectation("c", "src/lib.rs", "overflow", Kind::Defect),
  ];
  let findings = [
    finding("f1", "src/lib.rs", "integer overflow"),
    finding("f2", "src/lib.rs", "another overflow"),
  ];
  let outcome = match_findings(&rows, &findings);
  let ids: Vec<Option<&str>> = outcome
    .matches
    .iter()
    .map(|entry| entry.finding_id.as_deref())
    .collect();
  assert_eq!(ids, [Some("f1"), Some("f2"), None]);
  assert_eq!(
    statuses(&outcome.matches),
    [Status::Caught, Status::Caught, Status::Missed]
  );
  assert_eq!(outcome.unclassified.len(), 0);
}

#[test]
fn no_findings_misses_every_defect_row() {
  let rows = [expectation("d", "src/lib.rs", "bug", Kind::Defect)];
  let outcome = match_findings(&rows, &[]);
  assert_eq!(statuses(&outcome.matches), [Status::Missed]);
  assert!(!outcome.defects_satisfied());
}
