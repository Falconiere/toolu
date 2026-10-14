use serde_json::json;

use super::*;
use crate::dispatch::sandbox::{FIX, Sandbox, sandbox};

fn cleaned(sb: &Sandbox) -> Result<Value> {
  Dispatcher::new(&sb.options(false))?.cleanup()
}

fn start(sb: &Sandbox) -> Value {
  let (plan, items) = sb.plan("claude", Some("opus"));
  Dispatcher::new(&sb.options(false))
    .unwrap()
    .start(&sb.start(&plan, &items))
    .unwrap()
}

#[test]
fn cleanup_refuses_fixer_work_then_removes_the_worktree_branch_and_round_files() {
  let sb = sandbox();
  sb.herdr(r#"{"result":{"agent":{"name":"pb"}}}"#, "true");
  let ledger = sb.state()["actions"].clone();
  assert_eq!(start(&sb)["status"], "running");
  let worktree = sb.dir.path().join("herdr-wt");
  std::fs::create_dir(worktree.join(".cursor")).unwrap();
  std::fs::write(worktree.join(".cursor/package.json"), "{}").unwrap();
  std::fs::write(worktree.join("extra.ts"), "export {};\n").unwrap();
  assert_eq!(cleaned(&sb).unwrap_err().code, "worktree_dirty");
  assert!(worktree.exists());
  std::fs::remove_file(worktree.join("extra.ts")).unwrap();
  let done = cleaned(&sb).unwrap();
  let expected = json!({"version": 1, "status": "cleaned", "worktreeRemoved": true, "branchDeleted": true, "note": null});
  assert_eq!(done, expected);
  assert!(!worktree.exists());
  let state = sb.state();
  assert_eq!(
    (&state["fixer"], &state["herdrWorktree"]),
    (&Value::Null, &Value::Null)
  );
  assert_eq!(state["actions"], ledger);
}

#[test]
fn cleanup_keeps_a_slot_branch_holding_unpushed_fixer_commits() {
  let sb = sandbox();
  sb.herdr(r#"{"result":{"agent":{"name":"pb"}}}"#, FIX);
  start(&sb);
  assert_eq!(
    Dispatcher::new(&sb.options(false))
      .unwrap()
      .wait(30)
      .unwrap()["status"],
    "done"
  );
  let kept = cleaned(&sb).unwrap();
  assert_eq!(kept["worktreeRemoved"], true);
  assert_eq!(kept["branchDeleted"], false);
  let note = "kept pr-babysit/falconiere-toolu-165: it holds commits origin/feat/fix does not";
  assert_eq!(kept["note"], note);
}

#[test]
fn a_dry_cleanup_records_its_commands_and_without_a_worktree_says_so() {
  let sb = sandbox();
  let none = cleaned(&sb).unwrap();
  assert_eq!(none["note"], "no herdr worktree recorded");
  sb.herdr(r#"{"result":{"agent":{"name":"pb"}}}"#, FIX);
  start(&sb);
  let before = std::fs::read_to_string(&sb.state).unwrap();
  let dry = Dispatcher::new(&sb.options(true))
    .unwrap()
    .cleanup()
    .unwrap();
  let commands: Vec<Vec<String>> = serde_json::from_value(dry["commands"].clone()).unwrap();
  let verbs: Vec<String> = commands
    .iter()
    .map(|command| command[3..5].join(" "))
    .collect();
  assert_eq!(
    verbs,
    [
      "--workspace w1",
      "worktree prune",
      "fetch --quiet",
      "branch --quiet"
    ]
  );
  assert_eq!(std::fs::read_to_string(&sb.state).unwrap(), before);
}
