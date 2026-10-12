use serde_json::json;
use toolu_engine::resources::binding::bind_worktree;

use super::{fence_cleanup_ownership, release_recorded_ownership, verify_no_jobs};
use crate::finish_rig::{agent_lease, layout, leases, read};

#[test]
fn a_binding_or_lease_of_another_issue_is_refused() {
  let l = layout();
  let state = l.state.display().to_string();
  bind_worktree(&l.resources, &l.wt, "k-2", &state).expect("bind");
  let mut record = read(&l.record).as_object().cloned().expect("object");
  let other = fence_cleanup_ownership(&l.record, &state, "k-1", &mut record);
  assert_eq!(
    other,
    Err("worktree resource binding does not match issue ownership".to_owned())
  );
  bind_worktree(&l.resources, &l.wt, "k-1", &state).expect("bind");
  record.insert("resource_root".into(), json!("/elsewhere"));
  record.insert("lease_token".into(), json!("t"));
  let root = fence_cleanup_ownership(&l.record, &state, "k-1", &mut record);
  assert_eq!(
    root,
    Err("recorded resource root differs from worktree binding".to_owned())
  );
  record.insert("resource_root".into(), json!(l.resources));
  let missing = fence_cleanup_ownership(&l.record, &state, "k-1", &mut record);
  assert_eq!(
    missing,
    Err("recorded agent resource ownership does not match issue".to_owned())
  );
}

#[test]
fn a_legacy_binding_adopts_one_lease_and_release_drops_it() {
  let l = layout();
  let state = l.state.display().to_string();
  bind_worktree(&l.resources, &l.wt, "k-1", &state).expect("bind");
  let mut record = read(&l.record).as_object().cloned().expect("object");
  let owner = fence_cleanup_ownership(&l.record, &state, "k-1", &mut record)
    .expect("adopt")
    .expect("owner");
  assert_eq!(read(&l.record)["lease_token"], json!(owner.token));
  assert_eq!(
    verify_no_jobs(Some(&owner), &l.wt.display().to_string()),
    Ok(())
  );
  assert_eq!(verify_no_jobs(None, "/anywhere"), Ok(()));
  release_recorded_ownership(&record, &state).expect("release");
  assert_eq!(leases(&l), Vec::new());
  release_recorded_ownership(&record, &state).expect("released twice");
  agent_lease(&l, "running");
  let file = l.resources.join("state.json");
  let mut resources = read(&file);
  let mut twin = resources["leases"][0].clone();
  twin["token"] = json!("twin-token");
  resources["leases"]
    .as_array_mut()
    .expect("leases")
    .push(twin);
  crate::finish_rig::write(&file, &resources);
  let mut fresh = read(&l.record).as_object().cloned().expect("object");
  fresh.remove("resource_root");
  fresh.remove("lease_token");
  let ambiguous = fence_cleanup_ownership(&l.record, &state, "k-1", &mut fresh);
  assert_eq!(
    ambiguous,
    Err("ambiguous legacy agent ownership".to_owned())
  );
}
