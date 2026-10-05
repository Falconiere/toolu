use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn toolu_setup_lists_its_planned_verbs() {
  let matches = command()
    .try_get_matches_from(["setup", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some("toolu setup is not ported yet (#445).\nPlanned verbs: agents")
  );
}
