//! Live reviews use the host CLI and `TypeSafe` credentials, and fail closed without a host.

#[path = "helpers/rig.rs"]
mod rig;
#[path = "helpers/tools.rs"]
mod tools;

use std::os::unix::fs::PermissionsExt;

use rig::{REVIEW, Rig, finding, findings};
use toolu_code_review::budget::Effort;
use toolu_code_review::memory::Memory;
use toolu_code_review::review::{self, Input, Options, RunError};
use toolu_code_review::settings::Settings;
use toolu_http_test_support::Reply;

fn run_with_path(
  rig: &Rig,
  path: &str,
  openrouter_key: &str,
) -> Result<review::Reviewed, RunError> {
  let env = rig
    .env()
    .with("PATH", path)
    .with("OPENROUTER_API_KEY", openrouter_key)
    .with("TYPESAFE_API_KEY", "test-typesafe-key");
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
  review::run(&env, &input, &options)
}

fn rig() -> Result<Rig, String> {
  Rig::new(
    &[("src/lib.rs", "one\n".into())],
    &[("src/lib.rs", "one\ntwo\n".into())],
  )
}

#[test]
fn host_review_works_with_typesafe_key_and_no_openrouter_key() {
  let rig = rig().unwrap();
  let bin = tempfile::tempdir().unwrap();
  let codex = bin.path().join("codex");
  std::fs::write(&codex, "#!/bin/sh\nprintf '%s' '{\"findings\":[]}'\n").unwrap();
  std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).unwrap();
  let reviewed = run_with_path(&rig, &bin.path().to_string_lossy(), "").unwrap();
  assert_eq!(reviewed.unreviewed.len(), 0);
  assert!(
    rig
      .run(
        &Memory::default(),
        Some(Effort::Medium),
        &Options {
          skip_judge: true,
          ..rig.options()
        }
      )
      .is_ok()
  );
  assert_eq!(
    reviewed
      .calls
      .iter()
      .filter(|call| call.provider.as_deref() == Some("codex"))
      .count(),
    1
  );
  assert_eq!(rig.bodies(REVIEW).len(), 0);
}

#[test]
fn missing_host_fails_closed_without_sending_to_openrouter() {
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
  let empty_path = tempfile::tempdir().unwrap();
  let result = run_with_path(&rig, &empty_path.path().to_string_lossy(), rig::KEY);
  assert!(matches!(result, Err(RunError::Outage { .. })), "{result:?}");
  assert_eq!(rig.fixture.requests().unwrap().len(), 0);
}

#[test]
fn missing_typesafe_key_is_rejected_before_review() {
  let rig = rig().unwrap();
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
  let missing = review::run(
    &rig.env().with("TYPESAFE_API_KEY", ""),
    &input,
    &rig.options(),
  );
  assert!(
    matches!(missing, Err(RunError::Unconfigured(_))),
    "{missing:?}"
  );
  let off = Settings {
    enabled: false,
    ..Settings::default()
  };
  let disabled = rig.run_with(
    &Memory::default(),
    &off,
    Some(Effort::Medium),
    &rig.options(),
  );
  assert_eq!(disabled.map(|_| ()), Err(RunError::NotEnabled));
  assert_eq!(rig.fixture.requests().unwrap().len(), 0);
}

#[test]
fn failing_host_does_not_fall_back_to_openrouter() {
  let rig = rig().unwrap();
  rig
    .fixture
    .route(REVIEW, Reply::new(200, findings(&[])))
    .unwrap();
  let bin = tempfile::tempdir().unwrap();
  let codex = bin.path().join("codex");
  std::fs::write(&codex, "#!/bin/sh\nexit 1\n").unwrap();
  std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).unwrap();
  let result = run_with_path(&rig, &bin.path().to_string_lossy(), rig::KEY);
  assert!(matches!(result, Err(RunError::Outage { .. })), "{result:?}");
  assert_eq!(rig.fixture.requests().unwrap().len(), 0);
}

#[test]
fn no_host_records_the_masked_request_and_empty_response() {
  use std::cell::RefCell;
  use toolu_code_review::review::{Record, Stage};
  let secret = format!("{}{}", "ghp_", "Ab1".repeat(12));
  let rig = Rig::new(
    &[("src/lib.rs", "old\n".into())],
    &[("src/lib.rs", format!("const TOKEN: &str = \"{secret}\";\n"))],
  )
  .unwrap();
  let empty_path = tempfile::tempdir().unwrap();
  let env = rig
    .env()
    .with("PATH", &empty_path.path().to_string_lossy())
    .with("TYPESAFE_API_KEY", "test-typesafe-key");
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
  let records = RefCell::new(Vec::new());
  let recorder = |record: &Record<'_>| {
    records.borrow_mut().push((
      record.stage,
      record.request.to_owned(),
      record.response.to_owned(),
    ));
  };
  let options = Options {
    skip_judge: true,
    recorder: Some(&recorder),
    ..rig.options()
  };
  let result = review::run(&env, &input, &options);
  assert!(matches!(result, Err(RunError::Outage { .. })), "{result:?}");
  let records = records.into_inner();
  assert_eq!(records.len(), 1);
  assert_eq!(records[0].0, Stage::Review);
  let request: serde_json::Value = serde_json::from_str(&records[0].1).unwrap();
  assert!(request["messages"].is_array());
  assert!(!records[0].1.contains(secret.as_str()));
  assert_eq!(records[0].2, "");
  assert_eq!(rig.fixture.requests().unwrap().len(), 0);
}
