use std::path::Path;

use serde_json::Value;
use toolu_engine::status::StatusSnapshot;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{Snapshot, command};

#[test]
fn status_is_a_command_without_a_planned_verb() {
  let matches = command().try_get_matches_from(["status"]).unwrap();
  assert!(matches.subcommand().is_none());
}

#[test]
fn snapshot_outside_a_repo_has_empty_repo_fields() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().to_str().unwrap();
  let roots = Roots::new(
    Env::from_pairs([("HOME", home), ("PATH", "/usr/bin:/bin")]),
    Some(Host::Claude),
  );
  let value = Snapshot
    .snapshot(&roots, Path::new(dir.path()))
    .expect("snapshot");
  assert_eq!(value["namespace"], "status");
  assert_eq!(value["host"], "claude");
  assert_eq!(value["repo_root"], "");
  assert_eq!(value["branch"], "");
  assert_eq!(value["ahead"], 0);
  assert_eq!(value["working_tree"]["untracked"], 0);
  assert_eq!(value["gate"]["status"], "missing");
  assert_eq!(value["push_review"]["state"], "missing");
  assert_eq!(value["waivers"]["waiver"], Value::Null);
  assert_eq!(value["waivers"]["pending"], Value::Null);
}
