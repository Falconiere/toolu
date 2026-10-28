//! The review stage end to end against loopback servers: Jev keeps supported and
//! uncertain findings, only a verified dismissal removes a contradicted one, and
//! an outage, an oversized evidence package or an expired deadline keeps the
//! baseline findings.

#[path = "helpers/rig.rs"]
mod rig;

use std::cell::RefCell;
use std::time::{Duration, Instant};

use rig::{JEV, KEY, REVIEW, Rig, completion, finding, findings};
use serde_json::{Value, json};
use toolu_code_review::budget::Effort;
use toolu_code_review::contract::Cost;
use toolu_code_review::contract::Severity;
use toolu_code_review::memory::{Dismissal, Memory, PriorFinding, Status};
use toolu_code_review::review::{Assessment, Options, Stage};
use toolu_http_test_support::Reply;

const GUARD: &str = "if items.is_empty() { return None; }";

fn rig() -> Rig {
  let base = format!("fn last(items: &[u8]) -> Option<u8> {{\n{GUARD}\nSome(items[0])\n}}\n");
  let change =
    format!("fn last(items: &[u8]) -> Option<u8> {{\n{GUARD}\nSome(items[items.len() - 1])\n}}\n");
  Rig::new(&[("src/lib.rs", base)], &[("src/lib.rs", change)]).unwrap()
}

#[test]
fn supported_kept_dismissed_removed_invalid_dismissal_and_uncertain_kept() {
  let rig = rig();
  let model = [
    finding("src/lib.rs", "high", "claim zero", None),
    finding("src/lib.rs", "high", "claim one", None),
    finding("src/lib.rs", "medium", "claim two", None),
    finding("src/lib.rs", "low", "claim three", None),
  ];
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&model)))
    .unwrap();
  let jev = choices(&[
    ("f0", "supported", 0.9),
    ("f1", "contradicted", 0.9),
    ("f2", "contradicted", 0.8),
    ("f3", "supported", 0.5),
  ]);
  rig.fixture.route(JEV, Reply::new(200, jev)).unwrap();
  let rechecks = json!({"rechecks": [
    {"id": "f1", "decision": "dismiss", "explanation": "the empty slice returns before the index", "evidence_quotes": [GUARD]},
    {"id": "f2", "decision": "dismiss", "explanation": "the empty slice returns before the index", "evidence_quotes": ["if items.len() == 0"]}
  ]});
  rig
    .fixture
    .sequence_when(
      REVIEW,
      "code_review_recheck",
      vec![Reply::new(200, completion(&rechecks))],
    )
    .unwrap();
  let records = RefCell::new(Vec::new());
  let recorder = |record: &toolu_code_review::review::Record<'_>| {
    records.borrow_mut().push((
      record.stage,
      record.request.to_owned(),
      record.response.to_owned(),
    ));
  };
  let options = Options {
    recorder: Some(&recorder),
    ..rig.options()
  };
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::Medium), &options)
    .unwrap();
  let kept: Vec<(&str, Assessment)> = reviewed
    .findings
    .iter()
    .map(|item| (item.finding.claim.as_str(), item.assessment))
    .collect();
  assert_eq!(
    kept,
    [
      ("claim zero", Assessment::Supported),
      ("claim two", Assessment::Contradicted),
      ("claim three", Assessment::Insufficient)
    ]
  );
  assert_eq!(reviewed.dismissed.len(), 1);
  assert_eq!(reviewed.dismissed[0].finding.claim, "claim one");
  assert_eq!(reviewed.dismissed[0].quotes, [GUARD]);
  let stages: Vec<Stage> = reviewed.calls.iter().map(|call| call.stage).collect();
  assert_eq!(stages, [Stage::Review, Stage::Judge, Stage::Recheck]);
  assert_eq!(
    (reviewed.usage.tokens_in, reviewed.usage.tokens_out),
    (230, 83)
  );
  assert_eq!(reviewed.usage.cost_usd, Cost::usd(0.022));
  let records = records.into_inner();
  assert_eq!(records.len(), 3);
  assert!(
    records
      .iter()
      .all(|(_, request, response)| !request.contains(KEY) && !response.contains(KEY))
  );
  assert!(records[1].1.contains("typesafe/jev-1.13") && records[1].2.contains("\"answers\""));
  let log = serde_json::to_string(&reviewed.calls).unwrap();
  assert!(!log.contains(KEY) && !log.contains("items.len()"), "{log}");
}

#[test]
fn a_jev_outage_keeps_the_baseline_and_sends_no_recheck() {
  let rig = rig();
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

#[test]
fn oversized_evidence_is_unavailable_and_never_sent() {
  let big = lines(
    1350,
    "let value_with_a_fairly_long_name = compute_something_expensive();",
  );
  let rig = Rig::new(
    &[("src/big.rs", "fn a() {}\n".to_owned())],
    &[("src/big.rs", big)],
  )
  .unwrap();
  rig
    .fixture
    .route(
      REVIEW,
      Reply::new(
        200,
        findings(&[finding("src/big.rs", "high", "claim", None)]),
      ),
    )
    .unwrap();
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::Medium), &rig.options())
    .unwrap();
  assert_eq!(reviewed.findings[0].assessment, Assessment::Unavailable);
  assert!(
    reviewed
      .notes
      .iter()
      .any(|note| note.contains("evidence over the 90000-byte cap")),
    "{:?}",
    reviewed.notes
  );
  assert!(rig.bodies(JEV).is_empty());
}

#[test]
fn independent_questions_over_one_request_are_split() {
  let patch = lines(
    700,
    "let value_with_a_fairly_long_name = compute_something_expensive();",
  );
  let rig = Rig::new(
    &[("src/big.rs", "fn a() {}\n".to_owned())],
    &[("src/big.rs", patch)],
  )
  .unwrap();
  let quote = "q".repeat(4000);
  let many: Vec<_> = (0..40)
    .map(|index| {
      let mut item = finding("src/big.rs", "high", &format!("claim {index}"), None);
      item["quote"] = json!(quote);
      item
    })
    .collect();
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&many)))
    .unwrap();
  rig.fixture.route(JEV, Reply::new(500, "down")).unwrap();
  let options = Options {
    judge_deadline: Some(Duration::from_secs(30)),
    ..rig.options()
  };
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::Medium), &options)
    .unwrap();
  let bodies = rig.bodies(JEV);
  assert!(
    bodies.len() >= 2 * 3,
    "at least two requests, three attempts each: {}",
    bodies.len()
  );
  assert!(bodies.iter().all(|body| body.len() <= 180_000));
  assert_eq!(reviewed.findings.len(), 40);
}

#[test]
fn an_expired_judge_deadline_keeps_the_findings() {
  let rig = rig();
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
  let slow = choices(&[("f0", "contradicted", 0.9)]);
  rig
    .fixture
    .route(JEV, Reply::new(200, slow).delayed(Duration::from_secs(4)))
    .unwrap();
  let options = Options {
    judge_deadline: Some(Duration::from_millis(400)),
    ..rig.options()
  };
  let started = Instant::now();
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::Medium), &options)
    .unwrap();
  assert!(
    started.elapsed() < Duration::from_secs(3),
    "{:?}",
    started.elapsed()
  );
  assert_eq!(reviewed.findings[0].assessment, Assessment::Unavailable);
  assert!(
    reviewed
      .notes
      .contains(&"jev unavailable for src/lib.rs: deadline".to_owned()),
    "{:?}",
    reviewed.notes
  );
}

#[test]
fn small_judges_blocking_findings_only_and_never_rechecks() {
  let rig = rig();
  let model = [
    finding("src/lib.rs", "nit", "style", None),
    finding("src/lib.rs", "high", "bug", None),
  ];
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&model)))
    .unwrap();
  rig
    .fixture
    .route(
      JEV,
      Reply::new(200, choices(&[("f1", "contradicted", 0.9)])),
    )
    .unwrap();
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::Small), &rig.options())
    .unwrap();
  let jev = rig.bodies(JEV);
  assert_eq!(jev.len(), 1);
  assert!(jev[0].contains("\"f1\"") && !jev[0].contains("\"f0\""));
  assert_eq!(rig.bodies(REVIEW).len(), 1, "no recheck at small");
  let kept: Vec<Assessment> = reviewed
    .findings
    .iter()
    .map(|item| item.assessment)
    .collect();
  assert_eq!(kept, [Assessment::NotJudged, Assessment::Contradicted]);
}

#[test]
fn memory_sends_open_findings_and_dismissals_and_skips_re_raised_ones() {
  let rig = rig();
  let prior = |id: &str, status| PriorFinding {
    fingerprint: id.into(),
    id: id.into(),
    path: "src/lib.rs".into(),
    category: "correctness".into(),
    claim: format!("prior claim {id}"),
    severity: Severity::High,
    status,
  };
  let memory = Memory {
    prior: vec![
      prior("p-open", Status::Open),
      prior("p-done", Status::Resolved),
      prior("p-gone", Status::Dismissed),
    ],
    dismissals: vec![Dismissal {
      id: "p-gone".into(),
      reason: "guarded by the caller".into(),
      by: "agent".into(),
    }],
  };
  let model = [
    finding("src/lib.rs", "high", "re-raised", Some("p-open")),
    finding("src/lib.rs", "high", "fresh", None),
  ];
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&model)))
    .unwrap();
  rig
    .fixture
    .route(
      JEV,
      Reply::new(200, choices(&[("f1", "contradicted", 0.9)])),
    )
    .unwrap();
  let keep = json!({"rechecks": [{"id": "f1", "decision": "keep", "explanation": "", "evidence_quotes": []}]});
  rig
    .fixture
    .sequence_when(
      REVIEW,
      "code_review_recheck",
      vec![Reply::new(200, completion(&keep))],
    )
    .unwrap();
  let reviewed = rig
    .run(&memory, Some(Effort::Medium), &rig.options())
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

/// A Jev reply choosing, per key, `choice` with probability `p` (the rest spread).
fn choices(answers: &[(&str, &str, f64)]) -> String {
  let answers: serde_json::Map<String, Value> = answers
    .iter()
    .map(|(key, choice, p)| {
      let others = ["supported", "contradicted", "insufficient"]
        .into_iter()
        .filter(|option| option != choice);
      let mut probabilities = serde_json::Map::new();
      probabilities.insert((*choice).to_owned(), json!(p));
      for other in others {
        probabilities.insert(other.to_owned(), json!((1.0 - p) / 2.0));
      }
      let answer = json!({"type": "choice", "choice": choice, "probabilities": probabilities, "confidence": p});
      ((*key).to_owned(), answer)
    })
    .collect();
  json!({"model": "typesafe/jev-1.13", "usage": {"input_tokens": 30, "output_tokens": 3, "cost": 0.002},
         "answers": answers})
  .to_string()
}

/// Lines `prefix 0` .. `prefix count-1`, one per line.
fn lines(count: usize, prefix: &str) -> String {
  (0..count)
    .map(|index| format!("{prefix} {index}\n"))
    .collect()
}
