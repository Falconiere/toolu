use serde_json::json;

use super::*;
use crate::dispatch::names::{gh_config, native_worktree};
use crate::dispatch::procs::group_alive;
use crate::dispatch::sandbox::{FIX, Sandbox, sandbox};

fn cleaned(sb: &Sandbox) -> Result<Value> {
  Dispatcher::new(&sb.options(false))?.cleanup()
}

fn start(sb: &Sandbox) -> Value {
  let (plan, items) = sb.plan("opencode", Some("probe/m"));
  Dispatcher::new(&sb.options(false))
    .unwrap()
    .start(&sb.start(&plan, &items))
    .unwrap()
}

#[test]
fn cleanup_refuses_fixer_work_then_removes_the_worktree_branch_and_round_files() {
  let sb = sandbox();
  let ledger = sb.state()["actions"].clone();
  assert_eq!(start(&sb)["reason"], "agent_start_failed");
  let waited = Dispatcher::new(&sb.options(false))
    .unwrap()
    .wait(1)
    .unwrap();
  assert_eq!(
    (waited["status"].as_str(), waited["reason"].as_str()),
    (Some("failed"), Some("agent_start_failed"))
  );
  let worktree = native_worktree(&sb.state);
  std::fs::create_dir(worktree.join(".opencode")).unwrap();
  std::fs::write(worktree.join(".opencode/package.json"), "{}").unwrap();
  std::fs::write(worktree.join("extra.ts"), "export {};\n").unwrap();
  assert_eq!(cleaned(&sb).unwrap_err().code, "worktree_dirty");
  assert!(worktree.exists());
  std::fs::remove_file(worktree.join("extra.ts")).unwrap();
  std::fs::create_dir_all(gh_config(&sb.state).join("state")).unwrap();
  let done = cleaned(&sb).unwrap();
  let expected = json!({"version": 1, "status": "cleaned", "worktreeRemoved": true, "branchDeleted": true, "note": null});
  assert_eq!(done, expected);
  assert!(!worktree.exists());
  assert!(!gh_config(&sb.state).exists());
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
  sb.opencode(FIX, "done");
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
fn a_detached_fixer_survives_start_and_cleanup_reaps_its_group() {
  let sb = sandbox();
  sb.opencode("sleep 60 &\nsleep 60", "");
  assert_eq!(start(&sb)["status"], "running");
  let group = sb.state()["fixer"]["groups"][0].clone();
  let pid = u32::try_from(group["pid"].as_u64().unwrap()).unwrap();
  let since = group["pidStart"].as_str().unwrap().to_owned();
  std::thread::sleep(std::time::Duration::from_millis(300));
  assert!(group_alive(pid, &since));
  assert_eq!(cleaned(&sb).unwrap()["status"], "cleaned");
  assert!(!group_alive(pid, &since));
}

#[test]
fn a_dry_cleanup_records_its_commands_and_without_a_worktree_says_so() {
  let sb = sandbox();
  let none = cleaned(&sb).unwrap();
  assert_eq!(none["note"], "no herdr worktree recorded");
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
      "worktree remove",
      "worktree prune",
      "fetch --quiet",
      "branch --quiet"
    ]
  );
  assert_eq!(std::fs::read_to_string(&sb.state).unwrap(), before);
}
