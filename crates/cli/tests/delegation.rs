//! Machine setup and relay stdin through the real native CLI.

use serde_json::{Value, json};
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

fn command(root: &Path) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_toolu"));
  command.env_clear();
  command.env("PATH", std::env::var_os("PATH").unwrap_or_default());
  command.env("HOME", root);
  if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  command.arg("--json").arg("--config-dir").arg(root);
  command
}

fn register(root: &Path) -> std::io::Result<std::process::Output> {
  command(root)
    .args([
      "setup",
      "machines",
      "register",
      "worker",
      "--root",
      "/srv/tasks",
      "--agent",
      "codex",
    ])
    .output()
}

#[test]
fn worker_setup_survives_processes_and_reuses_configuration() {
  let dir = tempfile::tempdir().unwrap();
  let registered = register(dir.path()).unwrap();
  assert!(
    registered.status.success(),
    "{}",
    String::from_utf8_lossy(&registered.stderr)
  );
  let path = dir.path().join("delegation/config.json");
  let bytes = std::fs::read(&path).unwrap();
  let repeated = register(dir.path()).unwrap();
  assert!(repeated.status.success());
  assert_eq!(std::fs::read(&path).unwrap(), bytes);
  let drained = command(dir.path())
    .args(["machines", "drain", "worker"])
    .output()
    .unwrap();
  assert!(drained.status.success());
  let listed = command(dir.path())
    .args(["setup", "machines", "list"])
    .output()
    .unwrap();
  let config: Value = serde_json::from_slice(&listed.stdout).unwrap();
  assert_eq!(config["machines"]["worker"]["capacity"], 1);
  assert_eq!(config["machines"]["worker"]["draining"], true);
  assert_eq!(config["machines"].as_object().unwrap().len(), 1);
  let invalid = command(dir.path())
    .args([
      "setup", "machines", "register", "bad", "--root", "relative", "--agent", "codex",
    ])
    .output()
    .unwrap();
  assert!(!invalid.status.success());
  assert_eq!(
    serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap(),
    config
  );
}

#[test]
fn relay_receiver_reads_exact_stdin_and_preserves_retired_receipts() {
  let dir = tempfile::tempdir().unwrap();
  let mut child = command(dir.path())
    .args(["relay", "receive"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let payload = json!({"op":"enroll", "expected":{"version":1,"relay":null,"machines":{},"recipes":{}}, "config":{"version":1,"relay":"relay","generation":"first","machines":{},"recipes":{}}});
  child
    .stdin
    .take()
    .unwrap()
    .write_all(payload.to_string().as_bytes())
    .unwrap();
  let output = child.wait_with_output().unwrap();
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["relay"],
    "relay"
  );
  assert!(dir.path().join("delegation/authority.json").is_file());
  let invalid = command(dir.path())
    .args(["relay", "receive"])
    .stdin(Stdio::null())
    .output()
    .unwrap();
  assert!(!invalid.status.success());
  assert!(
    serde_json::from_slice::<Value>(&invalid.stdout)
      .unwrap()
      .get("error")
      .is_some()
  );
}

#[test]
fn relay_service_recovers_a_dead_owner_lock() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("delegation");
  std::fs::create_dir(&root).unwrap();
  std::fs::write(
    root.join("config.json"),
    r#"{"version":1,"relay":"relay","machines":{},"recipes":{}}"#,
  )
  .unwrap();
  std::fs::write(root.join("authority.json"), r#"{"machine":"relay"}"#).unwrap();
  let mut finished = Command::new("true").spawn().unwrap();
  let dead_pid = finished.id();
  assert!(finished.wait().unwrap().success());
  std::fs::write(root.join("service.lock"), format!("{dead_pid} previous\n")).unwrap();
  let mut service = command(dir.path())
    .args(["relay", "serve"])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
  while !root.join("health.json").exists() && std::time::Instant::now() < deadline {
    assert!(
      service.try_wait().unwrap().is_none(),
      "relay stopped before recovering its lock"
    );
    std::thread::yield_now();
  }
  let healthy = root.join("health.json").exists();
  std::fs::remove_file(root.join("authority.json")).unwrap();
  let output = service.wait_with_output().unwrap();
  assert!(healthy, "relay did not recover its dead service owner");
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(!root.join("service.lock").exists());
}
