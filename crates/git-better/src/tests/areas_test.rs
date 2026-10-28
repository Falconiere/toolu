//! Paths to areas, and changes summed per area.

use super::{AREA_ROOTS, Area, area_of, summarize};
use crate::gitio::FileChange;

fn roots(extra: &[&str]) -> Vec<String> {
  AREA_ROOTS
    .iter()
    .chain(extra)
    .map(|root| (*root).to_owned())
    .collect()
}

#[test]
fn a_path_maps_to_the_longest_root_and_its_next_segment() {
  let plain = roots(&[]);
  let core = roots(&["crates/core"]);
  let cases = [
    ("crates/git-better/src/lib.rs", &plain, "crates/git-better"),
    ("crates/core/shell/src/git.rs", &plain, "crates/core"),
    ("crates/core/shell/src/git.rs", &core, "crates/core/shell"),
    ("plugins/git-better/README.md", &plain, "plugins/git-better"),
    ("crates/Cargo.toml", &plain, "crates"),
    ("docs/cli/README.md", &plain, "docs"),
    ("README.md", &plain, "."),
  ];
  for (path, roots, area) in cases {
    assert_eq!(area_of(path, roots), area, "{path}");
  }
}

#[test]
fn changes_are_summed_per_area_in_order() {
  let change = |path: &str, added: Option<u64>| FileChange {
    path: path.to_owned(),
    old_path: None,
    status: "M".to_owned(),
    added,
    deleted: added.map(|_| 1),
  };
  let found = summarize(
    &[
      change("docs/a.md", Some(3)),
      change("crates/x/src/a.rs", Some(10)),
      change("crates/x/logo.png", None),
      change("docs/b.md", Some(2)),
    ],
    &roots(&[]),
  );
  assert_eq!(
    found,
    [
      Area {
        name: "crates/x".to_owned(),
        files: 2,
        added: 10,
        deleted: 1
      },
      Area {
        name: "docs".to_owned(),
        files: 2,
        added: 5,
        deleted: 2
      },
    ]
  );
}
