use serde_json::Value;

use super::{SCHEMA_ID, tree};
use crate::tree as full;

fn exported() -> Value {
  tree(&full::command())
}

fn command<'a>(doc: &'a Value, path: &[&str]) -> &'a Value {
  path.iter().fold(doc, |node, name| {
    node["commands"]
      .as_array()
      .unwrap()
      .iter()
      .find(|child| child["name"] == *name)
      .unwrap_or_else(|| panic!("no command {name}"))
  })
}

fn longs(node: &Value) -> Vec<&str> {
  node["flags"]
    .as_array()
    .unwrap()
    .iter()
    .map(|flag| flag["long"].as_str().unwrap())
    .collect()
}

#[test]
fn the_document_names_its_schema_and_carries_no_version() {
  let doc = exported();
  assert_eq!(doc["schema"], SCHEMA_ID);
  assert_eq!(doc["hookProtocol"], 1);
  assert!(doc.get("version").is_none());
  assert_eq!(doc["exitCodes"].as_array().unwrap().len(), 6);
  assert_eq!(doc["exitCodes"][3]["code"], 64);
}

#[test]
fn root_flags_are_the_globals_version_and_the_hidden_probe() {
  let doc = exported();
  assert_eq!(
    longs(&doc),
    [
      "json",
      "quiet",
      "host",
      "config-dir",
      "hook-protocol",
      "version"
    ]
  );
  let host = &doc["flags"][2];
  assert_eq!(host["valueName"], "HOST");
  assert_eq!(
    host["possibleValues"],
    serde_json::json!(["claude", "codex", "opencode", "cursor", "hermes"])
  );
  assert_eq!(doc["flags"][1]["short"], "q");
  assert_eq!(doc["flags"][4]["hidden"], true);
  assert_eq!(doc["flags"][0]["possibleValues"], serde_json::json!([]));
}

#[test]
fn namespaces_carry_owner_aliases_placeholders_and_hidden_hooks() {
  let doc = exported();
  let epic = command(&doc, &["epic"]);
  assert_eq!(epic["owner"], "epic-orchestrator");
  assert_eq!(epic["aliases"], serde_json::json!(["epic-orchestrator"]));
  assert_eq!(epic["path"], serde_json::json!(["epic"]));
  let planned = command(&doc, &["epic", "planned"]);
  assert_eq!(planned["placeholder"], true);
  assert_eq!(planned["path"], serde_json::json!(["epic", "planned"]));
  let hook = command(&doc, &["epic", "hook"]);
  assert_eq!(hook["hidden"], true);
  assert_eq!(hook["owner"], "epic-orchestrator");
  assert_eq!(hook["args"][0]["name"], "NAME");
  assert_eq!(hook["args"][0]["required"], true);
  assert_eq!(longs(hook), ["event", "plugin-root"]);
}

#[test]
fn subcommands_repeat_neither_globals_nor_the_help_command() {
  let doc = exported();
  let planned = command(&doc, &["jev", "planned"]);
  assert_eq!(longs(planned), Vec::<&str>::new());
  let names: Vec<&str> = command(&doc, &["jev"])["commands"]
    .as_array()
    .unwrap()
    .iter()
    .map(|child| child["name"].as_str().unwrap())
    .collect();
  assert_eq!(names, ["planned", "hook"]);
  assert_eq!(command(&doc, &["commands"])["owner"], "toolu-cli");
  assert_eq!(command(&doc, &["hook"])["owner"], "toolu");
}
