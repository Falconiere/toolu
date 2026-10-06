//! `OpenCode` callbacks for each normalized decision.

use super::callback;
use crate::encode::{Callback, CallbackAction, Normal};

fn of(action: CallbackAction, message: Option<&str>) -> Callback {
  Callback {
    action,
    message: message.map(str::to_owned),
  }
}

#[test]
fn a_refusal_throws_and_never_grants_a_permission() {
  let throw = |text| of(CallbackAction::Throw, Some(text));
  assert_eq!(
    callback(Normal::Ask("confirm .env write")),
    throw("confirm .env write")
  );
  assert_eq!(
    callback(Normal::Deny("protected file")),
    throw("protected file")
  );
}

#[test]
fn everything_else_continues_with_its_advice() {
  let go = |text| of(CallbackAction::Continue, text);
  assert_eq!(
    callback(Normal::Block("lint failed")),
    go(Some("lint failed"))
  );
  assert_eq!(
    callback(Normal::Advisory("run the tests")),
    go(Some("run the tests"))
  );
  assert_eq!(callback(Normal::Allow), go(None));
}

#[test]
fn a_callback_writes_the_typescript_object() {
  let throw = of(CallbackAction::Throw, Some("protected file"));
  assert_eq!(
    throw.json(),
    r#"{"kind":"callback","action":"throw","message":"protected file"}"#
  );
  let go = of(CallbackAction::Continue, None);
  assert_eq!(go.json(), r#"{"kind":"callback","action":"continue"}"#);
}
