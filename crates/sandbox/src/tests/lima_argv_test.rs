use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;

use super::{create, delete, edit, find, guest_env, list, shell, start, stop, version};
use crate::provision::VmSpec;

const LIMA: &str = "/opt/lima/limactl";

fn program() -> &'static Path {
  Path::new(LIMA)
}

#[test]
fn every_command_line_is_exact() {
  assert_eq!(version(program()), [LIMA, "--version"]);
  assert_eq!(list(program()), [LIMA, "list", "--format", "json"]);
  assert_eq!(
    create(program(), "vm", Path::new("/s/vm.yaml")),
    [
      LIMA,
      "--tty=false",
      "start",
      "--name",
      "vm",
      "--timeout",
      "30m",
      "/s/vm.yaml"
    ]
  );
  assert_eq!(
    start(program(), "vm"),
    [LIMA, "--tty=false", "start", "--timeout", "30m", "vm"]
  );
  assert_eq!(stop(program(), "vm"), [LIMA, "--tty=false", "stop", "vm"]);
  assert_eq!(
    delete(program(), "vm"),
    [LIMA, "--tty=false", "delete", "--force", "vm"]
  );
}

#[test]
fn edit_sets_cpus_memory_and_exactly_the_planned_mounts() {
  let spec = VmSpec {
    cpus: 6,
    memory_gib: 12,
    disk_gib: 100,
    base: "b".to_owned(),
    mounts: vec![PathBuf::from("/a b"), PathBuf::from("/c")],
  };
  assert_eq!(
    edit(program(), "vm", &spec),
    [
      LIMA,
      "--tty=false",
      "edit",
      "vm",
      "--cpus",
      "6",
      "--memory",
      "12",
      "--mount-only",
      "/a b:w",
      "--mount-only",
      "/c:w"
    ]
  );
}

#[test]
fn shell_runs_the_command_from_the_workdir_with_the_guest_env() {
  let mut command = guest_env("proj-12345678", true);
  command.extend(["cargo".to_owned(), "test".to_owned()]);
  assert_eq!(
    shell(program(), "vm", Path::new("/w/sub"), &command),
    [
      LIMA,
      "shell",
      "--workdir",
      "/w/sub",
      "vm",
      "--",
      "env",
      "TMPDIR=/var/cache/toolu-sandbox/tmp",
      "CARGO_TARGET_DIR=/var/cache/toolu-sandbox/target/proj-12345678",
      "cargo",
      "test"
    ]
  );
  assert_eq!(
    guest_env("x", false),
    ["env", "TMPDIR=/var/cache/toolu-sandbox/tmp"]
  );
}

#[test]
fn find_takes_the_first_executable_limactl_on_path() {
  let dir = tempfile::tempdir().unwrap();
  let (plain, tool) = (dir.path().join("plain"), dir.path().join("tool"));
  std::fs::create_dir_all(&plain).unwrap();
  std::fs::create_dir_all(&tool).unwrap();
  std::fs::write(plain.join("limactl"), "not executable").unwrap();
  let script = tool.join("limactl");
  std::fs::write(&script, "#!/bin/sh\n").unwrap();
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = format!("::{}:{}", plain.display(), tool.display());
  assert_eq!(
    find(&Env::from_pairs([("PATH", path.as_str())])),
    Some(script)
  );
  assert_eq!(find(&Env::from_pairs([("PATH", "/nowhere")])), None);
  assert_eq!(find(&Env::from_pairs([("HOME", "/h")])), None);
}

#[test]
fn a_mount_with_a_comma_or_quote_is_csv_quoted_for_lima() {
  let spec = VmSpec {
    cpus: 2,
    memory_gib: 2,
    disk_gib: 100,
    base: "b".to_owned(),
    mounts: vec![
      PathBuf::from("/a,b"),
      PathBuf::from("/q\"x"),
      PathBuf::from("/a b:c"),
    ],
  };
  let words = edit(program(), "vm", &spec);
  assert_eq!(
    words[8..],
    [
      "--mount-only",
      "\"/a,b:w\"",
      "--mount-only",
      "\"/q\"\"x:w\"",
      "--mount-only",
      "/a b:c:w"
    ]
  );
}
