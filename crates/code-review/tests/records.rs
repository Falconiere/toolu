//! The memory records round-trip through JSON, and the fixed caps keep their
//! starting values. The report's schema tests live with the CLI schema
//! (`crates/cli/tests/helpers/code_review_schema.rs`).

use toolu_code_review::budget::{
  CHECKLIST_MAX_BYTES, HTTP_RESPONSE_MAX_BYTES, JEV_REQUEST_MAX_BYTES, JEV_STATE_MAX_BYTES,
  RULES_MAX_BYTES,
};
use toolu_code_review::contract::{Severity, Verdict};
use toolu_code_review::memory::{Counts, Dismissal, Pass, PriorFinding, Status};

#[test]
fn the_memory_records_round_trip() {
  let prior = PriorFinding {
    fingerprint: "src/lib.rs\u{1f}correctness\u{1f}slice underflows".to_owned(),
    id: "f1".to_owned(),
    severity: Severity::Blocker,
    status: Status::Dismissed,
  };
  let dismissal = Dismissal {
    id: "f1".to_owned(),
    reason: "guarded by the caller".to_owned(),
    by: "agent".to_owned(),
  };
  let pass = Pass {
    diff_sha: "abc123".to_owned(),
    verdict: Verdict::Blocked,
    counts: Counts {
      blocking: 3,
      advisory: 2,
    },
  };
  let text = serde_json::to_string(&(&prior, &dismissal, &pass)).unwrap();
  assert!(
    text.contains("\"dismissed\"") && text.contains("\"blocked\""),
    "{text}"
  );
  let back: (PriorFinding, Dismissal, Pass) = serde_json::from_str(&text).unwrap();
  assert_eq!(back, (prior, dismissal, pass));
  let names =
    [Status::Open, Status::Resolved].map(|status| serde_json::to_string(&status).unwrap());
  assert_eq!(names, ["\"open\"", "\"resolved\""]);
}

#[test]
fn the_fixed_caps_are_the_starting_values() {
  assert_eq!(RULES_MAX_BYTES, 65_536);
  assert_eq!(CHECKLIST_MAX_BYTES, 4_506);
  assert_eq!(
    (JEV_STATE_MAX_BYTES, JEV_REQUEST_MAX_BYTES),
    (90_000, 180_000)
  );
  assert_eq!(HTTP_RESPONSE_MAX_BYTES, 1_048_576);
  assert_eq!(
    Counts::default(),
    Counts {
      blocking: 0,
      advisory: 0
    }
  );
}
