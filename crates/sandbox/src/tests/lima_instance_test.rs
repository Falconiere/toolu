use std::path::PathBuf;

use super::{Status, find, major_version};

/// The shape of `limactl list --format json` on Lima 2.2.1, trimmed to what is read.
const LIST: &str = r#"{"name":"other","status":"Running","cpus":2,"config":{"mounts":[]}}
{"name":"toolu-sandbox","status":"Stopped","cpus":4,"memory":6442450944,"disk":107374182400,"arch":"aarch64","vmType":"vz","config":{"mounts":[{"location":"/w/repo","mountPoint":"/w/repo","writable":true},{"location":"/w/main/.git","writable":true}]}}
"#;

#[test]
fn the_version_is_read_from_limactl_s_line() {
  assert_eq!(major_version("limactl version 2.2.1\n"), Some(2));
  assert_eq!(major_version("limactl version v1.0.7"), Some(1));
  assert_eq!(major_version("garbage"), None);
}

#[test]
fn the_named_instance_is_found_with_its_status_and_mounts() {
  let found = find(LIST, "toolu-sandbox").unwrap().unwrap();
  assert_eq!(found.status, Status::Stopped);
  assert_eq!(
    found.mounts,
    [PathBuf::from("/w/repo"), PathBuf::from("/w/main/.git")]
  );
  assert_eq!(
    find(LIST, "other").unwrap().unwrap().status,
    Status::Running
  );
  assert_eq!(find(LIST, "absent").unwrap(), None);
  assert_eq!(find("", "x").unwrap(), None);
}

#[test]
fn an_unknown_status_is_broken_and_a_bad_line_is_an_error() {
  let broken = find(r#"{"name":"vm","status":"Broken"}"#, "vm")
    .unwrap()
    .unwrap();
  assert_eq!(broken.status, Status::Broken);
  assert_eq!(broken.mounts, Vec::<PathBuf>::new());
  assert_eq!(
    [Status::Running, Status::Stopped, Status::Broken].map(Status::name),
    ["running", "stopped", "broken"]
  );
  let err = find("not json", "vm").unwrap_err();
  assert!(
    err.starts_with("limactl list printed a line that is not JSON"),
    "{err}"
  );
}
