use serde_json::{Value, json};
use toolu_protocol::exit::Exit;

use super::{envelope, first_line, outcome, summary};
use crate::tree;

fn error_of(argv: &[&str]) -> clap::Error {
  tree::command().try_get_matches_from(argv).unwrap_err()
}

#[test]
fn help_is_data_and_a_json_document_under_json() {
  let text = outcome(&error_of(&["toolu", "--help"]), false);
  assert_eq!(text.exit, Exit::Success);
  assert!(text.stdout.unwrap().contains("Usage: toolu"));
  let json = outcome(&error_of(&["toolu", "--help"]), true);
  let doc: Value = serde_json::from_str(&json.stdout.unwrap()).unwrap();
  assert!(doc["help"].as_str().unwrap().contains("Exit codes:"));
}

#[test]
fn an_unknown_flag_is_a_usage_error_that_suggests_the_flag() {
  let usage = outcome(&error_of(&["toolu", "--jsn", "commands"]), true);
  assert_eq!(usage.exit, Exit::Usage);
  let doc: Value = serde_json::from_str(&usage.stdout.unwrap()).unwrap();
  assert_eq!(doc["error"]["suggestion"], "--json");
  assert!(
    usage
      .stderr
      .unwrap()
      .starts_with("error: unexpected argument")
  );
}

#[test]
fn the_envelope_names_the_exit() {
  let doc: Value = serde_json::from_str(&envelope(Exit::TempFail, "rate limited", None)).unwrap();
  assert_eq!(
    doc,
    json!({ "error": { "code": 75, "name": "tempfail", "message": "rate limited", "suggestion": null } })
  );
}

/// The `error.message` of `argv`'s usage error under `--json`.
fn message_of(argv: &[&str]) -> Value {
  let usage = outcome(&error_of(argv), true);
  assert_eq!(usage.exit, Exit::Usage);
  let doc: Value = serde_json::from_str(&usage.stdout.unwrap()).unwrap();
  doc["error"].clone()
}

#[test]
fn the_message_is_clap_s_whole_first_paragraph_on_one_line() {
  assert_eq!(
    message_of(&["toolu", "--json", "hook"])["message"],
    "the following required arguments were not provided: <NAME>"
  );
  let host = message_of(&["toolu", "--json", "--host", "cladue", "commands"]);
  assert_eq!(
    host["message"],
    "invalid value 'cladue' for '--host <HOST>' \
     [possible values: claude, codex, opencode, cursor, hermes]"
  );
  assert_eq!(host["suggestion"], "claude");
}

#[test]
fn a_missing_verb_names_the_usage_not_the_namespace_about() {
  assert_eq!(
    message_of(&["toolu", "--json", "epic"])["message"],
    "a command is required: toolu epic [OPTIONS] <COMMAND>"
  );
}

#[test]
fn the_summary_drops_the_error_prefix_and_stops_at_the_first_blank_line() {
  assert_eq!(
    summary("error: no such verb\n  <X>\n\nmore"),
    "no such verb <X>"
  );
  assert_eq!(summary("plain"), "plain");
  assert_eq!(summary(""), "");
}

#[test]
fn the_first_line_drops_the_error_prefix() {
  assert_eq!(first_line("error: no such verb\n\nmore"), "no such verb");
  assert_eq!(first_line("plain"), "plain");
  assert_eq!(first_line(""), "");
}
