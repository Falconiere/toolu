use serde_json::{Value, json};

use crate::server::{Engine, Fault};

#[test]
fn local_worker_identity_matches_name_worktree_or_pane_and_excludes_unrelated_rows() {
  let fixture = crate::source_fix::fixture(true).expect("fixture");
  let mut engine = Engine::open(fixture.engine_paths, None, Fault::None).expect("engine");
  let worktree = fixture.tmp.path().join("worker");
  std::fs::create_dir(&worktree).expect("worktree");
  let issue = engine.world.issues.get_mut("a").expect("issue");
  issue.agent = "local-worker".into();
  issue.worktree = Some(worktree.display().to_string());
  issue.pane = Some("%7".into());
  let rows = [
    json!({"name": "local-worker", "pane_id": "w1:p1"}),
    json!({"name": "renamed", "cwd": worktree, "pane_id": "w2:p1"}),
    json!({"foreground_cwd": worktree, "pane_id": "w3:p1"}),
    json!({"name": "renamed", "cwd": "/elsewhere", "pane_id": "%7"}),
    json!({"name": "unrelated", "cwd": "/elsewhere", "pane_id": "%8"}),
    json!({}),
  ];
  for (row, expected) in rows.iter().zip([true, true, true, true, false, false]) {
    assert_eq!(super::same_agent(issue, row), expected, "{row}");
  }
  let subscriptions = super::filters(&engine.world, &json!({"agents": rows}));
  let panes: Vec<_> = subscriptions
    .iter()
    .filter_map(|item| item.get("pane_id").and_then(Value::as_str))
    .collect();
  assert_eq!(panes, ["w1:p1", "w2:p1", "w3:p1", "%7"]);
}
