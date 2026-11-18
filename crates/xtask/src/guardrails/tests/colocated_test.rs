use super::check;
use crate::guardrails::tests::{context, tree};

const WIRED: &str =
  "//! q\npub fn f() {}\n#[cfg(test)]\n#[path = \"tests/queue_test.rs\"]\nmod tests;\n";

#[test]
fn a_declared_test_module_with_no_file_names_the_expected_path() {
  let missing = tree(&[("crates/demo/src/queue.rs", WIRED)]);
  let found = check(&context(&missing.workspace));
  assert_eq!(found.len(), 1);
  assert_eq!(
    found[0].message,
    "declares tests but has no crates/demo/src/tests/queue_test.rs beside it"
  );
}

#[test]
fn a_wired_test_file_with_a_test_passes() {
  let fine = tree(&[
    ("crates/demo/src/queue.rs", WIRED),
    (
      "crates/demo/src/tests/queue_test.rs",
      "#[test]\nfn pops() {}\n",
    ),
  ]);
  assert_eq!(check(&context(&fine.workspace)), Vec::new());
}

#[test]
fn an_unwired_or_empty_test_file_fails() {
  let unwired = tree(&[
    ("crates/demo/src/queue.rs", "//! q\npub fn f() {}\n"),
    (
      "crates/demo/src/tests/queue_test.rs",
      "#[test]\nfn pops() {}\n",
    ),
  ]);
  let found = check(&context(&unwired.workspace));
  assert!(
    found[0].message.contains("is not wired"),
    "{}",
    found[0].message
  );
  let empty = tree(&[
    ("crates/demo/src/queue.rs", WIRED),
    ("crates/demo/src/tests/queue_test.rs", "fn helper() {}\n"),
  ]);
  let found = check(&context(&empty.workspace));
  assert!(
    found[0].message.ends_with("has no #[test] function"),
    "{}",
    found[0].message
  );
}

#[test]
fn nested_test_modules_look_beside_themselves() {
  let nested = tree(&[
    (
      "crates/demo/src/gates/commit_gate.rs",
      "//! g\nimpl S { fn f(&self) {} }\nstruct S;\n#[cfg(test)]\n#[path = \"tests/commit_gate_test.rs\"]\nmod tests;\n",
    ),
    (
      "crates/demo/src/types.rs",
      "//! t\npub struct T;\ntrait X { fn f(&self); }\n",
    ),
  ]);
  let found = check(&context(&nested.workspace));
  assert_eq!(found.len(), 1);
  assert!(
    found[0]
      .message
      .contains("crates/demo/src/gates/tests/commit_gate_test.rs")
  );
}

#[test]
fn functions_and_default_methods_do_not_require_separate_unit_tests() {
  let with_default = tree(&[(
    "crates/demo/src/x.rs",
    "//! x\npub fn f() {}\nstruct S;\nimpl S { fn f(&self) {} }\ntrait X { fn f(&self) {} }\n",
  )]);
  assert_eq!(check(&context(&with_default.workspace)), Vec::new());
}
