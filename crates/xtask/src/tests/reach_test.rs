use std::path::PathBuf;

use super::unreached;
use crate::workspace::{Member, Workspace};

#[test]
fn rust_files_outside_every_member_are_named() {
  let workspace = Workspace {
    root: PathBuf::from("/r"),
    members: vec![Member {
      name: "demo".to_owned(),
      dir: PathBuf::from("crates/demo"),
      is_lib: true,
    }],
    files: [
      "crates/demo/src/lib.rs",
      "crates/demo/fuzz/fuzz_targets/f.rs",
      "tools/x.rs",
      "tools/y.ts",
    ]
    .iter()
    .map(PathBuf::from)
    .collect(),
  };
  assert_eq!(
    unreached(&workspace),
    ["reach tools/x.rs: outside every workspace member"]
  );
}
