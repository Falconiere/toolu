use std::path::Path;

use super::{PLUGIN, command};

#[test]
fn the_toolu_review_crate_owns_the_native_writer() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command()
    .try_get_matches_from(["review", "write-state", "--findings-count", "0"])
    .unwrap();
  assert_eq!(matches.subcommand_name(), Some("write-state"));
}
