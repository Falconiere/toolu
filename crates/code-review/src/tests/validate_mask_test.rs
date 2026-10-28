use crate::contract::{Finding, RedactionSpan, Severity, Side, Source};
use crate::memory::Memory;
use crate::prepass::Redaction;
use crate::validate::tests::{file, line, model, paths, run, shaped};

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
  // The model saw the masked patch, so it quotes the mask, not the secret.
  let finding = model("src/a.rs", 1, "let key = \"[REDACTED]\";");
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
    assessment: None,
  };
  let memory = Memory::default();
  let out = run((&diff, &[finding], &[trusted], &memory, &redactions, &[]));
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
  let out = run((
    &diff,
    &[model("src/a.rs", 1, "let x = 1;")],
    &[],
    &Memory::default(),
    &redactions,
    &[],
  ));
  assert_eq!(out.findings.len(), 0);
  assert_eq!(paths(&out.unreviewed), ["src/a.rs"]);
}

#[test]
fn a_multiline_finding_carries_no_redaction_spans() {
  let secret = "tok12345";
  let text = format!("let key = \"{secret}\";");
  let diff = shaped(
    vec![file(
      "src/a.rs",
      vec![line(Side::New, 1, "let x = 1;"), line(Side::New, 2, &text)],
    )],
    &["src/a.rs"],
  );
  let start = text.find(secret).unwrap();
  let redactions = vec![Redaction {
    path: "src/a.rs".to_owned(),
    side: Side::New,
    line: 2,
    spans: vec![RedactionSpan {
      start,
      end: start + secret.len(),
    }],
  }];
  let mut finding = model("src/a.rs", 1, "let x = 1;\nlet key = \"[REDACTED]\";");
  finding.end_line = 2;
  let out = run((&diff, &[finding], &[], &Memory::default(), &redactions, &[]));
  assert_eq!(out.findings.len(), 1);
  assert_eq!(out.findings[0].redaction_spans.len(), 0);
}
