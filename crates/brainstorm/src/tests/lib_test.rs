use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

const TEXT: &str = "brainstorm is a Markdown-only plugin: it has no verbs. Its skill thinks a change through \
   before building: evidence-backed triage, alternatives, trade-offs and a recommended \
   approach, without editing code.\n\nRun it as /brainstorm:brainstorm in Claude Code or \
   $brainstorm:brainstorm in Codex. delivery-flow runs it as its first phase.";

#[test]
fn the_brainstorm_crate_is_its_plugin_and_prints_its_guide() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(command().get_name(), "brainstorm");
  let matches = command().try_get_matches_from(["brainstorm"]).unwrap();
  assert_eq!(run(&matches, &Ctx::default()).stdout.as_deref(), Some(TEXT));
}
