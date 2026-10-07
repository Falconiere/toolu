use std::path::Path;

use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickReport, TickRequest};
use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

/// A tick the placeholder never runs.
struct Unported;

impl BabysitTick for Unported {
  fn tick(&self, _request: &TickRequest) -> Result<TickReport, LinkError> {
    Err(LinkError::NotPorted { issue: 433 })
  }
}

#[test]
fn the_epic_orchestrator_crate_is_its_plugin_and_lists_the_planned_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command()
    .try_get_matches_from(["epic", "planned"])
    .expect("epic planned matches");
  let outcome = run(&matches, &Ctx::default(), &Unported);
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu epic is not ported yet (#435, #448). Planned verbs: graph, route, launch, finish, close, release, jira, probe, gate, queue"
    )
  );
}
