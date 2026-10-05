//! Quality-bar fixtures judged by `cargo xtask guardrails`: each rule's clean case
//! passes and each violating case fails with its documented message.

#[path = "helpers/fixture.rs"]
mod fixture;

#[test]
fn capabilities_clean_owner() {
  fixture::check("capabilities", "clean-owner").unwrap();
}

#[test]
fn capabilities_violating() {
  fixture::check("capabilities", "violating").unwrap();
}

#[test]
fn capabilities_violating_alias() {
  fixture::check("capabilities", "violating-alias").unwrap();
}

#[test]
fn capabilities_violating_glob() {
  fixture::check("capabilities", "violating-glob").unwrap();
}

#[test]
fn capabilities_violating_process() {
  fixture::check("capabilities", "violating-process").unwrap();
}

#[test]
fn capabilities_violating_stdout() {
  fixture::check("capabilities", "violating-stdout").unwrap();
}

#[test]
fn colocated_tests_clean() {
  fixture::check("colocated-tests", "clean").unwrap();
}

#[test]
fn colocated_tests_violating() {
  fixture::check("colocated-tests", "violating").unwrap();
}

#[test]
fn crate_hygiene_clean() {
  fixture::check("crate-hygiene", "clean").unwrap();
}

#[test]
fn crate_hygiene_violating() {
  fixture::check("crate-hygiene", "violating").unwrap();
}

#[test]
fn crate_hygiene_violating_doc() {
  fixture::check("crate-hygiene", "violating-doc").unwrap();
}

#[test]
fn crate_hygiene_violating_version() {
  fixture::check("crate-hygiene", "violating-version").unwrap();
}

#[test]
fn data_violating_ignore_key() {
  fixture::check("data", "violating-ignore-key").unwrap();
}

#[test]
fn data_violating_rules_key() {
  fixture::check("data", "violating-rules-key").unwrap();
}

#[test]
fn dependencies_violating_public_api() {
  fixture::check("dependencies", "violating-public-api").unwrap();
}

#[test]
fn file_length_clean() {
  fixture::check("file-length", "clean").unwrap();
}

#[test]
fn file_length_violating() {
  fixture::check("file-length", "violating").unwrap();
}

#[test]
fn fuzz_violating() {
  fixture::check("fuzz", "violating").unwrap();
}

#[test]
fn impl_length_clean() {
  fixture::check("impl-length", "clean").unwrap();
}

#[test]
fn impl_length_violating() {
  fixture::check("impl-length", "violating").unwrap();
}

#[test]
fn inventory_clean() {
  fixture::check("inventory", "clean").unwrap();
}

#[test]
fn inventory_violating() {
  fixture::check("inventory", "violating").unwrap();
}

#[test]
fn inventory_violating_unresolved() {
  fixture::check("inventory", "violating-unresolved").unwrap();
}

#[test]
fn leftovers_clean() {
  fixture::check("leftovers", "clean").unwrap();
}

#[test]
fn leftovers_violating() {
  fixture::check("leftovers", "violating").unwrap();
}

#[test]
fn real_tests_violating_ignore() {
  fixture::check("real-tests", "violating-ignore").unwrap();
}

#[test]
fn secrets_clean() {
  fixture::check("secrets", "clean").unwrap();
}

#[test]
fn secrets_violating() {
  fixture::check("secrets", "violating").unwrap();
}

#[test]
fn structure_clean() {
  fixture::check("structure", "clean").unwrap();
}

#[test]
fn structure_violating() {
  fixture::check("structure", "violating").unwrap();
}

#[test]
fn structure_violating_build() {
  fixture::check("structure", "violating-build").unwrap();
}

#[test]
fn structure_violating_crate_entry() {
  fixture::check("structure", "violating-crate-entry").unwrap();
}

#[test]
fn structure_violating_depth() {
  fixture::check("structure", "violating-depth").unwrap();
}

#[test]
fn structure_violating_include() {
  fixture::check("structure", "violating-include").unwrap();
}

#[test]
fn structure_violating_main() {
  fixture::check("structure", "violating-main").unwrap();
}

#[test]
fn structure_violating_name() {
  fixture::check("structure", "violating-name").unwrap();
}

#[test]
fn structure_violating_path() {
  fixture::check("structure", "violating-path").unwrap();
}

#[test]
fn structure_violating_plugin_bin() {
  fixture::check("structure", "violating-plugin-bin").unwrap();
}

#[test]
fn structure_violating_root() {
  fixture::check("structure", "violating-root").unwrap();
}

#[test]
fn suppression_clean() {
  fixture::check("suppression", "clean").unwrap();
}

#[test]
fn suppression_violating() {
  fixture::check("suppression", "violating").unwrap();
}

#[test]
fn suppression_violating_cfg_attr() {
  fixture::check("suppression", "violating-cfg-attr").unwrap();
}

#[test]
fn suppression_violating_expect() {
  fixture::check("suppression", "violating-expect").unwrap();
}

#[test]
fn suppression_violating_inner() {
  fixture::check("suppression", "violating-inner").unwrap();
}

#[test]
fn suppression_violating_jscpd() {
  fixture::check("suppression", "violating-jscpd").unwrap();
}

#[test]
fn test_layout_clean() {
  fixture::check("test-layout", "clean").unwrap();
}

#[test]
fn test_layout_violating() {
  fixture::check("test-layout", "violating").unwrap();
}

#[test]
fn test_layout_violating_nested() {
  fixture::check("test-layout", "violating-nested").unwrap();
}
