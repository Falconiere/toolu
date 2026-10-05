use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

const TEXT: &str = "delivery-flow is a Markdown-only plugin: it has no verbs. Its one skill delivers a task \
   end to end: brainstorm, an approved spec and plan, real-data execution, the pull request \
   and the pr-babysit handoff.\n\nRun it as /delivery-flow:delivery-flow in Claude Code or \
   $delivery-flow:delivery-flow in Codex.";

#[test]
fn the_delivery_flow_crate_is_its_plugin_and_prints_its_guide() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(command().get_name(), "delivery-flow");
  let matches = command().try_get_matches_from(["delivery-flow"]).unwrap();
  assert_eq!(run(&matches, &Ctx::default()).stdout.as_deref(), Some(TEXT));
}
