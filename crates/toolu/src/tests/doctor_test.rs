use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn toolu_doctor_lists_its_planned_verbs() {
  let matches = command()
    .try_get_matches_from(["doctor", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu doctor is not ported yet (#445).\nPlanned verbs: none; the command itself is planned"
    )
  );
}
