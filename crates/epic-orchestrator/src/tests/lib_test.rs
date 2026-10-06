use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

#[test]
fn the_epic_orchestrator_crate_is_its_plugin_and_lists_the_planned_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command().try_get_matches_from(["epic", "planned"]).unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu epic is not ported yet (#434, #435, #448). Planned verbs: engine, start, status, pause, resume, ack, answer, wait, report, job, graph, route, launch, finish, close, release, jira, probe, gate, queue"
    )
  );
}
