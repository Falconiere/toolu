use super::{fingerprint, normalize_text, validate};
use crate::contract::{Coverage, Finding, Omitted, RedactionSpan, Severity, Side, Source};
use crate::memory::{Memory, PriorFinding, Status};
use crate::model::ModelFinding;
use crate::prepass::Redaction;
use crate::shape::{ChangedLine, FileKind, Shaped, ShapedFile};

pub(crate) fn line(side: Side, line: u32, text: &str) -> ChangedLine {
  ChangedLine {
    side,
    line,
    text: text.to_owned(),
  }
}

pub(crate) fn shaped(files: Vec<ShapedFile>, reviewed: &[&str]) -> Shaped {
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

pub(crate) fn file(path: &str, lines: Vec<ChangedLine>) -> ShapedFile {
  ShapedFile {
    path: path.to_owned(),
    old_path: None,
    kind: FileKind::Modified,
    patch: String::new(),
    lines,
  }
}

pub(crate) fn model(path: &str, line_no: u32, quote: &str) -> ModelFinding {
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

pub(crate) fn prior(id: &str, path: &str, status: Status, severity: Severity) -> PriorFinding {
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

pub(crate) fn memory(prior: Vec<PriorFinding>) -> Memory {
  Memory {
    prior,
    dismissals: Vec::new(),
  }
}

fn paths(omitted: &[Omitted]) -> Vec<&str> {
  omitted.iter().map(|item| item.path.as_str()).collect()
}

#[test]
fn fingerprint_is_line_independent_and_normalized() {
  assert_eq!(
    normalize_text("  The   Slice\nUnderflows "),
    "the slice underflows"
  );
  assert_eq!(
    fingerprint("src/a.rs", " Correctness ", "The   slice underflows"),
    "src/a.rs\u{1f}correctness\u{1f}the slice underflows"
  );
}

#[test]
fn hallucinated_line_is_dropped_and_its_coverage_is_unreviewed() {
  let diff = shaped(
    vec![file("src/a.rs", vec![line(Side::New, 1, "let x = 1;")])],
    &["src/a.rs"],
  );
  let mut bad = model("src/a.rs", 99, "let x = 1;");
  let out = validate(&diff, &[bad.clone()], &[], &Memory::default(), &[]);
  assert_eq!(out.findings.len(), 0);
  assert_eq!(out.dropped, 1);
  assert_eq!(paths(&out.unreviewed), ["src/a.rs"]);
  bad.path = "src/ghost.rs".to_owned();
  let out = validate(&diff, &[bad], &[], &Memory::default(), &[]);
  assert_eq!(out.dropped, 1);
  assert_eq!(paths(&out.unreviewed), ["src/ghost.rs"]);
}

#[test]
fn quote_mismatch_drops_but_whitespace_only_difference_survives() {
  let diff = shaped(
    vec![file(
      "src/a.rs",
      vec![line(Side::New, 1, "let last = items[items.len() - 1];")],
    )],
    &["src/a.rs"],
  );
  let wrong = model("src/a.rs", 1, "let last = items[0];");
  let out = validate(&diff, &[wrong], &[], &Memory::default(), &[]);
  assert_eq!(out.findings.len(), 0);
  assert_eq!(out.dropped, 1);
  assert_eq!(paths(&out.unreviewed), ["src/a.rs"]);
  let spaced = model("src/a.rs", 1, "let  last = items[items.len()  - 1];");
  let out = validate(&diff, &[spaced], &[], &Memory::default(), &[]);
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.unreviewed.len(), 0);
}

#[test]
fn a_removed_line_anchors_on_the_old_side() {
  let mut removed = file("src/a.rs", vec![line(Side::Old, 3, "return old_value;")]);
  removed.kind = FileKind::Deleted;
  let diff = shaped(vec![removed], &["src/a.rs"]);
  let mut finding = model("src/a.rs", 3, "return old_value;");
  finding.side = Side::Old;
  finding.claim = "the fallback return was removed".to_owned();
  let out = validate(&diff, &[finding], &[], &Memory::default(), &[]);
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.findings[0].side, Side::Old);
}

#[test]
fn duplicates_keep_the_highest_severity_and_deterministic_wins_ties() {
  let diff = shaped(
    vec![file("src/a.rs", vec![line(Side::New, 1, "let x = 1;")])],
    &["src/a.rs"],
  );
  let mut low = model("src/a.rs", 1, "let x = 1;");
  low.severity = Severity::Low;
  let mut blocker = low.clone();
  blocker.severity = Severity::Blocker;
  let out = validate(&diff, &[low, blocker], &[], &Memory::default(), &[]);
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.findings[0].severity, Severity::Blocker);
  let trusted = Finding {
    id: "prepass:secrets:src/a.rs:1:correctness".to_owned(),
    path: "src/a.rs".to_owned(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity: Severity::Blocker,
    category: "correctness".to_owned(),
    source: Source::Secrets,
    claim: "the slice underflows when empty".to_owned(),
    quote: "let x = 1;".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
  };
  let mut copy = model("src/a.rs", 1, "let x = 1;");
  copy.severity = Severity::Blocker;
  let out = validate(&diff, &[copy], &[trusted], &Memory::default(), &[]);
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.findings[0].source, Source::Secrets);
}

#[test]
fn withdrawals_are_absent_and_uncounted_but_a_real_defect_survives() {
  let diff = shaped(
    vec![file("src/a.rs", vec![line(Side::New, 1, "let x = 1;")])],
    &["src/a.rs"],
  );
  let mut by_category = model("src/a.rs", 1, "let x = 1;");
  by_category.category = "no_defect".to_owned();
  let mut by_claim = model("src/a.rs", 1, "let x = 1;");
  by_claim.claim = "No defect, abstain".to_owned();
  let mut real = model("src/a.rs", 1, "let x = 1;");
  real.claim = "this is a defect even though the guard has no defect".to_owned();
  let out = validate(
    &diff,
    &[by_category, by_claim, real],
    &[],
    &Memory::default(),
    &[],
  );
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.dropped, 0);
  assert_eq!(out.unreviewed.len(), 0);
}

#[test]
fn prose_suggestion_is_dropped_and_code_suggestion_is_kept() {
  let diff = shaped(
    vec![file("src/a.rs", vec![line(Side::New, 1, "let x = 1;")])],
    &["src/a.rs"],
  );
  let mut prose = model("src/a.rs", 1, "let x = 1;");
  prose.suggestion = Some("You should change this value to something safer".to_owned());
  let out = validate(&diff, &[prose], &[], &Memory::default(), &[]);
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.findings[0].suggestion, None);
  let mut code = model("src/a.rs", 1, "let x = 1;");
  code.suggestion = Some("let x = 2;".to_owned());
  let out = validate(&diff, &[code], &[], &Memory::default(), &[]);
  assert_eq!(out.findings[0].suggestion.as_deref(), Some("let x = 2;"));
}

#[test]
fn deterministic_findings_are_exempt_and_model_quotes_are_masked() {
  let secret = "ghp_Ab1Ab1Ab1";
  let text = format!("let key = \"{secret}\";");
  let diff = shaped(
    vec![file("src/a.rs", vec![line(Side::New, 1, &text)])],
    &["src/a.rs"],
  );
  let start = text.find(secret).unwrap();
  let redactions = vec![Redaction {
    path: "src/a.rs".to_owned(),
    side: Side::New,
    line: 1,
    spans: vec![RedactionSpan {
      start,
      end: start + secret.len(),
    }],
  }];
  let finding = model("src/a.rs", 1, &text);
  let trusted = Finding {
    id: "prepass:ast-grep:src/a.rs:1:style".to_owned(),
    path: "src/a.rs".to_owned(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity: Severity::Low,
    category: "style".to_owned(),
    source: Source::AstGrep,
    claim: "unverifiable by design".to_owned(),
    quote: "not in the diff".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
  };
  let out = validate(
    &diff,
    &[finding],
    &[trusted],
    &Memory::default(),
    &redactions,
  );
  assert_eq!(out.findings.len(), 2);
  let masked = out
    .findings
    .iter()
    .find(|item| item.source == Source::Llm)
    .unwrap();
  assert!(
    masked.quote.contains("[REDACTED]") && !masked.quote.contains(secret),
    "{}",
    masked.quote
  );
}

#[test]
fn an_unresolvable_redaction_drops_the_paths_findings_as_unreviewed() {
  let diff = shaped(
    vec![file("src/a.rs", vec![line(Side::New, 1, "let x = 1;")])],
    &["src/a.rs"],
  );
  let redactions = vec![Redaction {
    path: "src/a.rs".to_owned(),
    side: Side::New,
    line: 1,
    spans: vec![RedactionSpan { start: 50, end: 60 }],
  }];
  let out = validate(
    &diff,
    &[model("src/a.rs", 1, "let x = 1;")],
    &[],
    &Memory::default(),
    &redactions,
  );
  assert_eq!(out.findings.len(), 0);
  assert_eq!(paths(&out.unreviewed), ["src/a.rs"]);
}
