use std::path::{Path, PathBuf};

use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickRequest};
use toolu_runtime::cli::Ctx;

use super::{PLUGIN, Tick, command, run};

#[test]
fn the_pr_babysit_crate_is_its_plugin_and_lists_the_planned_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command()
    .try_get_matches_from(["babysit", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu babysit is not ported yet (#433). Planned verbs: tick, collect, record, reply, resolve, route-fix, dispatch-fix"
    )
  );
}

#[test]
fn tick_is_not_ported_and_names_its_issue() {
  let tick: &dyn BabysitTick = &Tick;
  let request = TickRequest {
    repo: "Falconiere/toolu".into(),
    number: 460,
    state_file: PathBuf::from("/state/460.json"),
    now: None,
  };
  assert_eq!(
    tick.tick(&request),
    Err(LinkError::NotPorted { issue: 433 })
  );
}
