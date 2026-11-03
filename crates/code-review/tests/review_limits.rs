//! The judge's limits against loopback servers: evidence over the cap is never
//! sent, independent questions split under the request cap, an expired deadline
//! keeps findings, and `small` judges blocking findings only.

#[path = "helpers/host.rs"]
mod host;
#[path = "helpers/judge.rs"]
mod judge;
#[path = "helpers/rig.rs"]
mod rig;
#[path = "helpers/tools.rs"]
mod tools;

use std::time::{Duration, Instant};

use judge::{choices, rig};
use rig::{JEV, Rig, finding, findings};
use serde_json::json;
use toolu_code_review::budget::Effort;
use toolu_code_review::memory::Memory;
use toolu_code_review::review::{Assessment, Options};
use toolu_http_test_support::Reply;

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
  host::host(
    rig.bin.path(),
    "codex",
    &content(&findings(&[finding("src/big.rs", "high", "claim", None)])).unwrap(),
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
  assert_eq!(rig.bodies(JEV), Vec::<String>::new());
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
  host::host(rig.bin.path(), "codex", &content(&findings(&many)).unwrap()).unwrap();
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
  let rig = rig().unwrap();
  host::host(
    rig.bin.path(),
    "codex",
    &content(&findings(&[finding("src/lib.rs", "high", "claim", None)])).unwrap(),
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
  let rig = rig().unwrap();
  let model = [
    finding("src/lib.rs", "nit", "style", None),
    finding("src/lib.rs", "high", "bug", None),
  ];
  host::host(
    rig.bin.path(),
    "codex",
    &content(&findings(&model)).unwrap(),
  )
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
  assert_eq!(
    reviewed
      .calls
      .iter()
      .filter(|call| call.stage == toolu_code_review::review::Stage::Review)
      .count(),
    1
  );
  assert!(
    !reviewed
      .calls
      .iter()
      .any(|call| call.stage == toolu_code_review::review::Stage::Recheck)
  );
  let kept: Vec<Assessment> = reviewed
    .findings
    .iter()
    .map(|item| item.assessment)
    .collect();
  assert_eq!(kept, [Assessment::NotJudged, Assessment::Contradicted]);
}

/// Lines `prefix 0` .. `prefix count-1`, one per line.
fn lines(count: usize, prefix: &str) -> String {
  (0..count).fold(String::new(), |text, index| {
    text
      + prefix
      + " "
      + &index.to_string()
      + "
"
  })
}

fn content(body: &str) -> Option<String> {
  let doc: serde_json::Value = serde_json::from_str(body).ok()?;
  Some(
    doc
      .get("choices")?
      .get(0)?
      .get("message")?
      .get("content")?
      .as_str()?
      .to_owned(),
  )
}
