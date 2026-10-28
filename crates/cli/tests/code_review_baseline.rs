//! The recorded code-review eval baseline, black-box: replaying the committed
//! store answers every request byte for byte, so a prompt change that has not
//! been re-recorded fails here instead of silently going unmeasured. The pinned
//! totals are the baseline later changes are compared against.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// The replayed eval report at medium and high, both arms, after checking it
/// exited 2 (medium sends no callers, so it still misses `caller-break`) and
/// left no case unverified.
fn replay() -> Result<Value, String> {
  let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-review-eval");
  let project = tempfile::tempdir().map_err(|err| err.to_string())?;
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
    .map_err(|err| err.to_string())?;
  if output.status.code() != Some(2) {
    return Err(format!("{output:?}"));
  }
  let doc: Value = serde_json::from_slice(&output.stdout).map_err(|err| err.to_string())?;
  let cases = doc
    .get("cases")
    .and_then(Value::as_array)
    .ok_or("no cases")?;
  if let Some(case) = cases
    .iter()
    .find(|case| case.get("unverified") != Some(&serde_json::json!([])))
  {
    return Err(format!("unverified: {case}"));
  }
  Ok(doc)
}

#[test]
fn the_committed_replay_store_answers_every_case_and_pins_the_baseline() {
  let doc = replay().unwrap();
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

#[test]
fn reported_findings_carry_jev_assessments_only_where_jev_ran() {
  let doc = replay().unwrap();
  let assessed: Vec<(String, String)> = doc["cases"]
    .as_array()
    .unwrap()
    .iter()
    .flat_map(|case| case["arms"].as_array().unwrap())
    .flat_map(|arm| {
      let findings = arm["unclassified"].as_array().unwrap();
      findings.iter().map(|finding| {
        let assessment = finding["assessment"].as_str().unwrap();
        (
          arm["arm"].as_str().unwrap().to_owned(),
          assessment.to_owned(),
        )
      })
    })
    .collect();
  let has = |arm: &str, assessment: &str| {
    assessed
      .iter()
      .any(|item| item.0 == arm && item.1 == assessment)
  };
  assert!(has("enhanced", "supported"), "{assessed:?}");
  // The baseline arm never runs Jev.
  assert!(
    assessed
      .iter()
      .all(|(arm, assessment)| arm != "baseline" || assessment == "not-judged"),
    "{assessed:?}"
  );
}
