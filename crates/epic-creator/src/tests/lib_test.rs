use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

#[test]
fn the_epic_creator_crate_is_its_plugin_and_prints_its_guide() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(command().get_name(), "epic-creator");
  let matches = command().try_get_matches_from(["epic-creator"]).unwrap();
  let text = run(&matches, &Ctx::default()).stdout.unwrap_or_default();
  assert!(
    text.starts_with("epic-creator is a Markdown-only plugin"),
    "{text}"
  );
  assert!(text.contains("/epic-creator:epic-creator"), "{text}");
}
