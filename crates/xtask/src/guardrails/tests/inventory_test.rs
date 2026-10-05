use super::check;
use crate::data::InventoryEntry;
use crate::guardrails::tests::{context, tree};

const MAIN: &str = "crates/xtask/src/main.rs";
const TESTS: &str = "crates/xtask/src/tests/main_test.rs";
const TABLE: &str = "//! t\nconst TASKS: &[(&str, u8)] = &[(\"one\", 1), (\"two\", 2)];\n";
const TEST_FNS: &str = "#[test]\nfn passes() {}\n#[test]\n#[ignore]\nfn skipped() {}\n";

fn entry(id: &str, pass: &str, fail: &str) -> InventoryEntry {
  InventoryEntry {
    kind: "xtask-task".to_owned(),
    id: id.to_owned(),
    pass: format!("{TESTS}::{pass}"),
    fail: fail.to_owned(),
  }
}

#[test]
fn every_discovered_item_needs_an_entry() {
  let tree = tree(&[(MAIN, TABLE), (TESTS, TEST_FNS)]);
  let mut ctx = context(&tree.workspace);
  ctx.inventory.entries = vec![entry("one", "passes", &format!("{TESTS}::passes"))];
  let found = check(&ctx).unwrap();
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].path, MAIN);
  assert_eq!(found[0].line, 2);
  assert!(
    found[0]
      .message
      .starts_with("xtask-task `two` has no entry")
  );
  assert!(
    found[0]
      .message
      .ends_with("add a passing and a failing scenario")
  );
}

#[test]
fn scenarios_must_resolve_to_runnable_tests() {
  let tree = tree(&[(MAIN, TABLE), (TESTS, TEST_FNS)]);
  let mut ctx = context(&tree.workspace);
  ctx.inventory.entries = vec![
    entry("one", "skipped", "no-separator"),
    entry("two", "missing", "crates/xtask/src/nope.rs::x"),
    entry("three", "passes", &format!("{TESTS}::passes")),
  ];
  let messages: Vec<String> = check(&ctx)
    .unwrap()
    .into_iter()
    .map(|f| f.message)
    .collect();
  assert_eq!(
    messages,
    [
      format!("xtask-task `one`: {TESTS}::skipped is #[ignore]d"),
      "xtask-task `one`: `no-separator` is not <path>::<test fn>".to_owned(),
      format!("xtask-task `two`: {TESTS} has no #[test] fn missing"),
      "xtask-task `two`: crates/xtask/src/nope.rs is not a Rust file of the workspace".to_owned(),
      "xtask-task `three` is not a discovered item".to_owned(),
    ]
  );
}

#[test]
fn an_absent_discovery_file_discovers_nothing_and_a_missing_table_fails_closed() {
  let empty = tree(&[]);
  assert_eq!(check(&context(&empty.workspace)).unwrap(), Vec::new());
  let tableless = tree(&[(MAIN, "//! t\nfn main() {}\n")]);
  let err = check(&context(&tableless.workspace)).unwrap_err();
  assert_eq!(
    err,
    format!("{MAIN}: no const TASKS to discover xtask-task items from")
  );
  let broken = tree(&[(MAIN, "fn (")]);
  assert!(
    check(&context(&broken.workspace))
      .unwrap_err()
      .contains("cannot discover")
  );
}

#[test]
fn a_plain_string_array_is_a_table_too() {
  let solo = tree(&[(MAIN, "//! t\nconst TASKS: [&str; 1] = [\"solo\"];\n")]);
  let found = check(&context(&solo.workspace)).unwrap();
  assert!(found[0].message.starts_with("xtask-task `solo`"));
  let other = tree(&[(MAIN, "//! t\nconst TASKS: u8 = 1;\n")]);
  assert_eq!(check(&context(&other.workspace)).unwrap(), Vec::new());
}
