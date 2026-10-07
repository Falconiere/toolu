use std::os::unix::fs::symlink;
use std::path::Path;

use toolu_runtime::registry::ModuleKind;

use super::{list_dir, sorted_names};

fn touch(dir: &Path, name: &str) {
  std::fs::write(dir.join(name), "#!/bin/sh\n").unwrap();
}

#[test]
fn modules_come_in_byte_order_and_un_namespaced_ones_are_rejected() {
  let dir = tempfile::tempdir().unwrap();
  let d = dir.path();
  for name in [
    "b@t__x.sh",
    "a@t__y.js",
    "a@t__z.json",
    ".hidden@t__x.sh",
    "noname.sh",
    "__lead.sh",
    "a b@c__d.sh",
    "readme.txt",
    "B@t__upper.sh",
  ] {
    touch(d, name);
  }
  std::fs::create_dir(d.join("dir@t__d.sh")).unwrap();
  symlink(d.join("b@t__x.sh"), d.join("link@t__l.sh")).unwrap();
  symlink(d.join("missing"), d.join("dangling@t__d.sh")).unwrap();
  let listing = list_dir(d);
  let files: Vec<&str> = listing.entries.iter().map(|e| e.file.as_str()).collect();
  assert_eq!(
    files,
    [
      "B@t__upper.sh",
      "a@t__y.js",
      "a@t__z.json",
      "b@t__x.sh",
      "link@t__l.sh"
    ]
  );
  assert_eq!(listing.rejected, ["__lead.sh", "a b@c__d.sh", "noname.sh"]);
  let first = listing.entries.get(1).unwrap();
  assert_eq!(
    (first.spec.as_str(), first.name.as_str(), first.kind),
    ("a@t", "y", ModuleKind::Esm)
  );
  assert_eq!(first.path, d.join("a@t__y.js"));
}

#[test]
fn an_absent_directory_lists_nothing() {
  let dir = tempfile::tempdir().unwrap();
  let missing = dir.path().join("pre-tools.d");
  assert_eq!(list_dir(&missing), super::Listing::default());
  assert_eq!(sorted_names(&missing), [] as [String; 0]);
}
