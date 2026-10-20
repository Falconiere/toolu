//! Muse doctor through the real `toolu` binary with an isolated command path.

use std::process::Command;

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

#[test]
fn muse_doctor_reports_roots_and_unavailable_binary_and_inventory() {
  let home = tempfile::tempdir().unwrap();
  let empty_bin = home.path().join("bin");
  std::fs::create_dir_all(&empty_bin).unwrap();
  let output = Command::new(TOOLU)
    .args(["--host", "muse", "--json", "doctor"])
    .env("HOME", home.path())
    .env("XDG_CONFIG_HOME", home.path().join(".config"))
    .env("PATH", &empty_bin)
    .env("TOOLU_PROJECT_DIR", home.path())
    .env_remove("TOOLU_CONFIG_DIR")
    .env_remove("MUSE_CONFIG_DIR")
    .output()
    .unwrap();
  assert_eq!(output.status.code(), Some(1), "{output:?}");
  let document: Value = serde_json::from_slice(&output.stdout).unwrap();
  let checks = document["checks"].as_array().unwrap();
  let check = |id: &str| checks.iter().find(|item| item["id"] == id).unwrap();
  assert_eq!(check("host")["details"]["host"], "muse");
  let expected_config = home.path().join(".config/muse");
  assert_eq!(
    check("host")["details"]["configRoot"].as_str(),
    expected_config.to_str()
  );
  assert_eq!(check("reachability")["status"], "fail");
  assert_eq!(check("plugins")["status"], "warn");
  assert!(
    check("plugins")["summary"]
      .as_str()
      .unwrap()
      .contains("unavailable")
  );
}
