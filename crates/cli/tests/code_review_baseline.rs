//! The recorded code-review eval baseline, black-box: replaying the committed
//! store answers every request byte for byte, so a prompt change that has not
//! been re-recorded fails here instead of silently going unmeasured. The pinned
//! totals are the baseline later phases are compared against.

use std::path::Path;
use std::process::Command;

#[test]
fn the_committed_replay_store_answers_every_case_and_pins_the_baseline() {
  let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-review-eval");
  let project = tempfile::tempdir().unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_toolu"))
    .env("TOOLU_PROJECT_DIR", project.path())
    .env("TOOLU_CONFIG_DIR", project.path())
    .env_remove("OPENROUTER_API_KEY")
    .env_remove("TOOLU_USER_CONFIG_DIR")
    .args([
      "--json",
      "--host",
      "claude",
      "code-review",
      "eval",
      "--cases",
    ])
    .arg(&cases)
    .args(["--effort", "medium", "--effort", "high"])
    .output()
    .unwrap();
  // Exit 2: medium sends no callers, so it still misses `caller-break`.
  assert_eq!(output.status.code(), Some(2), "{output:?}");
  let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  for case in doc["cases"].as_array().unwrap() {
    assert_eq!(case["unverified"], serde_json::json!([]), "{}", case["id"]);
  }
  let settings = doc["calibration"]["settings"].as_array().unwrap();
  assert_eq!(settings.len(), 4);
  for setting in settings {
    let caught = if setting["setting"] == "high" { 8 } else { 7 };
    assert_eq!(setting["cases"], 10, "{setting}");
    assert_eq!(setting["defects_caught"], caught, "{setting}");
    assert_eq!(setting["defects_missed"], 8 - caught, "{setting}");
    assert_eq!(setting["false_positives"], 0, "{setting}");
  }
}
