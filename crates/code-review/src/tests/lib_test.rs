use std::path::Path;

use super::{PLUGIN, command};

#[test]
fn the_code_review_crate_is_its_plugin() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(command().get_name(), PLUGIN);
}
