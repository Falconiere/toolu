use toolu_runtime::env::Env;

use super::summarize;

#[test]
fn captured_rust_panic_keeps_app_frames_and_collapses_runtime() {
  let path =
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/debug/rust-panic.txt");
  let contents = std::fs::read_to_string(path).unwrap();
  let lines: Vec<&str> = contents.lines().collect();
  let doc: serde_json::Value =
    serde_json::from_str(&summarize(&lines, true, &Env::process())).unwrap();
  assert_eq!(doc["recognized"], true);
  assert!(doc["noise_frames"].as_u64().unwrap() > 0);
  assert!(doc["app_frames"].to_string().contains("level_three"));
}

#[test]
fn unrelated_text_is_bounded_raw_input() {
  let env = Env::process().with("DEBUG_MAX_LINES", "1");
  let text = summarize(&["one", "two"], false, &env);
  assert!(text.contains("one"));
  assert!(text.contains("(+1 more lines)"));
}
