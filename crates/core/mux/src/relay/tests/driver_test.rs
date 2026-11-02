use super::{quote, terminal_name, tick};
use toolu_runtime::env::Env;

#[test]
fn terminal_names_are_stable_and_fit_hc_limit() {
  assert_eq!(
    terminal_name("12345678-1234-1234-1234-123456789abc"),
    "td-12345678123412341234123456"
  );
}

#[test]
fn report_paths_are_shell_quoted() {
  assert_eq!(quote("/a b/it's"), "'/a b/it'\\''s'");
}

#[test]
fn empty_registry_tick_is_empty() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(tick(dir.path(), &Env::default()).unwrap(), Vec::new());
}
