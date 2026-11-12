use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::{build, removed_names, render};
use crate::shape::shape;
use crate::shape::tests::{commit, git, isolated_env as env};

fn write(root: &Path, path: &str, text: &str) {
  let path = root.join(path);
  fs::create_dir_all(path.parent().expect("parent")).expect("dir");
  fs::write(path, text).expect("write");
}

/// A repository on `feature` whose base commit on `main` holds `files`.
fn repo(files: &[(&str, &str)]) -> TempDir {
  let dir = tempfile::tempdir().expect("temp repo");
  git(dir.path(), &["init", "-q", "-b", "main"]);
  git(dir.path(), &["config", "user.name", "Test"]);
  git(dir.path(), &["config", "user.email", "test@example.com"]);
  for (path, text) in files {
    write(dir.path(), path, text);
  }
  commit(dir.path());
  git(dir.path(), &["switch", "-qc", "feature"]);
  dir
}

fn digest_text(root: &Path) -> String {
  let shaped = shape(&env(), root, Some("main")).expect("shaped");
  render::text(&build(&env(), root, &shaped))
}

#[test]
fn digest_names_items_cuts_long_hunks_and_counts_extra_hunks() {
  let numbered: String = (1..=20).fold(String::new(), |mut out, n| {
    let _ = writeln!(out, "line {n}");
    out
  });
  let dir = repo(&[
    (
      "src/lib.rs",
      "pub fn keep() -> u32 {\n  1\n}\n\npub fn gone() {}\n",
    ),
    ("notes.txt", &numbered),
  ]);
  let root = dir.path();
  write(root, "src/lib.rs", "pub fn keep() -> u32 {\n  2\n}\n");
  let body: String = (1..=24).fold(String::new(), |mut out, n| {
    let _ = writeln!(out, "  let _x{n} = {n};");
    out
  });
  write(
    root,
    "src/long.rs",
    &format!("pub fn long() {{\n{body}}}\n"),
  );
  write(
    root,
    "notes.txt",
    &numbered
      .replace("line 2\n", "two\n")
      .replace("line 6\n", "six\n")
      .replace("line 10\n", "ten\n")
      .replace("line 14\n", "fourteen\n"),
  );
  commit(root);
  let text = digest_text(root);
  for want in [
    ", 3 files, +31 -7",
    "\nsrc/lib.rs +1 -3 (modified)\n  items: keep 1-3\n  removed: gone\n",
    "\nsrc/long.rs +26 -0 (added)\n  items: long 1-26 (new)\n  @@ -0,0 +1,26 @@\n  +pub fn long() {\n",
    "\n  +  let _x7 = 7;\n  ... 18 more lines: read new 9-26",
    "\nnotes.txt +4 -4 (modified)\n",
    "\n  @@ -10 +10 @@ line 9\n  -line 10\n  +ten\n  ... 1 more hunks\n",
  ] {
    assert!(text.contains(want), "missing {want:?} in:\n{text}");
  }
  assert!(!text.contains("fourteen"), "{text}");
}

#[test]
fn digest_past_the_hunk_budget_prints_counts_and_the_path_hint() {
  let dir = repo(&[("a.txt", "a\n")]);
  let root = dir.path();
  let long = format!("{}\n", "y".repeat(300));
  for name in ["f1", "f2", "f3", "f4", "f5"] {
    write(root, &format!("{name}.txt"), &long.repeat(8));
  }
  commit(root);
  let text = digest_text(root);
  assert!(
    text.contains("\nf3.txt +8 -0 (added)\n  @@ -0,0 +1,8 @@\n"),
    "{text}"
  );
  assert!(
    text.contains("\nf4.txt +8 -0 (added)\nf5.txt +8 -0 (added)\n"),
    "{text}"
  );
  assert!(
    text.ends_with("\nhunks omitted for 2 files past 6 KB; pass --path to see them"),
    "{text}"
  );
}

#[test]
fn a_changed_signature_is_not_a_removal_without_an_outline() {
  let dir = repo(&[(
    "src/lib.rs",
    "pub fn keep() -> u32 {\n  1\n}\n\npub fn gone() {}\n",
  )]);
  let root = dir.path();
  write(root, "src/lib.rs", "pub fn keep() -> u64 {\n  1\n}\n");
  commit(root);
  let shaped = shape(&env(), root, Some("main")).expect("shaped");
  assert_eq!(removed_names(&shaped.files[0], None), ["gone"]);
}
