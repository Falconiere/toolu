//! Shared options for task unit tests.

use std::path::PathBuf;

use crate::options::Options;

/// Workspace root for task tests.
pub(crate) fn test_root() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Options rooted at `root`.
pub(crate) fn test_options(root: PathBuf) -> Options {
  Options {
    root,
    ..Options::default()
  }
}

#[test]
fn test_options_keeps_the_given_root() {
  let root = test_root();
  let options = test_options(root.clone());
  assert_eq!(options.root, root);
}
