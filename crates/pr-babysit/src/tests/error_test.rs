use serde_json::{Value, json};
use toolu_protocol::exit::Exit;

use super::{Failure, finish};

#[test]
fn codes_map_to_the_sysexits_exits() {
  for (code, exit) in [
    ("usage", Exit::Usage),
    ("gh_unavailable", Exit::Unavailable),
    ("herdr_unavailable", Exit::Unavailable),
    ("locked", Exit::TempFail),
    ("api_error", Exit::TempFail),
    ("head_moved", Exit::TempFail),
    ("resolve_unconfirmed", Exit::TempFail),
    ("duplicate_reply", Exit::Failure),
    ("state_malformed", Exit::Failure),
    ("worktree_dirty", Exit::Failure),
  ] {
    assert_eq!(Failure::new(code, "m").exit(), exit, "{code}");
  }
}

#[test]
fn the_document_carries_code_message_and_extras() {
  let failure = Failure::new("locked", "slot is held")
    .with("pid", 9103)
    .with("since", Value::Null);
  assert_eq!(
    failure.document(),
    json!({"version":1,"errors":[{"code":"locked","message":"slot is held","pid":9103,"since":null}]})
  );
  let outcome = failure.outcome();
  assert_eq!(outcome.exit, Exit::TempFail);
  let printed: Value = serde_json::from_str(outcome.stdout.as_deref().unwrap()).unwrap();
  assert_eq!(printed, failure.document());
  assert_eq!(failure.to_string(), "locked: slot is held");
}

#[test]
fn finish_prints_a_value_or_the_failure() {
  let ok = finish(Ok(json!({"a":1})));
  assert_eq!(ok.exit, Exit::Success);
  assert_eq!(ok.stdout.as_deref(), Some(r#"{"a":1}"#));
  let failed = finish(Err(Failure::usage("--pr <n> required")));
  assert_eq!(failed.exit, Exit::Usage);
  assert!(failed.stdout.unwrap().contains(r#""code":"usage""#));
}
