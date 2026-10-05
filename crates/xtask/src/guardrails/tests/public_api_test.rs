use super::check;
use crate::guardrails::tests::{context, member, tree, tree_with};

#[test]
fn a_public_fn_returning_a_type_erased_error_fails() {
  let source =
    "//! d\npub fn parse() -> Result<u8, Box<dyn std::error::Error + Send>> {\n  Ok(1)\n}\n";
  let found = check(&context(
    &tree(&[("crates/demo/src/lib.rs", source)]).workspace,
  ));
  assert_eq!(found.len(), 1);
  assert_eq!((found[0].rule, found[0].line), ("public-api", 2));
  assert!(
    found[0]
      .message
      .starts_with("pub fn parse returns or takes")
  );
}

#[test]
fn methods_and_parameters_count_but_private_and_typed_errors_pass() {
  let source = "//! d\npub struct S;\nimpl S {\n  pub fn take(&self, _e: &dyn Error) {}\n  fn hidden(&self) -> Box<dyn Error> { todo() }\n}\npub fn typed() -> Result<u8, std::io::Error> { Ok(1) }\npub fn display(_d: &dyn std::fmt::Display) {}\npub fn sized(_d: &(dyn Error + 'static)) {}\n";
  let found = check(&context(
    &tree(&[("crates/demo/src/lib.rs", source)]).workspace,
  ));
  let names: Vec<&str> = found.iter().map(|f| f.message.as_str()).collect();
  assert_eq!(names.len(), 2, "{names:?}");
  assert!(names[0].starts_with("pub fn take"));
  assert!(names[1].starts_with("pub fn sized"));
}

#[test]
fn binaries_and_tests_are_not_library_api() {
  let source = "//! x\npub fn run() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }\n";
  let bin = tree_with(
    vec![member("xtask", "crates/xtask", false)],
    &[("crates/xtask/src/main.rs", source)],
  );
  assert_eq!(check(&context(&bin.workspace)), Vec::new());
  let test = tree(&[("crates/demo/tests/it.rs", source)]);
  assert_eq!(check(&context(&test.workspace)), Vec::new());
}
