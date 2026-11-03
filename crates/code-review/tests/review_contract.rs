//! The review stage end to end against loopback servers: Jev keeps supported and
//! uncertain findings, only a verified dismissal removes a contradicted one, and
//! an outage, an oversized evidence package or an expired deadline keeps the
//! baseline findings.

#[path = "helpers/judge.rs"]
mod judge;
#[path = "helpers/rig.rs"]
mod rig;
#[path = "helpers/tools.rs"]
mod tools;

use std::cell::RefCell;

use judge::{GUARD, choices, rig};
use rig::{JEV, KEY, REVIEW, completion, finding, findings};
use serde_json::json;
use toolu_code_review::budget::Effort;
use toolu_code_review::contract::Cost;
use toolu_code_review::contract::Severity;
use toolu_code_review::memory::{Dismissal, Memory, PriorFinding, Status};
use toolu_code_review::review::{Assessment, Input, Options, Record, Reviewed, Stage};
use toolu_code_review::settings::Settings;
use toolu_http_test_support::Reply;

type Recorded = Vec<(Stage, String, String)>;

/// Four findings: supported, contradicted with a valid dismissal, contradicted
/// with a quote absent from the evidence, and uncertain.
fn judged_run() -> Result<(Reviewed, Recorded), String> {
  let rig = rig()?;
  let model = [
    finding("src/lib.rs", "high", "claim zero", None),
    finding("src/lib.rs", "high", "claim one", None),
    finding("src/lib.rs", "medium", "claim two", None),
    finding("src/lib.rs", "low", "claim three", None),
  ];
  let fail = |err: toolu_http_test_support::Error| err.to_string();
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&model)))
    .map_err(fail)?;
  let jev = choices(&[
    ("f0", "supported", 0.9),
    ("f1", "contradicted", 0.9),
    ("f2", "contradicted", 0.8),
    ("f3", "supported", 0.5),
  ]);
  rig.fixture.route(JEV, Reply::new(200, jev)).map_err(fail)?;
  let why = "the empty slice returns before the index";
  let rechecks = json!({"rechecks": [
    {"id": "f1", "decision": "dismiss", "explanation": why, "evidence_quotes": [GUARD]},
    {"id": "f2", "decision": "dismiss", "explanation": why, "evidence_quotes": ["if items.len() == 0"]}
  ]});
  let recheck = vec![Reply::new(200, completion(&rechecks))];
  rig
    .fixture
    .sequence_when(REVIEW, "code_review_recheck", recheck)
    .map_err(fail)?;
  let records = RefCell::new(Vec::new());
  let recorder = |record: &Record<'_>| {
    let entry = (
      record.stage,
      record.request.to_owned(),
      record.response.to_owned(),
    );
    records.borrow_mut().push(entry);
  };
  let options = Options {
    recorder: Some(&recorder),
    ..rig.options()
  };
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::Medium), &options)
    .map_err(|err| format!("{err:?}"))?;
  Ok((reviewed, records.into_inner()))
}

#[test]
fn supported_kept_dismissed_removed_invalid_dismissal_and_uncertain_kept() {
  let (reviewed, _) = judged_run().unwrap();
  let kept: Vec<(&str, Assessment)> = reviewed
    .findings
    .iter()
    .map(|item| (item.finding.claim.as_str(), item.assessment))
    .collect();
  let expected = [
    ("claim zero", Assessment::Supported),
    ("claim two", Assessment::Contradicted),
    ("claim three", Assessment::Insufficient),
  ];
  assert_eq!(kept, expected);
  assert_eq!(reviewed.dismissed.len(), 1);
  assert_eq!(reviewed.dismissed[0].finding.claim, "claim one");
  assert_eq!(reviewed.dismissed[0].quotes, [GUARD]);
}

#[test]
fn every_call_is_summed_logged_and_recorded_without_the_key_or_source() {
  let (reviewed, records) = judged_run().unwrap();
  let stages: Vec<Stage> = reviewed.calls.iter().map(|call| call.stage).collect();
  assert_eq!(stages, [Stage::Review, Stage::Judge, Stage::Recheck]);
  let usage = &reviewed.usage;
  assert_eq!((usage.tokens_in, usage.tokens_out), (230, 83));
  assert_eq!(usage.cost_usd, Cost::usd(0.022));
  assert_eq!(records.len(), 3);
  let clean = |(_, request, response): &(Stage, String, String)| {
    !request.contains(KEY) && !response.contains(KEY)
  };
  assert!(records.iter().all(clean));
  assert!(records[1].1.contains("typesafe/jev-1.13") && records[1].2.contains("\"answers\""));
  let log = serde_json::to_string(&reviewed.calls).unwrap();
  assert!(!log.contains(KEY) && !log.contains("items.len()"), "{log}");
}

#[test]
fn a_jev_outage_keeps_the_baseline_and_sends_no_recheck() {
  let rig = rig().unwrap();
  rig
    .fixture
    .route(
      REVIEW,
      Reply::new(
        200,
        findings(&[finding("src/lib.rs", "high", "claim", None)]),
      ),
    )
    .unwrap();
  rig.fixture.route(JEV, Reply::new(500, "down")).unwrap();
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::High), &rig.options())
    .unwrap();
  assert_eq!(reviewed.findings[0].assessment, Assessment::Unavailable);
  assert!(
    reviewed
      .notes
      .contains(&"jev unavailable for src/lib.rs: HTTP 500".to_owned()),
    "{:?}",
    reviewed.notes
  );
  assert_eq!(rig.bodies(JEV).len(), 3, "three attempts");
  assert_eq!(rig.bodies(REVIEW).len(), 1, "no recheck");
}

/// One open, one resolved and one dismissed prior finding, and the dismissal.
fn memory() -> Memory {
  let prior = |id: &str, status| PriorFinding {
    fingerprint: id.into(),
    id: id.into(),
    path: "src/lib.rs".into(),
    category: "correctness".into(),
    claim: format!("prior claim {id}"),
    severity: Severity::High,
    status,
  };
  let reason = "guarded by the caller".into();
  Memory {
    prior: vec![
      prior("p-open", Status::Open),
      prior("p-done", Status::Resolved),
      prior("p-gone", Status::Dismissed),
    ],
    dismissals: vec![Dismissal {
      id: "p-gone".into(),
      reason,
      by: "agent".into(),
    }],
  }
}

#[test]
fn memory_sends_open_findings_and_dismissals_and_skips_re_raised_ones() {
  let rig = rig().unwrap();
  let model = [
    finding("src/lib.rs", "high", "re-raised", Some("p-open")),
    finding("src/lib.rs", "high", "fresh", None),
  ];
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&model)))
    .unwrap();
  let contradicted = choices(&[("f1", "contradicted", 0.9)]);
  rig
    .fixture
    .route(JEV, Reply::new(200, contradicted))
    .unwrap();
  let keep = json!({"rechecks": [{"id": "f1", "decision": "keep", "explanation": "", "evidence_quotes": []}]});
  let recheck = vec![Reply::new(200, completion(&keep))];
  rig
    .fixture
    .sequence_when(REVIEW, "code_review_recheck", recheck)
    .unwrap();
  let reviewed = rig
    .run(&memory(), Some(Effort::Medium), &rig.options())
    .unwrap();
  let review = rig.bodies(REVIEW);
  assert!(review[0].contains("prior claim p-open") && review[0].contains("guarded by the caller"));
  assert!(!review[0].contains("prior claim p-done"));
  assert!(review[0].contains("prior_id"), "the rules ask for prior_id");
  assert!(review[1].contains("code_review_recheck") && review[1].contains("guarded by the caller"));
  let jev = rig.bodies(JEV);
  assert!(jev[0].contains("fresh") && !jev[0].contains("re-raised"));
  assert_eq!(reviewed.findings[0].assessment, Assessment::NotJudged);
  assert_eq!(reviewed.findings[1].assessment, Assessment::Contradicted);
}

/// A fake `codex` on PATH serves findings with no network: the review runs on
/// the host CLI, the usage log names it, and the loopback origin hears nothing.
#[test]
fn a_host_cli_reviews_the_chunk_with_no_network() {
  use std::os::unix::fs::PermissionsExt as _;
  let rig = judge::rig().unwrap();
  let content = json!({"findings": [{
    "path": "src/lib.rs", "line": 3, "end_line": 3, "side": "new", "severity": "high",
    "category": "correctness", "claim": "the index can underflow",
    "quote": "Some(items[items.len() - 1])", "suggestion": null, "prior_id": null}]});
  let bin = tempfile::tempdir().unwrap();
  let codex = bin.path().join("codex");
  std::fs::write(&codex, format!("#!/bin/sh\nprintf '%s' '{content}'")).unwrap();
  std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).unwrap();
  let env = rig.env().with("PATH", &bin.path().to_string_lossy());
  let settings = Settings {
    enabled: true,
    ..Settings::default()
  };
  let input = Input {
    root: rig.repo.path(),
    shaped: &rig.shaped,
    rules: &rig.rules,
    prepass: &rig.prepass,
    memory: &Memory::default(),
    settings: &settings,
    flag: Some(Effort::Medium),
  };
  let options = Options {
    skip_judge: true,
    ..rig.options()
  };
  let reviewed = toolu_code_review::review::run(&env, &input, &options).unwrap();
  assert_eq!(reviewed.findings.len(), 1);
  assert_eq!(reviewed.findings[0].finding.path, "src/lib.rs");
  let call = reviewed
    .calls
    .iter()
    .find(|call| call.stage == Stage::Review)
    .unwrap();
  assert_eq!(call.provider.as_deref(), Some("codex"));
  assert_eq!(
    (reviewed.usage.tokens_in, reviewed.usage.tokens_out),
    (0, 0),
    "a host CLI reports no tokens"
  );
  assert!(
    rig.fixture.requests().unwrap().is_empty(),
    "nothing was sent"
  );
}
