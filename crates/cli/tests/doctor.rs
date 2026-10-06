//! `toolu doctor` checks the same non-login shell that agent command tools use.

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn run(path: &str, args: &[&str]) -> std::io::Result<Output> {
  Command::new(TOOLU).args(args).env("PATH", path).output()
}

fn copy_toolu(dir: &Path) -> std::io::Result<PathBuf> {
  let path = dir.join("toolu");
  std::fs::copy(TOOLU, &path).map(|_| path)
}

#[test]
fn doctor_finds_the_real_binary_that_sh_resolves() {
  let temp = tempfile::tempdir().unwrap();
  let alias = temp.path().join("alias");
  symlink(temp.path(), &alias).unwrap();
  let path = copy_toolu(&alias).unwrap();
  let canonical = path.canonicalize().unwrap();
  let shell_path = format!("{}:/usr/bin:/bin", alias.display());
  let shell = Command::new("sh")
    .args(["-c", "command -v toolu"])
    .env("PATH", &shell_path)
    .output()
    .unwrap();
  assert_eq!(shell.status.code(), Some(0));
  assert_eq!(
    String::from_utf8_lossy(&shell.stdout).trim(),
    path.display().to_string()
  );
  let human = run(&shell_path, &["doctor"]).unwrap();
  assert_eq!(human.status.code(), Some(0), "{human:?}");
  assert!(String::from_utf8_lossy(&human.stdout).contains(&canonical.display().to_string()));
  let json = run(&shell_path, &["doctor", "--json"]).unwrap();
  assert_eq!(json.status.code(), Some(0), "{json:?}");
  let document: Value = serde_json::from_slice(&json.stdout).unwrap();
  assert_eq!(document["namespace"], "doctor");
  assert_eq!(document["reachable"], true);
  assert_eq!(document["path"], canonical.display().to_string());
  let schema_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.schema.json");
  let schema: Value = serde_json::from_str(&std::fs::read_to_string(schema_file).unwrap()).unwrap();
  let doctor_schema = json!({
    "$schema": schema["$schema"],
    "$defs": schema["$defs"],
    "$ref": "#/$defs/doctor",
  });
  assert!(
    jsonschema::validator_for(&doctor_schema)
      .unwrap()
      .is_valid(&document)
  );
  let reduced = run(&alias.display().to_string(), &["doctor"]).unwrap();
  assert_eq!(reduced.status.code(), Some(0), "{reduced:?}");
  assert!(String::from_utf8_lossy(&reduced.stdout).contains(&canonical.display().to_string()));
}

#[test]
fn doctor_fails_when_the_non_login_shell_has_no_toolu() {
  let missing = run("/usr/bin:/bin", &["doctor"]).unwrap();
  assert_eq!(missing.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&missing.stderr).contains("non-login shell"));
  let json = run("/usr/bin:/bin", &["doctor", "--json"]).unwrap();
  assert_eq!(json.status.code(), Some(1));
  let document: Value = serde_json::from_slice(&json.stdout).unwrap();
  assert_eq!(document["error"]["code"], 1);
}

#[test]
fn doctor_bounds_a_hanging_native_probe() {
  let temp = tempfile::tempdir().unwrap();
  let path = temp.path().join("toolu");
  std::fs::write(&path, "#!/bin/sh\nwhile :; do :; done\n").unwrap();
  let mut mode = std::fs::metadata(&path).unwrap().permissions();
  mode.set_mode(0o755);
  std::fs::set_permissions(&path, mode).unwrap();
  let mut doctor = Command::new(TOOLU)
    .arg("doctor")
    .env("PATH", temp.path())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();
  let started = Instant::now();
  loop {
    if let Some(status) = doctor.try_wait().unwrap() {
      assert_eq!(status.code(), Some(1));
      break;
    }
    if started.elapsed() > Duration::from_secs(5) {
      doctor.kill().unwrap();
      doctor.wait().unwrap();
      panic!("doctor did not time out its native probe");
    }
    sleep(Duration::from_millis(20));
  }
}

#[test]
fn doctor_does_not_wait_for_a_descendant_holding_protocol_output_open() {
  let temp = tempfile::tempdir().unwrap();
  let path = temp.path().join("toolu");
  std::fs::write(&path, "#!/bin/sh\n/bin/sleep 3 &\nprintf '1\\n'\n").unwrap();
  let mut mode = std::fs::metadata(&path).unwrap().permissions();
  mode.set_mode(0o755);
  std::fs::set_permissions(&path, mode).unwrap();
  let shell_path = format!("{}:/usr/bin:/bin", temp.path().display());
  let started = Instant::now();
  let output = run(&shell_path, &["doctor"]).unwrap();
  assert_eq!(output.status.code(), Some(0), "{output:?}");
  assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn doctor_rejects_the_real_npm_wrapper_first_on_path() {
  let temp = tempfile::tempdir().unwrap();
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let wrapper = temp.path().join("toolu");
  let built = Command::new("bun")
    .args([
      "build",
      "tools/toolu-cli/src/cli.ts",
      "--target=node",
      "--outfile",
    ])
    .arg(&wrapper)
    .current_dir(repo)
    .output()
    .unwrap();
  assert_eq!(built.status.code(), Some(0), "{built:?}");
  let mut mode = std::fs::metadata(&wrapper).unwrap().permissions();
  mode.set_mode(0o755);
  std::fs::set_permissions(&wrapper, mode).unwrap();
  let node = Command::new("sh")
    .args(["-c", "command -v node"])
    .output()
    .unwrap();
  assert_eq!(node.status.code(), Some(0));
  let node = PathBuf::from(String::from_utf8(node.stdout).unwrap().trim());
  let node_dir = node.parent().unwrap();
  let shell_path = format!(
    "{}:{}:/usr/bin:/bin",
    temp.path().display(),
    node_dir.display()
  );
  let output = run(&shell_path, &["doctor"]).unwrap();
  assert_eq!(output.status.code(), Some(1), "{output:?}");
  assert!(String::from_utf8_lossy(&output.stderr).contains("does not resolve a native toolu"));
}
