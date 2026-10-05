use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn toolu_debug_lists_its_planned_verbs() {
  let matches = command()
    .try_get_matches_from(["debug", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some("toolu debug is not ported yet (#425).\nPlanned verbs: io, log, stack, testfail")
  );
}
