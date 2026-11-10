use serde_json::json;

use crate::finish::finish_issue;
use crate::finish_rig::{agent_lease, layout, patch};
use crate::launch_rig::{Rig, snapshot_tree};
use crate::support::git;

#[test]
fn a_local_dry_run_preserves_dirty_worktree_records_leases_and_git_refs() {
  let (layout, rig) = (layout(), Rig::new());
  let token = agent_lease(&layout, "running");
  patch(
    &layout,
    &json!({"resource_root": layout.resources, "lease_token": token, "workspace_id": "w-1", "agent": "k-1"}),
  );
  std::fs::write(layout.wt.join("a.txt"), b"uncommitted edit\n").expect("tracked edit");
  std::fs::write(layout.wt.join("new.txt"), b"untracked work\n").expect("untracked edit");
  let files = snapshot_tree(&layout.wt);
  let records = snapshot_tree(&layout.state);
  let resources = snapshot_tree(&layout.resources);
  let refs = git(
    &layout.main,
    &["for-each-ref", "--format=%(refname):%(objectname)"],
  );
  let worktrees = git(&layout.main, &["worktree", "list", "--porcelain"]);

  let report = finish_issue(&layout.state, "k-1", (true, true), &mut rig.ctx()).expect("plan");

  assert_eq!(report.finished["dry_run"], true);
  assert_eq!(snapshot_tree(&layout.wt), files);
  assert_eq!(snapshot_tree(&layout.state), records);
  assert_eq!(snapshot_tree(&layout.resources), resources);
  assert_eq!(
    git(
      &layout.main,
      &["for-each-ref", "--format=%(refname):%(objectname)"]
    ),
    refs
  );
  assert_eq!(
    git(&layout.main, &["worktree", "list", "--porcelain"]),
    worktrees
  );
  assert!(
    rig.herdr.calls().is_empty(),
    "planning must not stop or remove a worker"
  );
}
