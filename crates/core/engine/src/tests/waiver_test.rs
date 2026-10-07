//! Push waivers on real files (`push-waiver.ts`): one diff, 0600, `jq -c` bytes.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::Waivers;

fn roots(state_dir: Option<&Path>) -> Roots {
  let env = Env::from_pairs([("HOME", "/nonexistent")]);
  let env = match state_dir {
    Some(dir) => env.with("STATE_DIR", &dir.display().to_string()),
    None => env,
  };
  Roots::new(env, Some(Host::Claude))
}

#[test]
fn a_promoted_waiver_covers_exactly_its_sha() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(Some(dir.path()));
  let waivers = Waivers {
    roots: &roots,
    root: None,
    slug: "feat_x",
  };
  let asked = UNIX_EPOCH + Duration::from_secs(1_927_857_906);
  assert!(waivers.pend("A", "main", "findings", asked));
  let pending = std::fs::read_to_string(waivers.pending_path()).unwrap();
  assert_eq!(
    pending,
    "{\"version\":1,\"branch\":\"feat_x\",\"diff_sha\":\"A\",\"base_branch\":\"main\",\"reason_code\":\"findings\",\"asked_at\":\"2031-02-03T04:05:06Z\"}\n"
  );
  assert!(!waivers.matches("A"), "a pending marker is not a waiver");
  assert!(
    !waivers.promote("B", asked),
    "a marker for another sha waives nothing"
  );
  assert!(waivers.promote("A", asked + Duration::from_secs(60)));
  let waiver = std::fs::read_to_string(waivers.path()).unwrap();
  assert_eq!(
    waiver,
    "{\"version\":1,\"branch\":\"feat_x\",\"diff_sha\":\"A\",\"base_branch\":\"main\",\"reason_code\":\"findings\",\"waived_at\":\"2031-02-03T04:06:06Z\"}\n"
  );
  let mode = std::fs::metadata(waivers.path())
    .unwrap()
    .permissions()
    .mode()
    & 0o777;
  assert_eq!(mode, 0o600);
  assert!(!Path::new(&waivers.pending_path()).exists());
  assert!(waivers.matches("A"));
  assert!(!waivers.matches("B"));
  assert!(!waivers.matches(""));
}

#[test]
fn malformed_or_foreign_files_never_match() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(Some(dir.path()));
  let waivers = Waivers {
    roots: &roots,
    root: None,
    slug: "s",
  };
  for body in [
    "{\"version\":2,\"diff_sha\":\"A\"}",
    "{\"version\":1,\"diff_sha\":\"\"}",
    "{\"version\":1,\"diff_sha\":false}",
    "[\"A\"]",
    "{not json",
  ] {
    std::fs::write(waivers.path(), body).unwrap();
    assert!(!waivers.matches("A"), "{body}");
  }
  std::fs::write(waivers.path(), "{\"version\":\"1\",\"diff_sha\":\"A\"}").unwrap();
  assert!(waivers.matches("A"), "jq -r reads the string 1 as 1");
  assert!(!waivers.pend("", "main", "x", UNIX_EPOCH));
  assert!(!waivers.promote("", UNIX_EPOCH));
  assert!(!waivers.promote("A", UNIX_EPOCH), "no pending marker");
}

#[test]
fn the_directory_is_the_override_or_the_projects_push_review_state() {
  let dir = tempfile::tempdir().unwrap();
  let plain = roots(None);
  let project = Waivers {
    roots: &plain,
    root: Some(dir.path()),
    slug: "s",
  };
  let expected = dir.path().join(".claude/tmp/push-review");
  assert_eq!(project.dir(), expected.display().to_string());
  assert!(
    project.pend("A", "main", "x", UNIX_EPOCH),
    "the directory is created"
  );
  let blocked = dir.path().join("file");
  std::fs::write(&blocked, "").unwrap();
  let roots = roots(Some(&blocked.join("sub")));
  let unwritable = Waivers {
    roots: &roots,
    root: None,
    slug: "s",
  };
  assert!(!unwritable.pend("A", "main", "x", UNIX_EPOCH));
}
