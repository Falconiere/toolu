//! `toolu epic metrics` and `toolu epic baseline` on committed fixtures (#451):
//! the documents validate against their `$defs` in `toolu commands --schema`
//! and equal the hand-computed expectations.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn fixtures() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic")
}

fn run(resources: &Path, args: &[&str]) -> Res<Output> {
  Ok(
    Command::new(TOOLU)
      .args(args)
      .env("TOOLU_RESOURCE_HOME", resources)
      .output()?,
  )
}

fn document(output: &Output) -> Res<Value> {
  if output.status.code() != Some(0) {
    return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
  }
  Ok(serde_json::from_slice(&output.stdout)?)
}

fn read(path: &Path) -> Res<Value> {
  Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

/// The errors of `doc` against `$defs/<def>` of the binary's schema.
fn violations(doc: &Value, def: &str) -> Res<Vec<String>> {
  let schema = document(
    &Command::new(TOOLU)
      .args(["commands", "--schema"])
      .output()?,
  )?;
  let schema = json!({
    "$schema": schema.get("$schema"),
    "$defs": schema.get("$defs"),
    "$ref": format!("#/$defs/{def}"),
  });
  let validator = jsonschema::validator_for(&schema).map_err(|err| err.to_string())?;
  Ok(
    validator
      .iter_errors(doc)
      .map(|err| err.to_string())
      .collect(),
  )
}

/// A resource root holding the fixture journal.
fn journal_root() -> Res<tempfile::TempDir> {
  let root = tempfile::tempdir()?;
  let journal = root.path().join("journal");
  std::fs::create_dir_all(&journal)?;
  for entry in std::fs::read_dir(fixtures().join("metrics/journal"))? {
    let path = entry?.path();
    if let Some(name) = path.file_name() {
      std::fs::copy(&path, journal.join(name))?;
    }
  }
  Ok(root)
}

fn without_clock(mut doc: Value) -> Value {
  if let Some(map) = doc.as_object_mut() {
    map.remove("generated_at");
  }
  doc
}

#[test]
fn metrics_of_the_fixture_journal_validate_and_match_the_hand_computed_values() {
  let root = journal_root().unwrap();
  let doc = document(&run(root.path(), &["--json", "epic", "metrics"]).unwrap()).unwrap();
  assert_eq!(
    violations(&doc, "epicMetrics").unwrap(),
    Vec::<String>::new()
  );
  let expected = read(&fixtures().join("metrics/expected.json")).unwrap();
  assert_eq!(without_clock(doc), expected);
}

#[test]
fn epic_and_since_narrow_the_document() {
  let root = journal_root().unwrap();
  let one =
    document(&run(root.path(), &["--json", "epic", "metrics", "--epic", "e1"]).unwrap()).unwrap();
  assert_eq!(one["records"], 37);
  assert_eq!(one["issues"], 2);
  assert_eq!(one["lead_time_ms"]["count"], 2);
  assert_eq!(one["launches"]["host_switches"], 0);
  assert_eq!(
    one["jobs"]["pressure_holds"], 1,
    "b's unstamped job records are e1's"
  );
  let args = [
    "--json",
    "epic",
    "metrics",
    "--since",
    "2026-10-10T00:50:00Z",
  ];
  let late = document(&run(root.path(), &args).unwrap()).unwrap();
  assert_eq!(late["since"], "2026-10-10T00:50:00.000Z");
  assert_eq!(
    late["lead_time_ms"]["count"], 1,
    "only c launched after the cutoff"
  );
  assert_eq!(late["ready_to_merged_ms"]["total"]["count"], 2);
}

#[test]
fn an_empty_root_is_a_zero_document_and_a_bad_date_is_a_usage_error() {
  let root = tempfile::tempdir().unwrap();
  let doc = document(&run(root.path(), &["--json", "epic", "metrics"]).unwrap()).unwrap();
  assert_eq!(doc["records"], 0);
  assert_eq!(
    violations(&doc, "epicMetrics").unwrap(),
    Vec::<String>::new()
  );
  let bad = run(root.path(), &["epic", "metrics", "--since", "yesterday"]).unwrap();
  assert_eq!(bad.status.code(), Some(64));
  assert!(String::from_utf8_lossy(&bad.stderr).contains("YYYY-MM-DD"));
}

#[test]
fn the_text_form_sets_each_measure_beside_the_baseline() {
  let root = journal_root().unwrap();
  let output = run(root.path(), &["epic", "metrics", "--epic", "e1"]).unwrap();
  assert_eq!(output.status.code(), Some(0));
  let text = String::from_utf8_lossy(&output.stdout);
  assert!(text.contains("Epic metrics for e1"), "{text}");
  assert!(text.contains("Lead time p50 / p90"), "{text}");
  assert!(text.contains("3 h 39 min / 15 h 24 min"), "{text}");
  assert!(text.contains("Rebase rounds"), "{text}");
  assert!(text.contains("prompts-while-waiting"), "{text}");
}

#[test]
fn baseline_of_the_fixture_state_matches_and_a_malformed_file_fails() {
  let root = tempfile::tempdir().unwrap();
  let state = fixtures().join("baseline/state");
  let state = state.to_string_lossy();
  let args = [
    "epic",
    "baseline",
    "--state",
    &state,
    "--until",
    "2026-10-02T00:00:00Z",
  ];
  let doc =
    document(&run(root.path(), &[&args[..], &["--outlier", "out"]].concat()).unwrap()).unwrap();
  assert_eq!(
    violations(&doc, "epicBaseline").unwrap(),
    Vec::<String>::new()
  );
  assert_eq!(
    doc,
    read(&fixtures().join("baseline/expected.json")).unwrap()
  );
  let broken = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(broken.path().join("e/issues")).unwrap();
  std::fs::write(broken.path().join("e/issues/x.json"), "{").unwrap();
  let path = broken.path().to_string_lossy();
  let failed = run(
    root.path(),
    &[
      "epic",
      "baseline",
      "--state",
      &path,
      "--until",
      "2026-10-02T00:00:00Z",
    ],
  )
  .unwrap();
  assert_eq!(failed.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&failed.stderr).contains("x.json"));
}

#[test]
fn summary_reports_an_epic_from_the_journal_and_refuses_an_unknown_one() {
  let root = journal_root().unwrap();
  let doc = document(&run(root.path(), &["--json", "epic", "summary", "e1"]).unwrap()).unwrap();
  assert_eq!(
    violations(&doc, "epicSummary").unwrap(),
    Vec::<String>::new()
  );
  assert_eq!(
    doc["rows"][0],
    json!({"measure": "Merged issues", "epic": "2", "baseline": "83"})
  );
  let text = run(root.path(), &["epic", "summary", "e1"]).unwrap();
  assert!(String::from_utf8_lossy(&text.stdout).starts_with("## Epic metrics"));
  let unknown = run(root.path(), &["epic", "summary", "nope"]).unwrap();
  assert_eq!(unknown.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown epic nope"));
}
