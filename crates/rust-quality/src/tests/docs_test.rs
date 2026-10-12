use std::path::Path;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;

use super::{docs, undocumented, verbose};
use crate::file::{Limits, RsFile};

#[test]
fn public_items_without_a_doc_are_listed_up_to_three() {
  let lines = [
    "/// Documented.",
    "#[derive(Debug)]",
    "",
    "pub struct A;",
    "pub fn b() {}",
    "pub(crate) const C: u8 = 1;",
    "pub enum D {}",
    "pub mod e;",
    "fn private() {}",
  ];
  assert_eq!(
    undocumented(&lines),
    [
      "5: pub fn b() {}",
      "6: pub(crate) const C: u8 = 1;",
      "7: pub enum D {}"
    ]
  );
  assert_eq!(
    undocumented(&["//! Crate.", "pub use x::y;", "pub fn z() {}"]),
    ["3: pub fn z() {}"]
  );
}

#[test]
fn doc_runs_over_twelve_lines_are_listed_up_to_two() {
  let run = |len: usize| std::iter::repeat_n("/// x", len);
  let lines: Vec<&str> = run(12)
    .chain(["fn a() {}"])
    .chain(run(13))
    .chain([""])
    .chain(run(14))
    .chain([""])
    .chain(run(20))
    .collect();
  assert_eq!(
    verbose(&lines),
    [
      "14: doc block is 13 lines — trim to the essentials",
      "28: doc block is 14 lines — trim to the essentials"
    ]
  );
  assert_eq!(
    verbose(&run(13).collect::<Vec<_>>()),
    ["1: doc block is 13 lines — trim to the essentials"]
  );
}

#[test]
fn the_advisories_apply_to_non_test_src_files() {
  let file = EditedFile {
    path: "src/a.rs".into(),
    absolute: Path::new("/r/src/a.rs").into(),
    removed: false,
  };
  let (limits, env) = (Limits::default(), Env::default());
  let f = RsFile {
    file: &file,
    scope: "/r/src/a.rs",
    lines: vec!["pub fn a() {}"],
    limits: &limits,
    env: &env,
    cwd: Path::new("/r"),
  };
  assert_eq!(
    docs(&f, false),
    [
      "Public items missing a /// doc comment in src/a.rs — add a concise one-line doc:\n1: pub fn a() {}",
      ""
    ]
  );
  let none: [String; 0] = [];
  assert_eq!(docs(&f, true), none);
  let outside = RsFile {
    scope: concat!("/r/build", ".rs"),
    ..f
  };
  assert_eq!(docs(&outside, false), none);
}
