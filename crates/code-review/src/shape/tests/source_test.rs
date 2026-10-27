use std::collections::BTreeSet;
use std::fs;

use tempfile::tempdir;

use super::{batch_generated, parse_matches};
use crate::shape::FileKind;
use crate::shape::parse::Status;
use crate::shape::tests::{git, isolated_env};

fn status(kind: FileKind, old_path: Option<&str>, path: &str) -> Status {
  Status {
    kind,
    old_path: old_path.map(str::to_owned),
    path: path.to_owned(),
  }
}

#[test]
fn grep_results_keep_newlines_in_paths_and_filter_header_lines() {
  let expected = BTreeSet::from(["a\nb.rs", "plain.rs"]);
  let bytes = b"HEAD:a\nb.rs\x005\x00// @generated\nHEAD:plain.rs\x006\x00// @generated\n";
  let generated = parse_matches(bytes, "HEAD", &expected).expect("parse");
  assert_eq!(generated, BTreeSet::from(["a\nb.rs".to_owned()]));
}

#[test]
fn batch_reads_base_attributes_and_requires_both_sides_of_a_changed_file() {
  let dir = tempdir().expect("repo");
  let root = dir.path();
  for args in [
    vec!["init", "-b", "main"],
    vec!["config", "user.name", "Test"],
    vec!["config", "user.email", "test@example.com"],
  ] {
    git(root, &args);
  }
  fs::write(
    root.join(".gitattributes"),
    "attr.rs linguist-generated\nlinked/** linguist-generated\n",
  )
  .expect("base attributes");
  fs::write(root.join("kept.rs"), "// @generated\nold\n").expect("base marker");
  for name in ["hand.rs", "moved.rs"] {
    fs::write(root.join(name), "code\n").expect("base source");
  }
  git(root, &["add", "-A"]);
  git(root, &["commit", "-m", "base"]);
  let base = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
  fs::write(root.join(".gitattributes"), "* linguist-generated\n").expect("branch attributes");
  for name in ["attr.rs", "plain.rs"] {
    fs::write(root.join(name), "code\n").expect("source");
  }
  fs::write(root.join("gen.rs"), "// @generated\ncode\n").expect("marker");
  fs::write(root.join("kept.rs"), "// @generated\nnew\n").expect("regenerated");
  fs::write(root.join("hand.rs"), "// @generated\nevil\n").expect("branch marker");
  fs::create_dir(root.join("linked")).expect("generated dir");
  git(root, &["mv", "moved.rs", "linked/moved.rs"]);
  git(root, &["add", "-A"]);
  git(root, &["commit", "-m", "branch"]);
  let statuses = [
    status(FileKind::Modified, None, ".gitattributes"),
    status(FileKind::Added, None, "attr.rs"),
    status(FileKind::Added, None, "gen.rs"),
    status(FileKind::Added, None, "plain.rs"),
    status(FileKind::Modified, None, "kept.rs"),
    status(FileKind::Modified, None, "hand.rs"),
    status(FileKind::Renamed, Some("moved.rs"), "linked/moved.rs"),
  ];
  let refs: Vec<&Status> = statuses.iter().collect();
  assert_eq!(
    batch_generated(&isolated_env(), root, &refs, &base),
    Some(BTreeSet::from([
      "attr.rs".to_owned(),
      "gen.rs".to_owned(),
      "kept.rs".to_owned()
    ]))
  );
}
