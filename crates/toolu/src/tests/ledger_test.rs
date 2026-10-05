use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn toolu_ledger_lists_its_planned_verbs() {
  let matches = command()
    .try_get_matches_from(["ledger", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu ledger is not ported yet (#421).\nPlanned verbs: run, status, preflight, path, root, self-test, verdict"
    )
  );
}
