//! The plan projection over real worktrees and ledgers written by `ledger run`.

use std::fs::File;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_protocol::host::Host;

use super::{head_branch, locate, plan, plan_summary};
use crate::model::Issue;
use crate::plan_rig::{STEPS, Tree, sh};

const SENTINEL: &str = "PLAN-SECRET-CANARY";

#[test]
fn plan_matches_the_status_ledger_for_every_host() {
  for (host, dir) in [
    (Host::Claude, ".claude/tmp/plan-ledger"),
    (Host::Codex, ".codex/tmp/plan-ledger"),
    (Host::Cursor, ".cursor/tmp/plan-ledger"),
    (Host::Muse, ".toolu/tmp/muse/plan-ledger"),
  ] {
    let tree = Tree::new();
    let ledger = tree.ledger(host, STEPS);
    assert!(
      tree.worktree.join(dir).join("feat_x.json").is_file(),
      "{dir}"
    );
    let shown = plan(Some(tree.path()), Some(host.name()));
    for key in ["plan_doc", "updated_at", "next"] {
      assert_eq!(shown[key], ledger[key], "{key}");
    }
    assert_eq!(shown["next"], "s2");
    for key in ["total", "green", "red", "pending", "running", "stale"] {
      assert_eq!(shown["summary"][key], ledger["summary"][key], "{key}");
    }
    assert_eq!(shown["summary"]["red"], 1);
    let steps = shown["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), 2);
    for (index, step) in steps.iter().enumerate() {
      for key in ["id", "title", "status", "activity", "last_run"] {
        assert_eq!(step[key], ledger["steps"][index][key], "{key}");
      }
    }
  }
}

#[test]
fn no_worktree_host_or_ledger_is_null() {
  let tree = Tree::new();
  assert_eq!(plan(Some(tree.path()), Some("claude")), Value::Null);
  assert_eq!(plan_summary(Some(tree.path()), Some("claude")), Value::Null);
  tree.ledger(Host::Claude, STEPS);
  assert_eq!(plan(None, Some("claude")), Value::Null);
  assert_eq!(plan(Some(""), Some("claude")), Value::Null);
  assert_eq!(plan(Some(tree.path()), None), Value::Null);
  assert_eq!(plan(Some(tree.path()), Some("emacs")), Value::Null);
  assert_eq!(plan(Some(tree.path()), Some("muse")), Value::Null);
  assert!(plan(Some(tree.path()), Some("claude")).is_object());
}

#[test]
fn a_corrupt_or_other_version_ledger_is_unreadable() {
  let tree = Tree::new();
  tree.ledger(Host::Claude, STEPS);
  let file = tree.file(Host::Claude, "feat/x");
  let unreadable = json!({"error": "unreadable"});
  tree.edit(Host::Claude, |ledger| ledger["version"] = json!(2));
  assert_eq!(plan(Some(tree.path()), Some("claude")), unreadable);
  for bytes in ["{\"version\":1,", "", "null", "[1]"] {
    std::fs::write(&file, bytes).expect("corrupt ledger");
    assert_eq!(
      plan(Some(tree.path()), Some("claude")),
      unreadable,
      "{bytes:?}"
    );
  }
}

#[test]
fn private_step_fields_never_show_and_text_is_cut() {
  let tree = Tree::new();
  tree.ledger(Host::Claude, STEPS);
  tree.edit(Host::Claude, |ledger| {
    let step = &mut ledger["steps"][0];
    for key in ["check", "evidence_tail", "input", "diff_sha", "scope_sha"] {
      step[key] = json!(SENTINEL);
    }
    step["retries"] = json!([{"evidence_tail": SENTINEL}]);
    step["title"] = json!("t".repeat(400));
    step["activity"] = json!("a".repeat(400));
  });
  let shown = plan(Some(tree.path()), Some("claude"));
  assert!(!shown.to_string().contains(SENTINEL), "{shown}");
  let step = shown["steps"][0].as_object().expect("step");
  let keys: Vec<&str> = step.keys().map(String::as_str).collect();
  assert_eq!(keys, ["activity", "id", "last_run", "status", "title"]);
  assert_eq!(step["title"].as_str().expect("title").chars().count(), 160);
  assert_eq!(
    step["activity"].as_str().expect("activity").chars().count(),
    160
  );
}

#[test]
fn odd_shapes_fall_back_to_null_and_zero() {
  let tree = Tree::new();
  tree.ledger(Host::Claude, STEPS);
  tree.edit(Host::Claude, |ledger| {
    ledger["summary"] = json!({"green": -1, "red": "2"});
    ledger["next"] = json!(7);
    ledger["steps"] = json!([1, {"id": 3}]);
  });
  let shown = plan(Some(tree.path()), Some("claude"));
  assert_eq!(shown["summary"]["green"], 0);
  assert_eq!(shown["summary"]["red"], 0);
  assert_eq!(shown["next"], Value::Null);
  assert_eq!(
    shown["steps"],
    json!([{"id": null, "title": null, "status": null, "activity": null, "last_run": null}])
  );
}

#[test]
fn plan_summary_rereads_only_when_the_mtime_moves() {
  let tree = Tree::new();
  tree.ledger(Host::Claude, STEPS);
  let file = tree.file(Host::Claude, "feat/x");
  let first = plan_summary(Some(tree.path()), Some("claude"));
  assert_eq!(
    (first["green"].clone(), first["next"].clone()),
    (json!(1), json!("s2"))
  );
  let before = std::fs::metadata(&file)
    .expect("meta")
    .modified()
    .expect("mtime");
  tree.edit(Host::Claude, |ledger| ledger["summary"]["green"] = json!(9));
  File::options()
    .write(true)
    .open(&file)
    .expect("open")
    .set_modified(before)
    .expect("old mtime");
  assert_eq!(plan_summary(Some(tree.path()), Some("claude")), first);
  let later = before + Duration::from_secs(5);
  File::options()
    .write(true)
    .open(&file)
    .expect("open")
    .set_modified(later)
    .expect("new mtime");
  assert_eq!(plan_summary(Some(tree.path()), Some("claude"))["green"], 9);
}

#[test]
fn plan_follows_the_worktrees_branch() {
  let tree = Tree::new();
  tree.ledger(Host::Claude, STEPS);
  tree.sh("git checkout -q -b other");
  assert_eq!(plan(Some(tree.path()), Some("claude")), Value::Null);
  tree.ledger(
    Host::Claude,
    r#"[{"id":"o1","title":"other","check":"true"}]"#,
  );
  assert_eq!(
    plan(Some(tree.path()), Some("claude"))["steps"][0]["id"],
    "o1"
  );
  tree.sh("git checkout -q feat/x");
  assert_eq!(
    plan(Some(tree.path()), Some("claude"))["steps"][0]["id"],
    "s1"
  );
}

#[test]
fn head_branch_agrees_with_git_rev_parse() {
  let tree = Tree::new();
  let git = |dir: &std::path::Path| sh(dir, "git rev-parse --abbrev-ref HEAD").trim().to_owned();
  assert_eq!(
    head_branch(&tree.main).as_deref(),
    Some(git(&tree.main).as_str())
  );
  assert_eq!(head_branch(&tree.worktree).as_deref(), Some("feat/x"));
  assert_eq!(git(&tree.worktree), "feat/x");
  tree.sh("git checkout -q --detach");
  assert_eq!(head_branch(&tree.worktree).as_deref(), Some("HEAD"));
  assert_eq!(git(&tree.worktree), "HEAD");
  assert_eq!(head_branch(&tree.worktree.join("a.txt")), None);
}

#[test]
fn locate_prefers_the_launch_record_and_the_current_route() {
  let mut issue = Issue::blank("r-1", "r-0", "/state", 0);
  issue.worktree = Some("/pane".to_owned());
  issue.kind = "codex".to_owned();
  let none = Value::Null;
  assert_eq!(locate(&issue, &none, &none), (Some("/pane"), Some("codex")));
  let meta = json!({"worktree": "/launch", "kind": "cursor"});
  assert_eq!(
    locate(&issue, &meta, &none),
    (Some("/launch"), Some("cursor"))
  );
  let route = json!({"host": "muse"});
  assert_eq!(
    locate(&issue, &meta, &route),
    (Some("/launch"), Some("muse"))
  );
  issue.kind.clear();
  issue.worktree = None;
  assert_eq!(locate(&issue, &none, &none), (None, None));
}
