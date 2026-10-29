use serde_json::json;

use crate::finish::finish_issue;
use crate::finish_rig::{agent_lease, layout, leases, patch, read};
use crate::launch_rig::Rig;

#[test]
fn missing_terminal_identity_keeps_the_worktree_and_lease() {
  let l = layout();
  let token = agent_lease(&l, "running");
  patch(
    &l,
    &json!({"backend": "herdr-app", "resource_root": l.resources, "lease_token": token}),
  );
  let rig = Rig::new();
  let err =
    finish_issue(&l.state, "k-1", (true, false), &mut rig.ctx()).expect_err("identity required");
  assert!(err.contains("missing hc terminal address"));
  assert!(l.wt.exists());
  assert_eq!(leases(&l).len(), 1);
  assert_eq!(read(&l.record)["stage"], "cleanup-incomplete");
}

#[test]
fn remote_cleanup_retains_the_worktree_and_lease() {
  let l = layout();
  let token = agent_lease(&l, "running");
  patch(
    &l,
    &json!({"backend": "herdr-app", "machine": "peer", "remote_repo": "/repo", "resource_root": l.resources, "lease_token": token}),
  );
  let rig = Rig::new();
  let err =
    finish_issue(&l.state, "k-1", (true, false), &mut rig.ctx()).expect_err("remote refused");
  assert!(err.contains("missing remote worktree"));
  assert!(l.wt.exists());
  assert_eq!(leases(&l).len(), 1);
  assert_eq!(read(&l.record)["stage"], "cleanup-incomplete");
}

#[test]
fn remote_result_must_be_imported_before_cleanup() {
  let l = layout();
  patch(
    &l,
    &json!({"backend": "herdr-app", "machine": "peer", "remote_repo": "/repo", "remote_worktree": "/wt"}),
  );
  let rig = Rig::new();
  let fixture = crate::finish_rig::live(&l, &rig, true);
  let github = crate::support::gh(&fixture);
  let mut ctx = rig.ctx();
  ctx.gh = Ok(&github);
  let err = finish_issue(&l.state, "k-1", (false, false), &mut ctx).expect_err("result required");
  assert!(err.contains("not been verified and imported"));
  assert!(l.wt.exists());
  assert_eq!(leases(&l).len(), 1);
}
