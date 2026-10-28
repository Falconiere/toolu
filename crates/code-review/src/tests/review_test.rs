use super::{Assessment, Judged, RunError, shown};
use crate::budget::Effort;
use crate::contract::{Cost, Coverage, Finding, Severity, Side, Source, Usage};
use crate::model::ModelFinding;
use crate::prepass::PrepassResult;
use crate::shape::{ChangedLine, FileKind, Shaped, ShapedFile};

/// A shaped diff of `files`, each a path and its added lines.
pub(crate) fn shaped(files: &[(&str, &[&str])]) -> Shaped {
  let files: Vec<ShapedFile> = files
    .iter()
    .map(|(path, added)| ShapedFile {
      path: (*path).into(),
      old_path: None,
      kind: FileKind::Modified,
      patch: added
        .iter()
        .fold(String::new(), |patch, line| patch + "+" + line + "\n"),
      lines: (1..)
        .zip(added.iter())
        .map(|(line, text)| ChangedLine {
          side: Side::New,
          line,
          text: (*text).into(),
        })
        .collect(),
    })
    .collect();
  Shaped {
    base_ref: "main".into(),
    merge_base: "abc".into(),
    diff_sha: "def".into(),
    gate_files: files.iter().map(|file| file.path.clone()).collect(),
    files,
    chunks: Vec::new(),
    coverage: Coverage::default(),
    renames: Vec::new(),
    uncommitted: false,
  }
}

/// A pre-pass result holding `findings` and no redactions.
pub(crate) fn prepass(findings: Vec<Finding>) -> PrepassResult {
  PrepassResult {
    findings,
    redactions: Vec::new(),
    notices: Vec::new(),
    overflow: false,
  }
}

/// A trusted pre-pass finding from `source`.
pub(crate) fn trusted(source: Source) -> Finding {
  Finding {
    id: "prepass:1".into(),
    path: "src/a.rs".into(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity: Severity::High,
    category: "secret".into(),
    source,
    claim: "a key is committed".into(),
    quote: "[REDACTED]".into(),
    suggestion: None,
    redaction_spans: Vec::new(),
  }
}

/// A model finding on `path` with `severity`.
pub(crate) fn finding(path: &str, severity: Severity) -> ModelFinding {
  ModelFinding {
    path: path.into(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity,
    category: "correctness".into(),
    claim: "the slice underflows when empty".into(),
    quote: "let last = items[items.len() - 1];".into(),
    suggestion: None,
    prior_id: None,
  }
}

#[test]
fn a_failed_run_reports_its_kind_and_keeps_its_usage() {
  let usage = Usage {
    tokens_in: 5,
    tokens_out: 7,
    cost_usd: Cost::usd(0.5),
    model: Some("m".into()),
  };
  let cases = [
    (RunError::NotEnabled, "not-enabled"),
    (
      RunError::Unconfigured("OPENROUTER_API_KEY is missing".into()),
      "unconfigured",
    ),
    (
      RunError::Outage {
        message: "down".into(),
        usage: usage.clone(),
      },
      "outage",
    ),
    (
      RunError::Deadline {
        usage: usage.clone(),
      },
      "deadline",
    ),
  ];
  for (error, kind) in cases {
    let report = error.report(Effort::High);
    let doc = serde_json::to_value(&report).unwrap();
    assert_eq!(
      (doc["verdict"].as_str(), doc["error"]["kind"].as_str()),
      (Some("error"), Some(kind))
    );
    let carries = matches!(error, RunError::Outage { .. } | RunError::Deadline { .. });
    assert_eq!(report.usage == usage, carries, "{kind}");
  }
  let message = RunError::Unconfigured("no key".into())
    .report(Effort::Auto)
    .error
    .unwrap()
    .message;
  assert_eq!(message, "no key; nothing was sent");
}

#[test]
fn a_shown_finding_carries_the_claim_but_no_prior_id() {
  let judged = Judged {
    finding: finding("src/a.rs", Severity::High),
    assessment: Assessment::NotJudged,
  };
  let shown = shown(&judged.finding);
  assert_eq!(shown["claim"], "the slice underflows when empty");
  assert_eq!(shown["severity"], "high");
  assert!(shown.get("prior_id").is_none());
}
