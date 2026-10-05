use super::check;
use crate::guardrails::tests::{context, fixture_text, rules, tree};

const LIB: &str = "crates/demo/src/lib.rs";

/// The findings for `crates/demo/src/lib.rs` taken from a committed fixture case.
fn lib_findings(rule: &str, case: &str) -> Vec<crate::guardrails::Finding> {
  let text = fixture_text(rule, case, LIB);
  check(&context(&tree(&[(LIB, &text)]).workspace))
}

#[test]
fn every_suppression_form_is_a_finding_with_its_line() {
  let found = lib_findings("suppression", "violating");
  assert_eq!(rules(&found), ["suppression"]);
  assert_eq!(found[0].line, 8);
  for case in [
    "violating-expect",
    "violating-inner",
    "violating-cfg-attr",
    "violating-jscpd",
  ] {
    let found = lib_findings("suppression", case);
    assert!(
      !found.is_empty() && found.iter().all(|f| f.rule == "suppression"),
      "{case}: {found:?}"
    );
  }
}

#[test]
fn ignore_include_and_stray_path_attributes_are_findings() {
  let ignored = fixture_text(
    "real-tests",
    "violating-ignore",
    "crates/demo/src/tests/lib_test.rs",
  );
  let found = check(&context(
    &tree(&[("crates/demo/src/tests/lib_test.rs", &ignored)]).workspace,
  ));
  assert_eq!(rules(&found), ["no-ignore"]);
  assert_eq!(
    rules(&lib_findings("structure", "violating-include")),
    ["structure"]
  );
  let found = lib_findings("structure", "violating-path");
  assert!(found[0].message.contains("is only for test wiring"));
}

#[test]
fn test_wiring_paths_pass_where_they_belong() {
  let clean = fixture_text("base", ".", LIB);
  assert_eq!(
    check(&context(&tree(&[(LIB, &clean)]).workspace)),
    Vec::new()
  );
  let helper = "#[path = \"helpers/fixture.rs\"]\nmod fixture;\n";
  let fine = tree(&[("crates/demo/tests/it.rs", helper)]);
  assert_eq!(check(&context(&fine.workspace)), Vec::new());
  let stray = "#[path = \"fixtures/data.rs\"]\nmod data;\n";
  let found = check(&context(
    &tree(&[("crates/demo/tests/it.rs", stray)]).workspace,
  ));
  assert_eq!(rules(&found), ["structure"]);
  let other = check(&context(&tree(&[("other/x.rs", helper)]).workspace));
  assert_eq!(
    other,
    Vec::new(),
    "files outside crates/ are not read: {other:?}"
  );
}

#[test]
fn a_path_into_a_nested_tests_directory_is_not_wiring() {
  let nested = "//! x\n#[cfg(test)]\n#[path = \"tests/deep/x_test.rs\"]\nmod tests;\n";
  let found = check(&context(&tree(&[(LIB, nested)]).workspace));
  assert_eq!(rules(&found), ["structure"]);
}
