use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn an_unknown_setup_verb_prints_usage() {
  let matches = command()
    .try_get_matches_from(["setup", "agents", "deploy"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(outcome.exit, Exit::Blocked);
  assert_eq!(
    outcome.stderr.as_deref(),
    Some("Usage: toolu setup agents preview | install [--force] | remove --yes [--force]")
  );
  assert_eq!(outcome.stdout, None);
}
