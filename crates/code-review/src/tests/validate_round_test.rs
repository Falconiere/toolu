use crate::contract::{Coverage, Severity, Side};
use crate::memory::{Memory, PriorFinding, Status};
use crate::model::ModelFinding;
use crate::shape::{ChangedLine, FileKind, Rename, Shaped, ShapedFile};
use crate::validate::{fingerprint, validate};

fn line(side: Side, line: u32, text: &str) -> ChangedLine {
  ChangedLine {
    side,
    line,
    text: text.to_owned(),
  }
}

fn shaped(files: Vec<ShapedFile>, reviewed: &[&str]) -> Shaped {
  Shaped {
    base_ref: "main".to_owned(),
    merge_base: "abc".to_owned(),
    diff_sha: "def".to_owned(),
    gate_files: files.iter().map(|file| file.path.clone()).collect(),
    files,
    chunks: Vec::new(),
    coverage: Coverage {
      reviewed: reviewed.iter().map(|path| (*path).to_owned()).collect(),
      unreviewed: Vec::new(),
      skipped: Vec::new(),
    },
    renames: Vec::new(),
    uncommitted: false,
  }
}

fn file(path: &str, lines: Vec<ChangedLine>) -> ShapedFile {
  ShapedFile {
    path: path.to_owned(),
    old_path: None,
    kind: FileKind::Modified,
    patch: String::new(),
    lines,
  }
}

fn model(path: &str, line_no: u32, quote: &str) -> ModelFinding {
  ModelFinding {
    path: path.to_owned(),
    line: line_no,
    end_line: line_no,
    side: Side::New,
    severity: Severity::High,
    category: "correctness".to_owned(),
    claim: "the slice underflows when empty".to_owned(),
    quote: quote.to_owned(),
    suggestion: None,
    prior_id: None,
  }
}

fn prior(id: &str, path: &str, status: Status, severity: Severity) -> PriorFinding {
  PriorFinding {
    fingerprint: fingerprint(path, "correctness", "the slice underflows when empty"),
    id: id.to_owned(),
    path: path.to_owned(),
    category: "correctness".to_owned(),
    claim: "the slice underflows when empty".to_owned(),
    severity,
    status,
  }
}

fn memory(prior: Vec<PriorFinding>) -> Memory {
  Memory {
    prior,
    dismissals: Vec::new(),
  }
}

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
  assert!(out.recap.new.is_empty());
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
  assert!(out.recap.new.is_empty());
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
  assert!(out.findings.is_empty());
}
