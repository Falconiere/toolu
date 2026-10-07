use std::time::Duration;

use super::{BUN_TIMEOUT, Check, Status, document, failure_line, host, runtime, text};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

#[test]
fn failure_line_pluralizes_one_check() {
  assert_eq!(failure_line(1), "toolu doctor: 1 check failed");
  assert_eq!(failure_line(2), "toolu doctor: 2 checks failed");
}

#[test]
fn text_lists_a_hint_under_the_check() {
  let checks = [Check::new(
    "tools",
    Status::Fail,
    "pr-babysit needs gh",
    Some("install gh".to_owned()),
    serde_json::json!({}),
  )];
  let rendered = text(&checks);
  assert_eq!(
    rendered,
    "fail tools: pr-babysit needs gh\n  fix: install gh"
  );
}

#[test]
fn document_is_not_ok_when_a_check_fails() {
  let checks = [Check::new(
    "config",
    Status::Fail,
    "bad",
    None,
    serde_json::json!({}),
  )];
  let document = document(&checks);
  assert_eq!(document["namespace"], "doctor");
  assert_eq!(document["ok"], false);
  assert_eq!(document["checks"][0]["status"], "fail");
  assert!(document["checks"][0]["hint"].is_null());
}

#[test]
fn a_warn_does_not_clear_ok() {
  let checks = [Check::new(
    "runtime",
    Status::Warn,
    "Bun is missing",
    None,
    serde_json::json!({}),
  )];
  assert_eq!(document(&checks)["ok"], true);
}

#[test]
fn runtime_warns_when_bun_is_not_on_path() {
  let env = Env::from_pairs([("PATH", "/nonexistent")]);
  let check = runtime(&env);
  assert_eq!(check.status, Status::Warn);
  assert_eq!(check.hint.as_deref(), Some("install Bun 1.4.x"));
  assert_eq!(BUN_TIMEOUT, Duration::from_secs(1));
}

#[test]
fn host_reports_the_bound_host_and_roots() {
  let home = tempfile::tempdir().unwrap();
  let roots = Roots::new(
    Env::from_pairs([
      ("HOME", home.path().to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.path().to_str().unwrap()),
    ]),
    Some(Host::Claude),
  );
  let check = host(&roots, home.path());
  assert_eq!(check.status, Status::Ok);
  assert_eq!(check.details["host"], "claude");
  assert!(check.summary.contains("project"));
}
