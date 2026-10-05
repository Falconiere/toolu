//! The guardrails runner, and the tree and context builders the rule tests share.

use std::path::{Path, PathBuf};

use super::{Context, Finding, check};
use crate::data::{self, Inventory};
use crate::source::Source;
use crate::workspace::{Member, Workspace};

/// This repository, whose real gate data the rule tests use.
pub(crate) const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// A temp directory holding `files`, seen as a workspace of `members`.
pub(crate) struct Tree {
  _dir: tempfile::TempDir,
  pub(crate) workspace: Workspace,
}

/// A member at `dir` named `name`.
pub(crate) fn member(name: &str, dir: &str, is_lib: bool) -> Member {
  Member {
    name: name.to_owned(),
    dir: PathBuf::from(dir),
    is_lib,
  }
}

/// A tree with the single library member `demo` at `crates/demo`.
pub(crate) fn tree(files: &[(&str, &str)]) -> Tree {
  tree_with(vec![member("demo", "crates/demo", true)], files)
}

/// A tree with `members` and `files` written to disk and listed.
pub(crate) fn tree_with(members: Vec<Member>, files: &[(&str, &str)]) -> Tree {
  let dir = tempfile::tempdir().unwrap();
  for (rel, body) in files {
    let path = dir.path().join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
  }
  let mut listed: Vec<PathBuf> = files.iter().map(|(rel, _)| PathBuf::from(rel)).collect();
  listed.sort();
  let workspace = Workspace {
    root: dir.path().to_path_buf(),
    members,
    files: listed,
  };
  Tree {
    _dir: dir,
    workspace,
  }
}

/// The context of `workspace` with this repository's rules, folders and limits
/// and an empty inventory.
pub(crate) fn context(workspace: &Workspace) -> Context<'_> {
  let repo = Path::new(REPO);
  let sources = workspace
    .files_in("crates", "rs")
    .map(|rel| Source::load(workspace, rel).unwrap())
    .collect();
  Context {
    workspace,
    rules: data::rules(repo).unwrap(),
    folders: data::load(repo, "folders.json").unwrap(),
    inventory: Inventory {
      entries: Vec::new(),
    },
    limits: data::limits(repo).unwrap(),
    sources,
  }
}

/// A member manifest that inherits the workspace version and lints.
pub(crate) const MANIFEST: &str =
  "[package]\nname = \"demo\"\nversion.workspace = true\n\n[lints]\nworkspace = true\n";

/// The text of a committed fixture file (`<rel>.fixture` when it is Rust).
pub(crate) fn fixture_text(rule: &str, case: &str, rel: &str) -> String {
  let dir = Path::new(REPO)
    .join("fixtures/guardrails/rust")
    .join(rule)
    .join(case);
  let suffixed = dir.join(format!("{rel}.fixture"));
  std::fs::read_to_string(if suffixed.is_file() {
    suffixed
  } else {
    dir.join(rel)
  })
  .unwrap()
}

/// The rule ids of `found`.
pub(crate) fn rules(found: &[Finding]) -> Vec<&'static str> {
  found.iter().map(|finding| finding.rule).collect()
}

#[test]
fn a_finding_prints_rule_path_line_and_message() {
  let finding = Finding::new(
    "file-length",
    "crates/a/src/lib.rs",
    1,
    "too long".to_owned(),
  );
  assert_eq!(
    finding.to_string(),
    "file-length crates/a/src/lib.rs:1: too long"
  );
}

#[test]
fn the_repository_passes_every_guardrail() {
  let workspace = Workspace::load(&std::fs::canonicalize(REPO).unwrap()).unwrap();
  let ctx = Context::load(&workspace).unwrap();
  let found = check(&ctx).unwrap();
  assert_eq!(found, Vec::new(), "{found:#?}");
}

#[test]
fn findings_come_back_sorted_by_file_and_line() {
  let tree = tree(&[
    ("clippy.toml", "too-many-lines-threshold = 50\n"),
    ("crates/demo/Cargo.toml", MANIFEST),
    ("crates/demo/src/b.rs", "//! b\n// TODO later\n"),
    ("crates/demo/src/a.rs", "//! a\n// FIXME soon\n"),
  ]);
  let mut ctx = context(&tree.workspace);
  ctx.folders.crates.push("demo".to_owned());
  let found = check(&ctx).unwrap();
  let paths: Vec<&str> = found.iter().map(|finding| finding.path.as_str()).collect();
  let mut sorted = paths.clone();
  sorted.sort_unstable();
  assert_eq!(paths, sorted);
  assert_eq!(rules(&found), ["leftovers", "leftovers"]);
}

#[test]
fn a_missing_data_file_is_a_setup_error() {
  let tree = tree(&[]);
  let err = Context::load(&tree.workspace).err().unwrap();
  assert!(err.contains("rules.json"), "{err}");
}
