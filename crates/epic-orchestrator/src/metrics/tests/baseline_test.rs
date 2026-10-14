use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Input, derive};

fn fixtures() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/baseline")
}

fn input(state: PathBuf, outliers: &[&str]) -> Input {
  Input {
    state,
    until: "2026-10-02T00:00:00Z".to_owned(),
    outliers: outliers.iter().map(|epic| (*epic).to_owned()).collect(),
  }
}

#[test]
fn the_fixture_state_gives_the_hand_computed_baseline() {
  let expected: Value =
    serde_json::from_str(&std::fs::read_to_string(fixtures().join("expected.json")).unwrap())
      .unwrap();
  let derived = derive(&input(fixtures().join("state"), &["out"])).unwrap();
  assert_eq!(derived, expected);
}

#[test]
fn without_outliers_every_epic_is_in_the_main_figures() {
  let derived = derive(&input(fixtures().join("state"), &[])).unwrap();
  assert_eq!(derived["main"]["merged_issues"], 3);
  assert_eq!(derived["main"]["epics"], 2);
  assert_eq!(derived["outliers"], serde_json::json!({}));
}

#[test]
fn a_malformed_issue_or_a_bad_cutoff_fails_naming_it() {
  let dir = tempfile::tempdir().unwrap();
  let issues = dir.path().join("e/issues");
  std::fs::create_dir_all(&issues).unwrap();
  std::fs::write(issues.join("bad.json"), "{not json").unwrap();
  let err = derive(&input(dir.path().to_owned(), &[])).unwrap_err();
  assert!(err.contains("bad.json"), "{err}");
  let mut bad = input(fixtures().join("state"), &[]);
  bad.until = "yesterday".to_owned();
  assert!(derive(&bad).unwrap_err().contains("--until"));
  let missing = derive(&input(dir.path().join("absent"), &[])).unwrap_err();
  assert!(missing.contains("absent"), "{missing}");
}

#[test]
fn the_committed_baseline_holds_the_issue_figures() {
  let committed: Value = serde_json::from_str(crate::metrics::text::BASELINE).unwrap();
  assert_eq!(committed["schema"], 1);
  assert_eq!(committed["source"]["records"], 115);
  assert_eq!(committed["source"]["merged"], 98);
  assert_eq!(committed["main"]["merged_issues"], 83);
  assert_eq!(committed["main"]["agent_hours"], 542.2);
  assert_eq!(committed["main"]["rebase_rounds"], 119);
  assert_eq!(committed["main"]["relaunched_issues"], 11);
  assert_eq!(committed["main"]["lead_time_ms"]["p50"], 13_195_000);
  assert_eq!(
    committed["outliers"]["falconiere-toolu-orm-145"]["merged_issues"],
    15
  );
  assert_eq!(committed["reported"]["supervisor_rss_mb_per_epic"], 31);
}
