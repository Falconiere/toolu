use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn toolu_serve_lists_its_planned_verbs() {
  let matches = command()
    .try_get_matches_from(["serve", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu serve is not ported yet (#437).\nPlanned verbs: none; the command itself is planned"
    )
  );
}
