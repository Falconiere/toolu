use serde_json::Value;

use super::{Fixer, safe};
use crate::fixture;

#[test]
fn fixer_argv_matches_the_recorded_host_contract_for_32_combinations() {
  let golden = fixture::json("hosts/fixer-hosts.json");
  let name = golden["name"].as_str().unwrap();
  let cases = golden["argv"].as_array().unwrap();
  assert_eq!(cases.len(), 32);
  for case in cases {
    let host = Fixer::parse(case["host"].as_str().unwrap()).unwrap();
    let args = host
      .args(
        name,
        case["model"].as_str(),
        case["effort"].as_str(),
        case["unattended"].as_bool().unwrap(),
      )
      .unwrap();
    let expected: Vec<&str> = case["argv"]
      .as_array()
      .unwrap()
      .iter()
      .map(|arg| arg.as_str().unwrap())
      .collect();
    assert_eq!(args, expected, "{case}");
  }
}

/// `{model?, effort?}`, the routing table's shape.
fn choice(choice: &super::Choice) -> Value {
  let mut out = serde_json::Map::new();
  if let Some(model) = &choice.model {
    out.insert("model".into(), Value::from(model.as_str()));
  }
  if let Some(effort) = &choice.effort {
    out.insert("effort".into(), Value::from(effort.as_str()));
  }
  Value::Object(out)
}

#[test]
fn defaults_and_aliases_match_the_recorded_routing_table() {
  let golden = fixture::json("hosts/fixer-hosts.json");
  for host in Fixer::ALL {
    let table: Vec<Value> = host.defaults().iter().map(choice).collect();
    assert_eq!(
      Value::from(table),
      golden["routing"][host.name()],
      "{host:?}"
    );
  }
  for (alias, kind) in golden["aliases"].as_object().unwrap() {
    assert_eq!(
      Fixer::parse(alias).unwrap().name(),
      kind.as_str().unwrap(),
      "{alias}"
    );
  }
  assert_eq!(Fixer::Cursor.cli(), "cursor-agent");
  assert_eq!(Fixer::Codex.cli(), "codex");
  let unknown = Fixer::parse("gemini").err().unwrap();
  assert_eq!(unknown.code, "config_invalid");
}

#[test]
fn shell_unsafe_model_identifiers_are_rejected_before_launch() {
  for (host, model) in [
    (Fixer::Claude, "a b"),
    (Fixer::Cursor, "claude[effort=high]"),
  ] {
    let failure = host.args("pb-1", Some(model), None, true).err().unwrap();
    assert!(failure.message.contains("unsafe"), "{}", failure.message);
  }
  assert!(safe("anthropic/claude-sonnet-4.5"));
  assert!(!safe("$(rm)"));
}
