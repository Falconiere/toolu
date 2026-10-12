use serde_json::Value;
use toolu_runtime::env::Env;

use super::{RoutingTable, default_table};
use crate::hosts::HOST_KINDS;
use crate::hosts::HostKind;
use crate::route::load_table;

const FIXTURE: &str = include_str!("../../../../../fixtures/epic/hosts.json");

#[test]
fn the_default_table_matches_the_shared_fixture() {
  let doc: Value = serde_json::from_str(FIXTURE).expect("fixture");
  let case = doc["cases"]
    .as_array()
    .expect("cases")
    .iter()
    .find(|case| case["kind"] == "table")
    .expect("table");
  assert_eq!(hosts_json(&default_table()), case["hosts"]);
}

#[test]
fn routing_json_replaces_hosts_and_preferences() {
  let tmp = tempfile::tempdir().expect("temp");
  let file = tmp.path().join("routing.json");
  std::fs::write(
    &file,
    r#"{"hosts": {"codex": [{"model": "m1"}, {"model": "m2"}, {"model": "m3"}, {"model": "m4", "effort": "xhigh"}], "gemini": []},
       "prefer": {"critical": ["claude", "nope"]}}"#,
  )
  .expect("routing");
  let env = Env::from_pairs([("EPIC_ROUTING_FILE", file.display().to_string())]);
  let table = load_table(&env);
  assert_eq!(
    table.hosts[&HostKind::Codex][3].effort.as_deref(),
    Some("xhigh")
  );
  assert_eq!(
    table.hosts[&HostKind::Claude][2].model.as_deref(),
    Some("opus")
  );
  assert_eq!(table.prefer["critical"], [HostKind::Claude]);
  let home = Env::from_pairs([(
    "EPIC_STATE_HOME",
    tmp.path().join("none").display().to_string(),
  )]);
  assert_eq!(load_table(&home), default_table());
}

/// The table's hosts as the shared fixture spells them.
fn hosts_json(table: &RoutingTable) -> Value {
  let mut out = serde_json::Map::new();
  for kind in HOST_KINDS {
    let rows = table.hosts.get(&kind).cloned().unwrap_or_default();
    out.insert(
      kind.name().to_owned(),
      serde_json::to_value(rows).unwrap_or(Value::Null),
    );
  }
  Value::Object(out)
}
