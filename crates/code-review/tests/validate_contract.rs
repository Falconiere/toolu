//! Black-box contract for #668: `validate` grounds model findings in the
//! shaped diff, and the public result types are what #669 consumes.

use toolu_code_review::contract::{Coverage, Severity, Side};
use toolu_code_review::memory::{Memory, PriorFinding, Status};
use toolu_code_review::model::ModelFinding;
use toolu_code_review::shape::{ChangedLine, FileKind, Shaped, ShapedFile};
use toolu_code_review::validate::{Input, Recap, SeverityChange, Validated, fingerprint, validate};

fn shaped() -> Shaped {
  Shaped {
    base_ref: "main".to_owned(),
    merge_base: "abc".to_owned(),
    diff_sha: "def".to_owned(),
    gate_files: vec!["src/a.rs".to_owned()],
    files: vec![ShapedFile {
      path: "src/a.rs".to_owned(),
      old_path: None,
      kind: FileKind::Modified,
      patch: String::new(),
      lines: vec![ChangedLine {
        side: Side::New,
        line: 1,
        text: "let x = items[0];".to_owned(),
      }],
    }],
    chunks: Vec::new(),
    coverage: Coverage {
      reviewed: vec!["src/a.rs".to_owned()],
      unreviewed: Vec::new(),
      skipped: Vec::new(),
    },
    renames: Vec::new(),
    uncommitted: false,
  }
}

fn finding(claim: &str) -> ModelFinding {
  ModelFinding {
    path: "src/a.rs".to_owned(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity: Severity::High,
    category: "correctness".to_owned(),
    claim: claim.to_owned(),
    quote: "let x = items[0];".to_owned(),
    suggestion: None,
    prior_id: None,
    assessment: None,
  }
}

#[test]
fn a_grounded_finding_survives_and_a_hallucinated_one_marks_coverage_unreviewed() {
  // Invented both ways: a line outside the diff quoting text it never had.
  let mut hallucinated = finding("invented defect");
  hallucinated.line = 42;
  hallucinated.end_line = 42;
  hallucinated.quote = "let y = items.pop();".to_owned();
  let shaped = shaped();
  let out: Validated = validate(&Input {
    shaped: &shaped,
    model: &[finding("indexing can panic"), hallucinated],
    deterministic: &[],
    memory: &Memory::default(),
    redactions: &[],
    unreviewed: &[],
  });
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.dropped, 1);
  assert_eq!(out.unreviewed.len(), 1);
  assert_eq!(out.unreviewed[0].path, "src/a.rs");
  let recap: Recap = out.recap;
  assert_eq!(recap.new.len(), 1);
}

#[test]
fn severity_drift_against_an_open_prior_is_recorded() {
  let memory = Memory {
    prior: vec![PriorFinding {
      fingerprint: fingerprint("src/a.rs", "correctness", "indexing can panic"),
      id: "f1".to_owned(),
      path: "src/a.rs".to_owned(),
      category: "correctness".to_owned(),
      claim: "indexing can panic".to_owned(),
      severity: Severity::Low,
      status: Status::Open,
    }],
    dismissals: Vec::new(),
  };
  let shaped = shaped();
  let out = validate(&Input {
    shaped: &shaped,
    model: &[finding("indexing can panic")],
    deterministic: &[],
    memory: &memory,
    redactions: &[],
    unreviewed: &[],
  });
  assert_eq!(out.findings[0].id, "f1");
  assert_eq!(
    out.recap.severity_changes,
    vec![SeverityChange {
      id: "f1".to_owned(),
      from: Severity::Low,
      to: Severity::High,
    }]
  );
  assert_eq!(out.recap.still_open, ["f1"]);
}
