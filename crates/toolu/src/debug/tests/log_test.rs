use toolu_runtime::env::Env;

use super::summarize;

#[test]
fn repeated_timestamped_errors_collapse_and_tail_stays() {
  let env = Env::process().with("DEBUG_MAX_LINES", "10");
  let lines = [
    "2026-06-18T10:00:00Z error: x",
    "2026-06-18T10:00:01Z error: x",
    "last line",
  ];
  let doc: serde_json::Value = serde_json::from_str(&summarize(&lines, true, &env)).unwrap();
  assert_eq!(doc["errors"].as_array().unwrap().len(), 1);
  assert_eq!(doc["tail"].as_array().unwrap().len(), 3);
  assert_eq!(doc["total_lines"], 3);
}

#[test]
fn real_big_log_is_bounded() {
  let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/debug/big.log");
  let contents = std::fs::read_to_string(path).unwrap();
  let lines: Vec<&str> = contents.lines().collect();
  let env = Env::process().with("DEBUG_MAX_LINES", "10");
  let text = summarize(&lines, false, &env);
  assert!(text.lines().count() <= 10);
  assert!(text.contains("TOTAL lines:"));
}
