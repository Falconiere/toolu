use std::path::Path;

use super::{PLUGIN, command};

#[test]
fn ast_grep_crate_exposes_five_real_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let tree = command();
  let verbs: Vec<&str> = tree
    .get_subcommands()
    .map(clap::Command::get_name)
    .collect();
  assert_eq!(verbs, ["search", "files", "scan", "debug", "savings"]);
}
