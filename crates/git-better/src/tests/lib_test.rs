//! The crate's surface: the plugin name, its namespace and the stdin probe.

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, needs_stdin, run};

#[test]
fn the_namespace_is_the_plugin_and_check_runs_through_it() {
  assert_eq!(command().get_name(), PLUGIN);
  let stdin = command()
    .try_get_matches_from(["git-better", "check", "-F", "-"])
    .unwrap();
  assert!(needs_stdin(&stdin));
  let matches = command()
    .try_get_matches_from(["git-better", "check", "--branch", "feat/1-x"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stdout.as_deref(), Some("semantic: ok"));
}
