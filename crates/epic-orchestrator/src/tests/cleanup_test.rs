use super::*;
use crate::disk::read_value;
use crate::model::{Action, Issue, fresh};
use crate::server::Fault;

fn add(paths: &Paths, key: &str, complete: bool, closed: bool) -> std::path::PathBuf {
  let dir = paths.root.join(key);
  let mut graph = crate::support::graph_248();
  graph.issues.clear();
  graph.complete = complete;
  graph.epic.state = if closed { "closed" } else { "open" }.to_owned();
  crate::graph::write_graph(&dir.join("graph.json"), &graph).unwrap();
  crate::control::register(paths, dir.to_str().unwrap()).unwrap();
  dir
}

fn removed(doc: &Value) -> usize {
  doc["removed"].as_array().unwrap().len()
}

#[test]
fn completed_closed_missing_and_unfinished_entries_are_selected_conservatively() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  let complete = add(&paths, "complete", true, false);
  add(&paths, "closed", false, true);
  add(&paths, "unfinished", false, false);
  crate::control::register(&paths, paths.root.join("missing").to_str().unwrap()).unwrap();
  let before = std::fs::read(paths.registry()).unwrap();
  let preview = apply(&paths, None, true).unwrap();
  assert_eq!(removed(&preview), 3);
  assert_eq!(std::fs::read(paths.registry()).unwrap(), before);
  assert_eq!(removed(&apply(&paths, None, false).unwrap()), 3);
  assert!(complete.join("graph.json").is_file());
  assert_eq!(removed(&apply(&paths, None, false).unwrap()), 0);
  assert_eq!(
    read_value(&paths.registry()).unwrap()["epics"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
}

#[test]
fn corrupt_unreadable_and_incomplete_worker_state_is_retained() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  for (key, body) in [
    (
      "active",
      r#"{"key":"a","repo":"o/r","branch":"a","stage":"running","checkout":"/c","worktree":"/w"}"#,
    ),
    (
      "incomplete",
      r#"{"key":"a","repo":"o/r","branch":"a","stage":"cleanup-incomplete","checkout":"/c","worktree":"/w"}"#,
    ),
    ("corrupt", "{"),
  ] {
    let dir = add(&paths, key, true, true);
    std::fs::create_dir_all(dir.join("issues")).unwrap();
    std::fs::write(dir.join("issues/a.json"), body).unwrap();
  }
  let unreadable = add(&paths, "unreadable", true, true).join("status/a.json");
  // A directory at a JSON path fails on both root and unprivileged hosts.
  std::fs::create_dir_all(unreadable).unwrap();
  let bad = add(&paths, "bad-graph", true, true);
  std::fs::write(bad.join("graph.json"), "null").unwrap();
  let active = add(&paths, "active-status", true, true);
  crate::disk::write_value(
    &active.join("status/a.json"),
    &json!({"phase": "execution"}),
  )
  .unwrap();
  let doc = apply(&paths, None, false).unwrap();
  assert_eq!(removed(&doc), 0);
  assert_eq!(doc["retained"].as_array().unwrap().len(), 6);
}

#[test]
fn pending_and_failed_closure_survive_even_when_the_directory_is_missing() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  for (key, name, note) in [
    ("pending", "close-intent", ""),
    ("failed", "close-done", "failed"),
    ("uncertain", "close-done", "uncertain"),
  ] {
    crate::control::register(&paths, paths.root.join(key).to_str().unwrap()).unwrap();
    crate::journal::append(
      &paths.journal_dir(),
      &paths.journal_lock(),
      &crate::journal::Record::new("action", name, key, "t", note),
      std::time::SystemTime::now(),
    )
    .unwrap();
  }
  assert_eq!(removed(&apply(&paths, None, false).unwrap()), 0);
}

#[test]
fn dry_run_keeps_live_state_and_commit_prunes_it_before_restart() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  let dir = add(&paths, "done", true, true);
  let mut engine = Engine::open(paths.clone(), None, Fault::None).unwrap();
  engine.pause(Some("done")).unwrap();
  let issue = Issue::blank("a", "done", dir.to_str().unwrap(), 0);
  engine.world.issues.insert("a".into(), issue);
  crate::attention::raise(&mut engine.world, "epic-complete", "done", "done");
  engine.flush().unwrap();
  let before = engine.world.clone();
  assert_eq!(removed(&apply(&paths, Some(&mut engine), true).unwrap()), 1);
  assert_eq!(engine.world, before);
  assert_eq!(
    removed(&apply(&paths, Some(&mut engine), false).unwrap()),
    1
  );
  assert!(engine.world.epics.is_empty());
  assert!(engine.world.issues.is_empty());
  assert_eq!(engine.world.attention.len(), 0);
  let reopened = Engine::open(paths, None, Fault::None).unwrap();
  assert_eq!(reopened.world.attention.len(), 0);
  assert!(reopened.world.epics.is_empty());
  assert!(reopened.world.paused.is_empty());
}

#[test]
fn live_activity_overrides_a_completed_graph_and_a_missing_directory() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  add(&paths, "done", true, true);
  let mut engine = Engine::open(paths.clone(), None, Fault::None).unwrap();
  std::fs::remove_dir_all(paths.root.join("done")).unwrap();
  engine.world.epics.get_mut("done").unwrap().pending = Some(fresh(Action::Close, "t".into()));
  assert_eq!(
    removed(&apply(&paths, Some(&mut engine), false).unwrap()),
    0
  );
  engine.world.epics.get_mut("done").unwrap().pending = None;
  engine.world.epics.get_mut("done").unwrap().close_blocked = true;
  assert_eq!(
    removed(&apply(&paths, Some(&mut engine), false).unwrap()),
    0
  );
}

#[test]
fn a_missing_directory_with_a_resource_owner_or_spooled_request_is_retained() {
  use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  let dir = paths.root.join("missing").display().to_string();
  crate::control::register(&paths, &dir).unwrap();
  let request = LeaseRequest {
    kind: LeaseKind::Agent,
    key: "a".into(),
    state_dir: dir.clone(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  acquire_lease(&paths.root, &request, std::time::SystemTime::now()).unwrap();
  assert_eq!(removed(&apply(&paths, None, false).unwrap()), 0);
  std::fs::remove_file(paths.root.join("state.json")).unwrap();
  crate::disk::write_value(
    &paths.spool().join("t.json"),
    &json!({"op": "report", "epic": "missing", "state_dir": dir}),
  )
  .unwrap();
  assert_eq!(removed(&apply(&paths, None, false).unwrap()), 0);
}

#[test]
fn settled_records_override_an_old_graph_but_require_teardown_evidence() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  let dir = add(&paths, "done", true, true);
  let mut graph = crate::graph::read_graph(&dir.join("graph.json")).unwrap();
  let mut child = crate::support::graph_248().issues.remove(0);
  child.key = "a".into();
  child.stage = Some("running".into());
  graph.issues.push(child);
  crate::graph::write_graph(&dir.join("graph.json"), &graph).unwrap();
  let mut record = json!({"key": "a", "repo": "o/r", "branch": "a", "checkout": "/c", "worktree": dir.join("gone"), "stage": "merged"});
  crate::disk::write_value(&dir.join("issues/a.json"), &record).unwrap();
  assert_eq!(removed(&apply(&paths, None, false).unwrap()), 0);
  record["worktree_removed"] = json!(true);
  record["branch_deleted"] = json!(true);
  crate::disk::write_value(&dir.join("issues/a.json"), &record).unwrap();
  assert_eq!(removed(&apply(&paths, None, false).unwrap()), 1);
}

#[test]
fn registry_write_failure_keeps_live_state_and_the_original_registry() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  add(&paths, "done", true, true);
  let mut engine = Engine::open(paths.clone(), None, Fault::None).unwrap();
  let before = engine.world.clone();
  let body = std::fs::read(paths.registry()).unwrap();
  std::fs::rename(paths.registry(), paths.root.join("saved.json")).unwrap();
  std::fs::create_dir(paths.registry()).unwrap();
  let result = commit(
    &paths,
    &json!({"epics": []}),
    Some(&mut engine),
    &BTreeSet::from(["done".into()]),
  );
  assert!(result.is_err());
  assert_eq!(engine.world, before);
  std::fs::remove_dir(paths.registry()).unwrap();
  std::fs::rename(paths.root.join("saved.json"), paths.registry()).unwrap();
  assert_eq!(std::fs::read(paths.registry()).unwrap(), body);
  assert_eq!(
    removed(&apply(&paths, Some(&mut engine), false).unwrap()),
    1
  );
}

#[test]
fn settled_abandonment_is_not_rewound_by_a_completed_launch() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  let dir = add(&paths, "done", true, true);
  let record = json!({"key": "a", "repo": "o/r", "branch": "a",
    "checkout": dir.join("checkout"), "worktree": dir.join("gone"),
    "stage": "abandoned", "worktree_removed": true});
  crate::disk::write_value(&dir.join("issues/a.json"), &record).unwrap();
  for name in ["launch-intent", "launch-done"] {
    let mut line = crate::journal::Record::new("action", name, "a", "t1", "");
    line.epic = "done".into();
    crate::journal::append(
      &paths.journal_dir(),
      &paths.journal_lock(),
      &line,
      std::time::SystemTime::now(),
    )
    .unwrap();
    if name == "launch-intent" {
      assert_eq!(removed(&apply(&paths, None, true).unwrap()), 0);
    }
  }
  let doc = apply(&paths, None, false).unwrap();
  assert_eq!(removed(&doc), 1, "{doc}");
}

#[test]
fn an_unknown_graph_worker_stage_is_retained() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  let dir = add(&paths, "done", true, true);
  let mut graph = crate::graph::read_graph(&dir.join("graph.json")).unwrap();
  let mut child = crate::support::graph_248().issues.remove(0);
  child.key = "a".into();
  child.state = "closed".into();
  child.stage = Some("unrecognized-stage".into());
  graph.issues.push(child);
  crate::graph::write_graph(&dir.join("graph.json"), &graph).unwrap();
  assert_eq!(
    apply(&paths, None, false).unwrap(),
    json!({"dry_run": false, "removed": [],
    "retained": [{"entry": {"key": "done", "state_dir": dir}, "reason": "unrecognized graph stage for a"}]})
  );
}
