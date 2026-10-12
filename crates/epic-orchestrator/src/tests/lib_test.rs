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
fn the_epic_orchestrator_crate_is_its_plugin_and_dispatches_its_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert!(command().try_get_matches_from(["epic", "planned"]).is_err());
  let matches = command()
    .try_get_matches_from(["epic", "gate", "--pr", "nonsense"])
    .expect("epic gate matches");
  let outcome = run(&matches, &Ctx::default(), &Unported);
  assert_eq!(outcome.exit, toolu_protocol::exit::Exit::Usage, "{outcome:?}");
}
