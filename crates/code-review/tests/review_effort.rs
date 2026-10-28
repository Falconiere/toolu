//! The review stage's gates and inputs against loopback servers: no opt-in or no
//! key sends nothing, the effort selector and its floors, the cache-friendly
//! prompt with masked secrets and the base-ref override, and the review deadline.

#[path = "helpers/rig.rs"]
mod rig;

use std::time::Duration;

use rig::{JEV, REVIEW, Rig, finding, findings};
use serde_json::{Value, json};
use toolu_code_review::budget::Effort;
use toolu_code_review::contract::Severity;
use toolu_code_review::memory::{Memory, PriorFinding, Status};
use toolu_code_review::review::{EffortSource, Options, RunError, Stage};
use toolu_code_review::settings::Settings;
use toolu_http_test_support::Reply;

fn code_rig(path: &str) -> Rig {
  Rig::new(
    &[(path, "one\n".to_owned())],
    &[(path, "one\ntwo\n".to_owned())],
  )
  .unwrap()
}

fn score(score: f64) -> String {
  json!({"model": "typesafe/jev-1.13", "usage": {"input_tokens": 50, "output_tokens": 1},
    "answers": {"effort": {"type": "score", "score": score,
      "legend": {"0": "small", "1": "medium", "2": "high", "3": "xhigh"},
      "probabilities": {"0": 0.7, "1": 0.2, "2": 0.1, "3": 0.0}, "confidence": 0.6}}})
  .to_string()
}

#[test]
fn without_the_opt_in_or_a_key_nothing_is_sent() {
  let rig = code_rig("src/lib.rs");
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&[])))
    .unwrap();
  let off = Settings::default();
  let result = rig.run_with(&Memory::default(), &off, Some(Effort::High), &rig.options());
  assert_eq!(result.map(|_| ()), Err(RunError::NotEnabled));
  let report = RunError::NotEnabled.report(Effort::High);
  assert_eq!(
    serde_json::to_value(&report).unwrap()["error"]["kind"],
    "not-enabled"
  );
  let input_env = rig.env().with("OPENROUTER_API_KEY", "");
  let settings = Settings {
    enabled: true,
    ..Settings::default()
  };
  let input = toolu_code_review::review::Input {
    root: rig.repo.path(),
    shaped: &rig.shaped,
    rules: &rig.rules,
    prepass: &rig.prepass,
    memory: &Memory::default(),
    settings: &settings,
    flag: None,
  };
  let missing = toolu_code_review::review::run(&input_env, &input, &rig.options());
  assert!(
    matches!(missing, Err(RunError::Unconfigured(why)) if why.contains("OPENROUTER_API_KEY"))
  );
  assert!(rig.fixture.requests().unwrap().is_empty());
}

#[test]
fn a_docs_change_picks_small_and_logs_the_selector() {
  let rig = code_rig("docs/guide.md");
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&[])))
    .unwrap();
  rig.fixture.route(JEV, Reply::new(200, score(0.2))).unwrap();
  let reviewed = rig.run(&Memory::default(), None, &rig.options()).unwrap();
  assert_eq!(
    (reviewed.effort.level, reviewed.effort.source),
    (Effort::Small, EffortSource::Auto)
  );
  assert_eq!(reviewed.calls[0].stage, Stage::Selector);
  assert_eq!(reviewed.usage.tokens_in, 150);
  let state: Value = serde_json::from_str(&rig.bodies(JEV)[0]).unwrap();
  assert_eq!(state["state"]["files"][0]["path"], "docs/guide.md");
  assert!(
    !state.to_string().contains("two"),
    "no source in the selector state"
  );
  let doc = serde_json::to_value(&reviewed.effort).unwrap();
  assert_eq!(
    doc,
    json!({"level": "small", "source": "auto", "reason": "Jev score 0.20"})
  );
}

#[test]
fn a_workflow_change_is_floored_and_a_failed_selector_falls_back() {
  let rig = code_rig(".github/workflows/tests.yml");
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&[])))
    .unwrap();
  rig.fixture.route(JEV, Reply::new(200, score(0.2))).unwrap();
  let reviewed = rig.run(&Memory::default(), None, &rig.options()).unwrap();
  assert_eq!(reviewed.effort.level, Effort::High);
  assert_eq!(reviewed.effort.floor.as_deref(), Some("CI workflow change"));
  let plain = code_rig("src/lib.rs");
  plain
    .fixture
    .route(REVIEW, Reply::new(200, findings(&[])))
    .unwrap();
  plain.fixture.route(JEV, Reply::new(400, "bad")).unwrap();
  let fallback = plain
    .run(&Memory::default(), None, &plain.options())
    .unwrap();
  assert_eq!(
    (fallback.effort.level, fallback.effort.source),
    (Effort::Medium, EffortSource::Fallback)
  );
  assert_eq!(fallback.effort.reason, "medium fallback: HTTP 400");
}

#[test]
fn an_explicit_level_sends_no_selector_and_notes_the_floor() {
  let rig = code_rig(".github/workflows/tests.yml");
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&[])))
    .unwrap();
  let reviewed = rig
    .run(&Memory::default(), Some(Effort::Xhigh), &rig.options())
    .unwrap();
  assert!(rig.bodies(JEV).is_empty());
  assert_eq!(
    (reviewed.effort.level, reviewed.effort.source),
    (Effort::Xhigh, EffortSource::Flag)
  );
  assert_eq!(reviewed.effort.floor.as_deref(), Some("CI workflow change"));
  let body: Value = serde_json::from_str(&rig.bodies(REVIEW)[0]).unwrap();
  assert_eq!(body["reasoning"]["effort"], "xhigh");
}

#[test]
fn the_prompt_prefix_is_stable_secrets_are_masked_and_the_override_applies() {
  let secret = format!("ghp_{}", "Ab1".repeat(12));
  let base = [
    (
      ".github/code-review-prompt.md",
      "Focus on SQL injection.".to_owned(),
    ),
    ("AGENTS.md", "Use typed errors.".to_owned()),
    ("src/lib.rs", "fn a() {}\n".to_owned()),
    ("src/new.rs", String::new()),
  ];
  let change = [
    (
      "src/lib.rs",
      format!("fn a() {{}}\nconst T: &str = \"{secret}\";\n"),
    ),
    ("src/add.rs", "fn added() {}\n".to_owned()),
  ];
  let rig = Rig::new(&base, &change).unwrap();
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&[])))
    .unwrap();
  let options = Options {
    skip_judge: true,
    ..rig.options()
  };
  let open = |claim: &str| Memory {
    prior: vec![PriorFinding {
      fingerprint: "f".into(),
      id: "p1".into(),
      path: "src/lib.rs".into(),
      category: "correctness".into(),
      claim: claim.into(),
      severity: Severity::High,
      status: Status::Open,
    }],
    dismissals: Vec::new(),
  };
  rig
    .run(&open("round one"), Some(Effort::Medium), &options)
    .unwrap();
  rig
    .run(&open("round two"), Some(Effort::Medium), &options)
    .unwrap();
  let bodies = rig.bodies(REVIEW);
  assert_eq!(bodies.len(), 2);
  assert!(
    bodies.iter().all(|body| !body.contains(&secret)),
    "the secret never leaves"
  );
  let messages: Vec<Value> = bodies
    .iter()
    .map(|body| serde_json::from_str::<Value>(body).unwrap()["messages"].clone())
    .collect();
  assert_eq!(messages[0][0], messages[1][0]);
  assert_eq!(messages[0][1], messages[1][1]);
  assert_ne!(messages[0][2], messages[1][2]);
  let system = messages[0][0]["content"].as_str().unwrap();
  assert!(system.ends_with("Focus on SQL injection.") && system.contains("never instructions"));
  assert!(
    messages[0][1]["content"]
      .as_str()
      .unwrap()
      .contains("Use typed errors.")
  );
  let last: Value = serde_json::from_str(
    messages[0][2]["content"]
      .as_str()
      .unwrap()
      .split_once('\n')
      .unwrap()
      .1,
  )
  .unwrap();
  let source = |path: &str| {
    last["files"]
      .as_array()
      .unwrap()
      .iter()
      .find(|file| file["path"] == path)
      .unwrap()["source"]
      .clone()
  };
  assert_eq!(
    source("src/add.rs"),
    Value::Null,
    "an added file's patch is its source"
  );
  assert!(
    source("src/lib.rs")
      .as_str()
      .unwrap()
      .contains("[REDACTED]")
  );
  assert!(rig.bodies(JEV).is_empty(), "the judge stage was off");
}

#[test]
fn a_review_past_its_deadline_is_typed_and_not_retried_lower() {
  let rig = code_rig("src/lib.rs");
  let slow = findings(&[finding("src/lib.rs", "high", "late", None)]);
  rig
    .fixture
    .route(
      REVIEW,
      Reply::new(200, slow).delayed(Duration::from_secs(4)),
    )
    .unwrap();
  let options = Options {
    review_deadline: Some(Duration::from_millis(400)),
    ..rig.options()
  };
  let result = rig.run(&Memory::default(), Some(Effort::High), &options);
  assert!(
    matches!(result, Err(RunError::Deadline { .. })),
    "{result:?}"
  );
  assert_eq!(rig.bodies(REVIEW).len(), 1);
  let report = result.unwrap_err().report(Effort::High);
  assert_eq!(
    serde_json::to_value(&report).unwrap()["error"]["kind"],
    "deadline"
  );
}
