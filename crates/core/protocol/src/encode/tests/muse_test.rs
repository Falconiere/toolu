//! Muse output shapes for each normalized decision.

use serde_json::json;

use super::output;
use crate::encode::Normal;
use crate::encode::tests::parsed;

#[test]
fn allow_is_silent() {
  assert_eq!(output(Normal::Allow), "");
}

#[test]
fn a_deny_or_a_post_block_is_the_exit_0_block_line() {
  assert_eq!(
    output(Normal::Deny("no .env writes")),
    "{\"decision\":\"block\",\"reason\":\"no .env writes\"}\n"
  );
  assert_eq!(
    parsed(&output(Normal::Block("lint failed"))),
    json!({ "decision": "block", "reason": "lint failed" })
  );
}

#[test]
fn advice_is_a_system_message_never_additional_context() {
  assert_eq!(
    output(Normal::Advisory("prefer rg")),
    "{\"systemMessage\":\"prefer rg\"}\n"
  );
}

#[test]
fn an_ask_that_reaches_the_encoder_fails_closed_to_the_block_line() {
  assert_eq!(
    parsed(&output(Normal::Ask("confirm .env write"))),
    json!({ "decision": "block", "reason": "confirm .env write" })
  );
}

#[test]
fn reasons_are_escaped_as_json_strings() {
  assert_eq!(
    output(Normal::Deny("a \"quoted\"\npath")),
    "{\"decision\":\"block\",\"reason\":\"a \\\"quoted\\\"\\npath\"}\n"
  );
}
