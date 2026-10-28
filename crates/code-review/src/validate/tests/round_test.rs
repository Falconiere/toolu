use crate::contract::{Severity, Side};
use crate::memory::Status;
use crate::shape::Rename;
use crate::validate::tests::{file, line, memory, model, prior, shaped};
use crate::validate::{fingerprint, validate};

#[test]
fn cross_round_match_uses_prior_id_then_fingerprint_through_a_rename() {
  let mut renamed = file(
    "src/new.rs",
    vec![line(Side::New, 7, "let last = items[items.len() - 1];")],
  );
  renamed.old_path = Some("src/old.rs".to_owned());
  let mut diff = shaped(vec![renamed], &["src/new.rs"]);
  diff.renames = vec![Rename {
    old_path: "src/old.rs".to_owned(),
    path: "src/new.rs".to_owned(),
  }];
  diff.gate_files.push("src/old.rs".to_owned());
  let priors = memory(vec![prior(
    "f1",
    "src/old.rs",
    Status::Open,
    Severity::High,
  )]);
  let mut raised = model("src/new.rs", 7, "let last = items[items.len() - 1];");
  raised.severity = Severity::Blocker;
  raised.claim = "The  slice underflows when empty".to_owned();
  let out = validate(&diff, &[raised], &[], &priors, &[]);
  assert_eq!(out.findings[0].id, "f1");
  assert_eq!(out.findings[0].severity, Severity::Blocker);
  assert_eq!(out.recap.still_open, ["f1"]);
  assert_eq!(out.recap.new.len(), 0);
  assert_eq!(out.recap.severity_changes.len(), 1);
  assert_eq!(out.recap.severity_changes[0].from, Severity::High);
  assert_eq!(out.recap.severity_changes[0].to, Severity::Blocker);
  // An echoed prior_id matches even when the claim text moved.
  let mut echoed = model("src/new.rs", 7, "let last = items[items.len() - 1];");
  echoed.prior_id = Some("f1".to_owned());
  echoed.claim = "empty input panics here".to_owned();
  let out = validate(&diff, &[echoed], &[], &priors, &[]);
  assert_eq!(out.findings[0].id, "f1");
  assert_eq!(out.recap.still_open, ["f1"]);
}

#[test]
fn recap_resolves_only_reexamined_priors() {
  let diff = shaped(
    vec![
      file("src/a.rs", vec![line(Side::New, 1, "let x = 1;")]),
      file("src/b.rs", vec![line(Side::New, 1, "let y = 2;")]),
    ],
    &["src/a.rs"],
  );
  let mut gone = prior("gone", "src/a.rs", Status::Open, Severity::High);
  gone.claim = "gone claim".to_owned();
  gone.fingerprint = fingerprint("src/a.rs", "correctness", "gone claim");
  let mut kept = prior("kept", "src/b.rs", Status::Open, Severity::High);
  kept.claim = "kept claim".to_owned();
  kept.fingerprint = fingerprint("src/b.rs", "correctness", "kept claim");
  let out = validate(&diff, &[], &[], &memory(vec![gone, kept]), &[]);
  assert_eq!(out.recap.resolved, ["gone"]);
  assert_eq!(out.recap.still_open, ["kept"]);
  assert_eq!(out.recap.new.len(), 0);
}

#[test]
fn dismissals_suppress_nonblocking_by_fingerprint_and_blockers_only_by_exact_id() {
  let diff = shaped(
    vec![file(
      "src/a.rs",
      vec![
        line(Side::New, 1, "let x = 1;"),
        line(Side::New, 2, "let y = 2;"),
      ],
    )],
    &["src/a.rs"],
  );
  let mut low_prior = prior("settled-low", "src/a.rs", Status::Dismissed, Severity::Low);
  low_prior.claim = "low claim".to_owned();
  low_prior.fingerprint = fingerprint("src/a.rs", "correctness", "low claim");
  let mut blocker_prior = prior(
    "settled-blocker",
    "src/a.rs",
    Status::Dismissed,
    Severity::Blocker,
  );
  blocker_prior.claim = "blocker claim".to_owned();
  blocker_prior.fingerprint = fingerprint("src/a.rs", "correctness", "blocker claim");
  let priors = memory(vec![low_prior, blocker_prior]);
  let mut low = model("src/a.rs", 1, "let x = 1;");
  low.severity = Severity::Low;
  low.claim = "low claim".to_owned();
  let mut blocker_fuzzy = model("src/a.rs", 2, "let y = 2;");
  blocker_fuzzy.severity = Severity::Blocker;
  blocker_fuzzy.claim = "blocker claim".to_owned();
  let out = validate(&diff, &[low, blocker_fuzzy], &[], &priors, &[]);
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.findings[0].severity, Severity::Blocker);
  let mut blocker_exact = model("src/a.rs", 2, "let y = 2;");
  blocker_exact.severity = Severity::Blocker;
  blocker_exact.claim = "blocker claim".to_owned();
  blocker_exact.prior_id = Some("settled-blocker".to_owned());
  let out = validate(&diff, &[blocker_exact], &[], &priors, &[]);
  assert_eq!(out.findings.len(), 0);
}
