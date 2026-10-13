use toolu_runtime::env::Env;

use super::summarize;

#[test]
fn captured_bun_and_cargo_failures_name_tests() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins/toolu/scripts/__tests__/fixtures/debug");
  for (file, name) in [
    ("bun-testfail.txt", "add sums"),
    ("cargo-testfail.txt", "tests::adds"),
  ] {
    let contents = std::fs::read_to_string(root.join(file)).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    let doc: serde_json::Value =
      serde_json::from_str(&summarize(&lines, true, &Env::process())).unwrap();
    assert_eq!(doc["recognized"], true);
    assert!(doc["failures"].to_string().contains(name));
    assert!(doc["locations"][0].is_string());
  }
}

#[test]
fn unknown_output_uses_a_capped_raw_preview() {
  let env = Env::process().with("DEBUG_MAX_LINES", "1");
  let text = summarize(&["hello", "world"], false, &env);
  assert!(text.contains("no recognizable test failures"));
  assert!(text.contains("(+1 more lines)"));
}
