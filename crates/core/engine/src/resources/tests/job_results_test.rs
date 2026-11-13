//! Job result records in a temp resource home: lifecycle, claim-once, order.

use std::path::{Path, PathBuf};

use super::{announce, claim_finished, claim_one, create, get, line, pending, update};

fn finish(root: &Path, id: &str, code: i64) -> super::Record {
  let mut record = get(root, id).unwrap().unwrap();
  record.state = "done".to_owned();
  record.exit_code = Some(code);
  record.finished_at = Some(record.started_at.clone());
  record.output = "tail".to_owned();
  update(root, &record).unwrap();
  record
}

#[test]
fn a_record_runs_waiting_running_done_and_reads_back() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("resources");
  let worktree = dir.path().join("tree");
  let mut record = create(&root, "issue-1", &worktree, "cargo test").unwrap();
  assert_eq!(
    (record.state.as_str(), record.exit_code, record.stale),
    ("waiting", None, false)
  );
  let path = root
    .join("jobs/results")
    .join(format!("{}.json", record.id));
  let text = std::fs::read_to_string(&path).unwrap();
  assert!(text.contains("\"schema\":1"), "{text}");
  record.state = "running".to_owned();
  update(&root, &record).unwrap();
  let done = finish(&root, &record.id, 3);
  assert_eq!(
    line(&done),
    format!("job {} finished: exit 3: cargo test", done.id)
  );
  assert_eq!(get(&root, &done.id).unwrap(), Some(done));
  assert_eq!(get(&root, "missing").unwrap(), None);
  let mut invalid: serde_json::Value = serde_json::from_str(&text).unwrap();
  invalid["finished_at"] = serde_json::json!(3);
  std::fs::write(path, invalid.to_string()).unwrap();
  assert!(get(&root, &record.id).is_err());
}

#[test]
fn pending_is_oldest_first_per_worktree_and_claim_takes_a_done_record_once() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("resources");
  let tree: PathBuf = dir.path().join("tree");
  let other: PathBuf = dir.path().join("other");
  let mut first = create(&root, "k", &tree, "first").unwrap();
  let mut second = create(&root, "k", &tree, "second").unwrap();
  first.started_at = "2020-01-01T00:00:00.000Z".to_owned();
  second.started_at = "2021-01-01T00:00:00.000Z".to_owned();
  update(&root, &first).unwrap();
  update(&root, &second).unwrap();
  create(&root, "k", &other, "elsewhere").unwrap();
  let running = pending(&root, &tree).unwrap();
  assert_eq!(
    running.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
    ["first", "second"]
  );
  assert_eq!(claim_finished(&root, &tree).unwrap().len(), 0);
  finish(&root, &first.id, 0);
  finish(&root, &second.id, 1);
  let claimed = claim_finished(&root, &tree).unwrap();
  assert_eq!(claimed.len(), 2);
  assert!(claimed.iter().all(|r| r.delivered_at.is_some()));
  assert_eq!(claim_finished(&root, &tree).unwrap().len(), 0);
  assert_eq!(pending(&root, &tree).unwrap().len(), 0);
  assert_eq!(pending(&root, &other).unwrap().len(), 1);
}

#[test]
fn a_stale_done_record_lines_and_announce_without_an_engine_is_ignored() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("resources");
  let record = create(&root, "k", dir.path(), "gate").unwrap();
  let mut done = finish(&root, &record.id, 2);
  done.stale = true;
  assert_eq!(
    line(&done),
    format!("job {} finished: exit 2, stale: gate", done.id)
  );
  announce(&root, "k");
  assert_eq!(get(&root, &record.id).unwrap().unwrap().state, "done");
}

#[test]
fn claim_one_preserves_other_results_and_rejects_a_foreign_worktree() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("resources");
  let tree = dir.path().join("tree");
  let other = dir.path().join("other");
  let first = create(&root, "k", &tree, "first").unwrap();
  let second = create(&root, "k", &tree, "second").unwrap();
  assert!(claim_one(&root, &tree, &first.id).unwrap().is_none());
  finish(&root, &first.id, 0);
  finish(&root, &second.id, 1);
  assert!(claim_one(&root, &other, &first.id).is_err());
  assert!(claim_one(&root, &tree, &first.id).unwrap().is_some());
  assert!(claim_one(&root, &tree, &first.id).unwrap().is_none());
  assert_eq!(pending(&root, &tree).unwrap()[0].id, second.id);
}
