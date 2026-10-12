use super::{Place, ensure_worktree};
use crate::herdr_state::Scripted;
use crate::launch_rig::Rig;

#[test]
fn a_listed_worktree_without_a_path_is_refused() {
  let rig = Rig::new();
  rig.herdr.script(
    "worktree.list",
    Scripted::Raw(
      r#"{"result":{"worktrees":[{"branch":"feat/x","open_workspace_id":"w1"}]}}"#.into(),
    ),
  );
  rig.herdr.script(
    "pane.list",
    Scripted::Raw(r#"{"result":{"panes":[{"pane_id":"p1"}]}}"#.into()),
  );
  let mut issue = rig.graph.find("comemory-255").expect("issue").clone();
  issue.branch = "feat/x".into();
  let place = Place {
    checkout: "/repo",
    base: "main",
    dry: false,
  };
  let mut ctx = rig.ctx();
  let refused =
    ensure_worktree(&mut ctx.herdr, &place, &issue, &mut Vec::new()).expect_err("no path");
  assert_eq!(
    refused.message,
    "herdr listed feat/x without a worktree path"
  );
  rig.herdr.script(
    "worktree.list",
    Scripted::Raw(
      r#"{"result":{"worktrees":[{"branch":"feat/x","open_workspace_id":"w9"}]}}"#.into(),
    ),
  );
  rig.herdr.script(
    "pane.list",
    Scripted::Raw(r#"{"result":{"panes":[]}}"#.into()),
  );
  let empty =
    ensure_worktree(&mut ctx.herdr, &place, &issue, &mut Vec::new()).expect_err("no pane");
  assert_eq!(empty.message, "no panes in workspace w9");
}

#[test]
fn a_worktree_is_created_then_reused_while_open() {
  let mut rig = Rig::new();
  let checkout = rig.checkout("comemory-255");
  let issue = rig.graph.find("comemory-255").expect("issue").clone();
  let checkout = checkout.display().to_string();
  let place = Place {
    checkout: &checkout,
    base: "main",
    dry: false,
  };
  let mut ctx = rig.ctx();
  crate::support::git(
    std::path::Path::new(&checkout),
    &["fetch", "-q", "origin", "main"],
  );
  let mut log = Vec::new();
  let created = ensure_worktree(&mut ctx.herdr, &place, &issue, &mut log).expect("created");
  assert!(log[0].starts_with(&format!("herdr worktree create --cwd {checkout} --branch {} --base origin/main --label comemory-255 --no-focus", issue.branch)));
  let reused = ensure_worktree(&mut ctx.herdr, &place, &issue, &mut log).expect("reused");
  assert_eq!(reused, created);
  assert_eq!(
    log[1],
    format!("reuse open worktree workspace {}", created.workspace_id)
  );
}
