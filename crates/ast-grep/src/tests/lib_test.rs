use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

#[test]
fn the_ast_grep_crate_is_its_plugin_and_lists_the_planned_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command()
    .try_get_matches_from(["ast-grep", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu ast-grep is not ported yet (#429). Planned verbs: search, files, scan, debug, savings"
    )
  );
}
