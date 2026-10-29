use super::{address, decode, process_error, qualified, require_owner, status, validate_target};
use serde_json::json;
use std::time::Duration;
use toolu_runtime::process::{Output, RunError};

fn output(text: &str) -> Output {
  Output {
    pid: 1,
    stdout: text.into(),
    stdout_bytes: text.as_bytes().to_vec(),
    stderr: String::new(),
    exit_code: 0,
    duration: Duration::ZERO,
    timed_out: false,
    truncated: false,
  }
}

#[test]
fn preserves_machine_addresses_and_reported_status() {
  let terminal = json!({"address":"vps/t7", "state":"live", "status":{"state":"blocked"}, "labels":{"toolu.owner":"run-1"}});
  assert_eq!(address(&terminal).unwrap(), "vps/t7");
  assert_eq!(status(&terminal), "blocked");
  assert!(require_owner(&terminal, "toolu.owner=run-1").is_ok());
  assert!(require_owner(&terminal, "run-2").is_err());
  assert!(require_owner(&json!({"labels":{"toolu.owner":""}}), "").is_err());
  assert!(address(&json!({})).is_err());
  assert_eq!(status(&json!({"state":"live"})), "unknown");
  assert_eq!(status(&json!({"state":"finished"})), "done");
}

#[test]
fn maps_structured_errors_and_uncertain_transport() {
  let blocked = decode(&output(
    r#"{"error":{"kind":"blocked","message":"question"}}"#,
  ))
  .unwrap_err();
  assert_eq!(blocked.code, "blocked");
  assert!(!blocked.uncertain);
  let mut timeout = output("{}");
  timeout.timed_out = true;
  assert!(decode(&timeout).unwrap_err().uncertain);
  timeout.timed_out = false;
  timeout.truncated = true;
  assert!(decode(&timeout).unwrap_err().uncertain);
  assert!(decode(&output("broken")).unwrap_err().uncertain);
  assert_eq!(
    decode(&output(r#"{"address":"t1"}"#)).unwrap()["address"],
    "t1"
  );
}

#[test]
fn uncertainty_and_target_boundaries_are_explicit() {
  assert!(!process_error(&RunError::Spawn("missing".into())).uncertain);
  assert!(process_error(&RunError::Wait("lost".into())).uncertain);
  assert!(process_error(&RunError::Stdin("lost".into())).uncertain);
  assert!(
    decode(&output(r#"{"error":{"kind":"timeout"}}"#))
      .unwrap_err()
      .uncertain
  );
  let mut failed = output(r#"{"address":"t1"}"#);
  failed.exit_code = 1;
  assert!(decode(&failed).unwrap_err().uncertain);
  assert!(validate_target(&["get", "--help"]).is_err());
  assert!(validate_target(&["send", "-x", "--", "message"]).is_err());
  assert_eq!(qualified("t1", Some("vps")).unwrap(), "vps/t1");
  assert!(qualified("other/t1", Some("vps")).is_err());
}
