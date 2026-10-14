use std::path::{Path, PathBuf};

use serde_json::json;
use toolu_runtime::env::Env;

use super::{closing, markdown, report};
use crate::paths::Paths;

fn tokens_state() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens/state")
}

/// A resource root whose registry names epic `e` at the token fixture's state.
fn root() -> tempfile::TempDir {
  let root = tempfile::tempdir().unwrap();
  let paths = Paths::at(root.path());
  let registry = json!({"epics": [{"key": "e", "state_dir": tokens_state()}]});
  std::fs::write(paths.registry(), registry.to_string()).unwrap();
  std::fs::create_dir_all(paths.journal_dir()).unwrap();
  let line = |seq: u32, name: &str, key: &str, second: u32| {
    json!({"seq": seq, "at": format!("2026-10-10T00:00:{second:02}.000Z"), "kind": "action",
      "name": name, "epic": "e", "key": key, "token": "", "note": ""})
    .to_string()
  };
  let text = [
    line(1, "launch-intent", "a", 0),
    line(2, "launch-done", "a", 1),
    line(3, "cleanup-done", "a", 30),
  ]
  .join("\n");
  std::fs::write(paths.journal_dir().join("2026-10-10.jsonl"), text + "\n").unwrap();
  root
}

#[test]
fn the_report_sets_the_epic_beside_the_baseline_with_budgets_and_tokens() {
  let root = root();
  let claude = tokens_state().join("../claude");
  let env = Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("CLAUDE_CONFIG_DIR", claude.to_str().unwrap()),
  ]);
  let report = report(&Paths::at(root.path()), "e", 0, Some(&env)).unwrap();
  assert_eq!(
    report["rows"][0],
    json!({"measure": "Merged issues", "epic": "1", "baseline": "83"})
  );
  assert_eq!(report["tokens"].as_array().unwrap().len(), 4);
  assert_eq!(report["tokens"][0]["tokens"]["output_tokens"], 23);
  let text = markdown(&report);
  assert!(text.starts_with("## Epic metrics\n\n| Measure | This epic | TypeScript baseline |"));
  assert!(
    text.contains("| Lead time p50 / p90 | 30 s / 30 s | 3 h 39 min / 15 h 24 min |"),
    "{text}"
  );
  assert!(text.contains("| check-gap |"), "{text}");
  assert!(text.contains("| c | opencode |"), "{text}");
}

#[test]
fn the_closing_section_has_no_tokens_and_an_unknown_epic_fails() {
  let root = root();
  let paths = Paths::at(root.path());
  let section = closing(&paths, &tokens_state(), 0).unwrap();
  assert!(section.contains("| Merged issues | 1 | 83 |"), "{section}");
  assert!(!section.contains("| Issue | Host |"), "{section}");
  assert_eq!(closing(&paths, Path::new("/elsewhere"), 0), None);
  let err = report(&paths, "nope", 0, None).unwrap_err();
  assert!(err.contains("unknown epic nope"), "{err}");
}
