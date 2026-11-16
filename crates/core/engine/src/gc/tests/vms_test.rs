use std::os::unix::fs::PermissionsExt as _;

use super::{colima, docker, idle};
use crate::gc::support::Repo;

/// A `colima` that reports the gate profile Running and accepts `stop`, and a
/// `docker` that reports no container and no dangling image, on the test PATH.
fn gate_programs(repo: &Repo) {
  let bin = repo.tmp.join("bin");
  for (name, body) in [
    (
      "colima",
      "#!/bin/sh\n[ \"$1\" = list ] && printf '{\"name\":\"toolu-gate\",\"status\":\"Running\"}\\n'\nexit 0\n",
    ),
    ("docker", "#!/bin/sh\nexit 0\n"),
  ] {
    let path = bin.join(name);
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
  }
}

#[test]
fn vms_are_skipped_without_their_programs_and_kept_while_a_lease_is_held() {
  let repo = Repo::new().unwrap();
  let mut request = repo.request(false);
  assert_eq!(
    idle(&request.resource_root),
    Ok(true),
    "an absent state holds no lease"
  );
  assert_eq!(colima(&request, &Ok(true))["toolu-gate"], "no-colima");
  assert_eq!(docker(&request)["skipped"], "no docker");

  gate_programs(&repo);
  assert_eq!(colima(&request, &Ok(true))["toolu-gate"], "would-stop");
  assert_eq!(docker(&request)["dangling"], 0);
  request.apply = true;
  assert_eq!(colima(&request, &Ok(true))["toolu-gate"], "stopped");

  std::fs::create_dir_all(&request.resource_root).unwrap();
  std::fs::write(
    request.resource_root.join("state.json"),
    r#"{"version":1,"leases":[{"token":"t","type":"agent","key":"k","stateDir":"/s","ownerPid":1,"stage":"running","createdAt":"2026-01-01T00:00:00Z","heartbeatAt":"2026-01-01T00:00:00Z"}],"cooldowns":{}}"#,
  )
  .unwrap();
  assert_eq!(idle(&request.resource_root), Ok(false));
  let held = colima(&request, &Ok(false));
  assert_eq!(held["toolu-gate"], "kept");
  assert_eq!(held["reason"], "a worker lease is held");
}
