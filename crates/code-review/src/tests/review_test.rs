use super::calls::Ledger;
use super::{Assessment, Input, Judged, Options, RunError, shown};
use crate::budget::Effort;
use crate::contract::{Cost, Coverage, Finding, Severity, Side, Source, Usage};
use crate::model::ModelFinding;
use crate::prepass::PrepassResult;
use crate::rules::Bundle;
use crate::settings::Settings;
use crate::shape::{ChangedLine, Chunk, FileKind, Shaped, ShapedFile};
use toolu_runtime::env::Env;

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
    assessment: None,
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
    assessment: None,
  }
}

#[test]
fn a_failed_run_reports_its_kind_and_keeps_its_usage() {
  let usage = Usage {
    tokens_in: 5,
    tokens_out: 7,
    cost_usd: Cost::usd(0.5),
    model: Some("m".into()),
    calls: Vec::new(),
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

/// One selectable diff with an isolated PATH: `path` selects the binaries
/// `select_host` sees, so replay, absence, and skip-judge share one setup.
struct SelectCase {
  bin: tempfile::TempDir,
  root: tempfile::TempDir,
  shaped: Shaped,
  rules: Bundle,
  prepass: PrepassResult,
  memory: crate::memory::Memory,
  settings: Settings,
  env: Env,
  jev: toolu_jev_client::Jev,
  ledger: Ledger<'static>,
}

fn select_case(use_bin: bool, executable: bool) -> SelectCase {
  use std::os::unix::fs::PermissionsExt as _;
  let bin = tempfile::tempdir().unwrap();
  let codex = bin.path().join("codex");
  std::fs::write(&codex, "#!/bin/sh\nexit 3").unwrap();
  if executable {
    std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).unwrap();
  }
  let path = if use_bin {
    bin.path().to_string_lossy().into_owned()
  } else {
    "/nonexistent-bin-dir".to_owned()
  };
  let root = tempfile::tempdir().unwrap();
  let mut shaped = shaped(&[("src/a.rs", &["new"])]);
  shaped.chunks.push(Chunk {
    files: vec![0],
    bytes: 4,
  });
  let env = Env::from_pairs([
    ("PATH", path),
    ("OPENROUTER_API_KEY", "private-sentinel".to_owned()),
  ]);
  let jev = toolu_jev_client::Jev::from_env_key(
    &env,
    "OPENROUTER_API_KEY",
    toolu_jev_client::Config::default(),
  )
  .expect("key");
  SelectCase {
    bin,
    root,
    shaped,
    rules: Bundle {
      available: false,
      files: Vec::new(),
      prompt_override: None,
      dropped: Vec::new(),
      notices: Vec::new(),
      bytes: 0,
    },
    prepass: prepass(Vec::new()),
    memory: crate::memory::Memory::default(),
    settings: Settings::default(),
    env,
    jev,
    ledger: Ledger::new(None),
  }
}

fn run(case: &mut SelectCase, options: &Options<'_>) -> Option<super::host::HostPlan> {
  let input = Input {
    root: case.root.path(),
    shaped: &case.shaped,
    rules: &case.rules,
    prepass: &case.prepass,
    memory: &case.memory,
    settings: &case.settings,
    flag: None,
  };
  super::host::select_host(&case.env, &input, options, &case.jev, &mut case.ledger)
}

#[test]
fn replayed_runs_never_select_a_host() {
  let mut case = select_case(true, true);
  let replay: super::Replay<'_> = &|_, _| None;
  let options = Options {
    replay: Some(replay),
    ..Options::default()
  };
  assert_eq!(run(&mut case, &options), None);
  assert_eq!(case.ledger.calls, Vec::new());
}

#[test]
fn without_a_cli_there_is_nothing_to_select() {
  let mut case = select_case(false, false);
  assert_eq!(run(&mut case, &Options::default()), None);
}

#[test]
fn jev_off_selects_the_detected_host_without_asking() {
  let mut case = select_case(true, true);
  let options = Options {
    skip_judge: true,
    ..Options::default()
  };
  let plan = run(&mut case, &options).expect("codex is on PATH");
  assert_eq!(plan.host, toolu_protocol::host::Host::Codex);
  assert_eq!(plan.model, "default");
  assert_eq!(plan.bin, case.bin.path().join("codex"));
  assert_eq!(case.ledger.calls, Vec::new());
}
