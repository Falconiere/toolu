use std::time::Duration;

use serde_json::json;
use toolu_runtime::env::Env;

use super::{Failure, argv, run_checked};
use crate::herdr_calls::CallError;

#[test]
fn details_name_the_outcome_and_the_native_error() {
  let limited = Failure::unknown("You've hit your usage limit");
  assert_eq!(limited.details()["lifecycle_outcome"], "provider-limited");
  let refused = Failure::refusal("no host capacity");
  let details = refused.details();
  assert_eq!(details["lifecycle_outcome"], "uncertain");
  assert_eq!(
    details["native_error"],
    json!({"exitCode": null, "code": null, "uncertain": false})
  );
  let herdr: Failure = CallError {
    message: "herdr agent.prompt: timeout: x".into(),
    code: Some("timeout".into()),
    exit_code: None,
    uncertain: true,
  }
  .into();
  assert_eq!(herdr.details()["native_error"]["code"], "timeout");
  assert!(!herdr.not_started());
  let busy: Failure = CallError {
    message:
      "herdr agent.start: agent_pane_busy: agent target pane wE0:p1 is not an available shell"
        .into(),
    code: Some("agent_pane_busy".into()),
    exit_code: None,
    uncertain: false,
  }
  .into();
  assert!(busy.not_started());
  assert_eq!(busy.details()["lifecycle_outcome"], "not-started");
  assert_eq!(
    herdr.report("r#1"),
    json!({"issue": "r#1", "error": "herdr agent.prompt: timeout: x", "native_exit_code": null})
  );
}

#[test]
fn a_checked_command_reports_its_line_status_and_output() {
  let env = Env::from_pairs([("PATH", "/usr/bin:/bin"), ("HOME", "/tmp")]);
  let ok = run_checked(
    &argv(&["sh", "-c", "printf hi"]),
    None,
    &env,
    Duration::from_secs(10),
  );
  assert_eq!(ok, Ok("hi".to_owned()));
  let failed = run_checked(
    &argv(&["sh", "-c", "echo nope >&2; exit 3"]),
    None,
    &env,
    Duration::from_secs(10),
  )
  .expect_err("exit");
  assert_eq!(
    failed.message,
    "sh -c echo nope >&2; exit 3 failed (3): nope"
  );
  assert_eq!((failed.exit_code, failed.uncertain), (Some(3), false));
  let empty = Env::from_pairs([("PATH", ""), ("HOME", "/tmp")]);
  let missing = run_checked(
    &argv(&["gh", "repo", "view"]),
    None,
    &empty,
    Duration::from_secs(10),
  )
  .expect_err("missing");
  assert!(
    missing.message.starts_with("gh repo view failed (spawn)"),
    "{}",
    missing.message
  );
  assert!(missing.uncertain && missing.exit_code.is_none());
}
