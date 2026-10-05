use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn toolu_config_lists_its_planned_verbs() {
  let matches = command()
    .try_get_matches_from(["config", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some("toolu config is not ported yet (#445). Planned verbs: get, set, validate")
  );
}
