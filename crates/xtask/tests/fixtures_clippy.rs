//! Quality-bar fixtures judged by rustfmt, clippy and rustdoc through `cargo xtask
//! gate`, with this repository's real lint levels and thresholds.

#[path = "helpers/fixture.rs"]
mod fixture;

#[test]
fn capabilities_violating_exit() {
  fixture::check("capabilities", "violating-exit").unwrap();
}

#[test]
fn capabilities_violating_print() {
  fixture::check("capabilities", "violating-print").unwrap();
}

#[test]
fn complexity_clean() {
  fixture::check("complexity", "clean").unwrap();
}

#[test]
fn complexity_violating() {
  fixture::check("complexity", "violating").unwrap();
}

#[test]
fn complexity_violating_arguments() {
  fixture::check("complexity", "violating-arguments").unwrap();
}

#[test]
fn complexity_violating_bool_fields() {
  fixture::check("complexity", "violating-bool-fields").unwrap();
}

#[test]
fn complexity_violating_bool_params() {
  fixture::check("complexity", "violating-bool-params").unwrap();
}

#[test]
fn complexity_violating_nesting() {
  fixture::check("complexity", "violating-nesting").unwrap();
}

#[test]
fn dead_code_violating() {
  fixture::check("dead-code", "violating").unwrap();
}

#[test]
fn dead_code_violating_unreachable_pub() {
  fixture::check("dead-code", "violating-unreachable-pub").unwrap();
}

#[test]
fn docs_clean() {
  fixture::check("docs", "clean").unwrap();
}

#[test]
fn docs_violating() {
  fixture::check("docs", "violating").unwrap();
}

#[test]
fn docs_violating_rustdoc() {
  fixture::check("docs", "violating-rustdoc").unwrap();
}

#[test]
fn fn_length_clean() {
  fixture::check("fn-length", "clean").unwrap();
}

#[test]
fn fn_length_violating() {
  fixture::check("fn-length", "violating").unwrap();
}

#[test]
fn format_clean() {
  fixture::check("format", "clean").unwrap();
}

#[test]
fn format_clean_nested() {
  fixture::check("format", "clean-nested").unwrap();
}

#[test]
fn format_violating() {
  fixture::check("format", "violating").unwrap();
}

#[test]
fn format_violating_tab() {
  fixture::check("format", "violating-tab").unwrap();
}

#[test]
fn leftovers_violating_dbg() {
  fixture::check("leftovers", "violating-dbg").unwrap();
}

#[test]
fn panics_clean() {
  fixture::check("panics", "clean").unwrap();
}

#[test]
fn panics_violating() {
  fixture::check("panics", "violating").unwrap();
}

#[test]
fn panics_violating_index() {
  fixture::check("panics", "violating-index").unwrap();
}

#[test]
fn unsafe_clean() {
  fixture::check("unsafe", "clean").unwrap();
}

#[test]
fn unsafe_violating() {
  fixture::check("unsafe", "violating").unwrap();
}
