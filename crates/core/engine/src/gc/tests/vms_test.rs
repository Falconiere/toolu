use super::super::support::Repo;
use super::{colima, docker, idle, sandbox};

#[test]
fn vms_are_skipped_without_their_programs_and_kept_while_a_lease_is_held() {
  let repo = Repo::new().unwrap();
  let mut request = repo.request(true);
  assert_eq!(
    idle(&request.resource_root),
    Ok(true),
    "an absent state holds no lease"
  );
  assert_eq!(sandbox(&request, &Ok(true))["action"], "skipped");
  assert_eq!(colima(&request, &Ok(true))["toolu-gate"], "no-colima");
  assert_eq!(docker(&request)["skipped"], "no docker");

  std::fs::create_dir_all(&request.resource_root).unwrap();
  std::fs::write(
    request.resource_root.join("state.json"),
    r#"{"version":1,"leases":[{"token":"t","type":"agent","key":"k","stateDir":"/s","ownerPid":1,"stage":"running","createdAt":"2026-01-01T00:00:00Z","heartbeatAt":"2026-01-01T00:00:00Z"}],"cooldowns":{}}"#,
  )
  .unwrap();
  assert_eq!(idle(&request.resource_root), Ok(false));
  request.toolu = Some(repo.tmp.join("no-such-toolu"));
  let held = sandbox(&request, &Ok(false));
  assert_eq!(held["action"], "kept");
  assert_eq!(held["reason"], "a worker lease is held");
  assert!(held["clones"]["error"].as_str().is_some(), "{held}");
}
