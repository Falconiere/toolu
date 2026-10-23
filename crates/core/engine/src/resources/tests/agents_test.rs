//! Agent lease reclamation over a real isolated resource home (#583).

use std::path::Path;
use std::process::Command;

use super::reconcile_agent_leases;
use crate::resources::store::{leases, read_resource_state};

fn dead_pid() -> u32 {
  let mut child = Command::new("true").spawn().unwrap();
  let pid = child.id();
  child.wait().unwrap();
  pid
}

fn seed(root: &Path, items: &[String]) {
  std::fs::create_dir_all(root).unwrap();
  let body = format!(
    r#"{{"version":1,"leases":[{}],"cooldowns":{{}}}}"#,
    items.join(",")
  );
  std::fs::write(root.join("state.json"), body).unwrap();
}

fn lease(kind: &str, key: &str, owner: u32, pane: Option<&str>) -> String {
  let pane = pane.map_or(String::new(), |p| format!(r#","pane":"{p}""#));
  format!(
    r#"{{"token":"t-{key}","type":"{kind}","key":"{key}","stateDir":"s","ownerPid":{owner},"stage":"running","createdAt":"c","heartbeatAt":"h"{pane}}}"#
  )
}

fn keys(root: &Path) -> Vec<String> {
  leases(&read_resource_state(root).unwrap())
    .iter()
    .filter_map(|l| match l.get("key") {
      Some(toolu_runtime::json::ordered::Ordered::String(k)) => Some(k.clone()),
      _ => None,
    })
    .collect()
}

#[test]
fn a_dead_owner_with_no_live_agent_is_reclaimed_and_the_rest_are_kept() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let dead = dead_pid();
  let alive = std::process::id();
  seed(
    root,
    &[
      lease("agent", "gone", dead, Some("p1")),
      lease("agent", "never-started", dead, None),
      lease("agent", "still-listed", dead, Some("p2")),
      lease("agent", "owner-alive", alive, Some("p3")),
      lease("job", "job-alive", alive, None),
    ],
  );
  let live = |key: &str, _pane: Option<&str>| key == "still-listed";
  assert_eq!(reconcile_agent_leases(root, live), Ok(2));
  assert_eq!(keys(root), ["still-listed", "owner-alive", "job-alive"]);
  assert_eq!(reconcile_agent_leases(root, live), Ok(0));
}

#[test]
fn a_missing_state_reclaims_nothing() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(reconcile_agent_leases(dir.path(), |_, _| false), Ok(0));
}
