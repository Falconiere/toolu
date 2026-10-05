use std::path::{Path, PathBuf};

use super::unused;
use crate::source::Source;
use crate::workspace::Member;

fn member(name: &str, dir: &str, is_lib: bool) -> Member {
  Member {
    name: name.to_owned(),
    dir: PathBuf::from(dir),
    is_lib,
  }
}

#[test]
fn public_items_need_a_user_in_another_crate_or_a_test() {
  let lib = member("toolu-a", "crates/a", true);
  let other = member("toolu-b", "crates/b", true);
  let items = "//! a\npub fn used_elsewhere() {}\npub struct UsedInTest;\npub const ORPHAN: u8 = 1;\npub(crate) fn private() {}\npub enum Shape {}\npub trait Speak {}\npub static S: u8 = 1;\npub type T = u8;\npub union U { a: u8 }\nfn caller() { used_elsewhere(); ORPHAN; }\n";
  let sources = [
    Source::new(Path::new("crates/a/src/lib.rs"), Some(&lib), items),
    Source::new(
      Path::new("crates/a/tests/it.rs"),
      Some(&lib),
      "fn t() { UsedInTest; Shape; Speak; S; T; U; }",
    ),
    Source::new(
      Path::new("crates/b/src/lib.rs"),
      Some(&other),
      "//! b\nfn f() { toolu_a::used_elsewhere(); }",
    ),
  ];
  assert_eq!(
    unused(&sources),
    ["unused-pub crates/a/src/lib.rs:4: ORPHAN is used by no other crate and no test"]
  );
}

#[test]
fn binaries_tests_and_unparsable_files_are_skipped() {
  let bin = member("xtask", "crates/xtask", false);
  let lib = member("toolu-a", "crates/a", true);
  let sources = [
    Source::new(
      Path::new("crates/xtask/src/main.rs"),
      Some(&bin),
      "pub fn f() {}",
    ),
    Source::new(
      Path::new("crates/a/tests/it.rs"),
      Some(&lib),
      "pub fn helper() {}",
    ),
    Source::new(Path::new("crates/a/src/lib.rs"), Some(&lib), "pub fn ("),
    Source::new(Path::new("loose.rs"), None, "pub fn loose() {}"),
  ];
  assert_eq!(unused(&sources), Vec::<String>::new());
}
