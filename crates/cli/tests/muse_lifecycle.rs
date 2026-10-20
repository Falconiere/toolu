//! Muse lifecycle output through the real `toolu` binary.

use std::io::Write as _;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn hook(root: &std::path::Path, name: &str, path: &str, payload: &str) -> Output {
  let mut child = Command::new(TOOLU)
    .args(["hook", name, "--event", "SessionStart"])
    .current_dir(root)
    .env("HOME", root)
    .env("TOOLU_HOST_OVERRIDE", "muse")
    .env("MUSE_CONFIG_DIR", root.join("config"))
    .env("TOOLU_PROJECT_DIR", root)
    .env("PATH", path)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all(payload.as_bytes())
    .unwrap();
  child.wait_with_output().unwrap()
}

#[test]
fn muse_startup_and_resume_use_system_message_with_runtime_line() {
  let root = tempfile::tempdir().unwrap();
  let path = std::path::Path::new(TOOLU)
    .parent()
    .unwrap()
    .display()
    .to_string();
  for (source, title) in [("startup", "Toolu is on!"), ("resume", "Session resumed")] {
    let output = hook(
      root.path(),
      "session-start",
      &path,
      &format!(r#"{{"source":"{source}"}}"#),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("hookSpecificOutput").is_none(), "{value}");
    let message = value["systemMessage"].as_str().unwrap();
    assert!(
      message.starts_with(&format!("{title}\ntoolu runtime: native ")),
      "{message}"
    );
  }
}

#[test]
fn muse_check_binary_reports_missing_native_once_per_session() {
  let root = tempfile::tempdir().unwrap();
  let payload = r#"{"session_id":"muse-missing"}"#;
  let first = hook(root.path(), "check-binary", "/nonexistent", payload);
  assert_eq!(first.status.code(), Some(0), "{first:?}");
  let value: Value = serde_json::from_slice(&first.stdout).unwrap();
  assert!(value.get("hookSpecificOutput").is_none());
  assert!(value["systemMessage"].as_str().unwrap().contains("Muse"));
  let second = hook(root.path(), "check-binary", "/nonexistent", payload);
  assert_eq!(second.status.code(), Some(0), "{second:?}");
  assert!(second.stdout.is_empty(), "{second:?}");
}

#[cfg(unix)]
#[test]
fn muse_check_binary_advises_install_when_a_shim_shadows_it() {
  use std::os::unix::fs::PermissionsExt as _;

  let root = tempfile::tempdir().unwrap();
  let shadow = root.path().join("shadow");
  std::fs::create_dir_all(&shadow).unwrap();
  let shim = shadow.join("toolu");
  std::fs::write(&shim, "#!/bin/sh\nexit 1\n").unwrap();
  std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
  let output = hook(
    root.path(),
    "check-binary",
    &shadow.display().to_string(),
    r#"{"session_id":"muse-shadow"}"#,
  );
  assert_eq!(output.status.code(), Some(0), "{output:?}");
  let value: Value = serde_json::from_slice(&output.stdout).unwrap();
  assert!(
    value["systemMessage"]
      .as_str()
      .unwrap()
      .contains("Install it with:")
  );
}
