use super::check;
use crate::guardrails::tests::{context, rules, tree};

#[test]
fn an_inline_test_module_in_src_fails() {
  let text = "//! demo\nfn f() {}\n#[cfg(test)]\nmod tests {\n  #[test]\n  fn t() {}\n}\n";
  let inline = tree(&[("crates/demo/src/lib.rs", text)]);
  let found = check(&context(&inline.workspace));
  assert_eq!(rules(&found), ["test-layout"]);
  assert_eq!(found[0].line, 4);
  assert!(found[0].message.contains("inline #[cfg(test)] module body"));
}

#[test]
fn a_cfg_combinator_counts_and_a_bodyless_declaration_passes() {
  let combinator = "//! demo\n#[cfg(all(test, unix))]\nmod tests {}\n";
  let found = check(&context(
    &tree(&[("crates/demo/src/lib.rs", combinator)]).workspace,
  ));
  assert_eq!(rules(&found), ["test-layout"]);
  let bodyless = "//! demo\n#[cfg(test)]\n#[path = \"tests/lib_test.rs\"]\nmod tests;\n";
  assert_eq!(
    check(&context(
      &tree(&[("crates/demo/src/lib.rs", bodyless)]).workspace
    )),
    Vec::new()
  );
}

#[test]
fn inline_modules_in_test_files_pass() {
  let text = "#[cfg(test)]\nmod inner {\n  #[test]\n  fn t() {}\n}\n";
  let fine = tree(&[("crates/demo/tests/it.rs", text)]);
  assert_eq!(check(&context(&fine.workspace)), Vec::new());
}

#[test]
fn tests_directories_are_flat_except_the_allowed_subdirectories() {
  let nested = tree(&[
    ("crates/demo/tests/deep/it.rs", "#[test]\nfn t() {}\n"),
    ("crates/demo/tests/helpers/fixture.rs", "pub fn f() {}\n"),
    (
      "crates/demo/src/tests/deep/x_test.rs",
      "#[test]\nfn t() {}\n",
    ),
  ]);
  let found = check(&context(&nested.workspace));
  let paths: Vec<&str> = found.iter().map(|finding| finding.path.as_str()).collect();
  assert_eq!(
    paths,
    [
      "crates/demo/src/tests/deep/x_test.rs",
      "crates/demo/tests/deep/it.rs"
    ]
  );
  assert!(found[0].message.contains("fixtures, helpers, common"));
}

#[test]
fn a_unit_test_file_is_named_after_its_module() {
  let misnamed = tree(&[("crates/demo/src/tests/lib.rs", "#[test]\nfn t() {}\n")]);
  let found = check(&context(&misnamed.workspace));
  assert_eq!(rules(&found), ["test-layout"]);
  assert!(found[0].message.contains("<module>_test.rs"));
}
