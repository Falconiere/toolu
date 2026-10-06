use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;

use super::{detect_ast_grep, tool_available};

const TRUE: &str = "/usr/bin/true";

fn env_of(path: &str) -> Env {
  Env::from_pairs([("PATH", path)])
}

/// A bin dir `label` under `root` holding real executables under `names`.
fn bin(root: &Path, label: &str, names: &[&str]) -> PathBuf {
  let dir = root.join(label);
  std::fs::create_dir_all(&dir).unwrap();
  for name in names {
    symlink(TRUE, dir.join(name)).unwrap();
  }
  dir
}

#[test]
fn ast_grep_is_sg_or_ast_grep_on_the_path() {
  let root = tempfile::tempdir().unwrap();
  let cases: [(&str, &[&str], bool); 4] = [
    ("neither", &[], false),
    ("sg-only", &["sg"], true),
    ("ast-grep-only", &["ast-grep"], true),
    ("both", &["sg", "ast-grep"], true),
  ];
  for (label, names, found) in cases {
    let dir = bin(root.path(), label, names);
    assert_eq!(
      detect_ast_grep(&env_of(dir.to_str().unwrap())),
      found,
      "{label}"
    );
  }
}

#[test]
fn executables_plain_files_directories_and_missing_names_are_told_apart() {
  let root = tempfile::tempdir().unwrap();
  let dir = bin(root.path(), "bin", &["exe"]);
  std::fs::write(dir.join("plain"), "#!/bin/sh\n").unwrap();
  std::fs::create_dir(dir.join("adir")).unwrap();
  symlink(root.path().join("missing"), dir.join("dangling")).unwrap();
  let env = env_of(dir.to_str().unwrap());
  let on_path = ["exe", "plain", "adir", "dangling", "nope", "git", ""];
  let found: Vec<bool> = on_path
    .iter()
    .map(|name| tool_available(name, &env))
    .collect();
  // A non-executable file on PATH still counts, as `command -v` reports it.
  assert_eq!(found, [true, true, false, false, false, false, false]);
}

#[test]
fn a_name_with_a_slash_must_be_an_executable_non_directory() {
  let root = tempfile::tempdir().unwrap();
  let dir = bin(root.path(), "bin", &["exe"]);
  std::fs::write(dir.join("plain"), "#!/bin/sh\n").unwrap();
  std::fs::create_dir(dir.join("adir")).unwrap();
  let env = env_of("");
  for (name, found) in [
    ("exe", true),
    ("plain", false),
    ("adir", false),
    ("nope", false),
  ] {
    assert_eq!(
      tool_available(dir.join(name).to_str().unwrap(), &env),
      found,
      "{name}"
    );
  }
}

#[test]
fn an_empty_path_entry_is_the_current_directory() {
  let root = tempfile::tempdir().unwrap();
  let dir = bin(root.path(), "bin", &[]);
  // `cargo test` runs from the package directory, which holds a Cargo.toml.
  assert!(tool_available("Cargo.toml", &env_of("")));
  assert!(tool_available(
    "Cargo.toml",
    &env_of(&format!("{}:", dir.display()))
  ));
  assert!(tool_available("Cargo.toml", &Env::default()));
  assert!(!tool_available(
    "Cargo.toml",
    &env_of(dir.to_str().unwrap())
  ));
}

#[test]
fn answers_are_cached_per_path_so_a_new_path_re_probes() {
  let root = tempfile::tempdir().unwrap();
  let without = bin(root.path(), "without", &[]);
  let with = bin(root.path(), "with", &["sg"]);
  assert!(!detect_ast_grep(&env_of(without.to_str().unwrap())));
  assert!(detect_ast_grep(&env_of(with.to_str().unwrap())));
  assert!(!detect_ast_grep(&env_of(without.to_str().unwrap())));
  // The same absolute PATH keeps its first answer; a changed PATH is a new key.
  symlink(TRUE, without.join("sg")).unwrap();
  assert!(!tool_available("sg", &env_of(without.to_str().unwrap())));
  assert!(tool_available(
    "sg",
    &env_of(&format!("{}:{}", without.display(), with.display()))
  ));
}

#[test]
fn a_relative_path_entry_is_never_cached() {
  let root = tempfile::tempdir().unwrap();
  let dir = bin(root.path(), "bin", &[]);
  // Enough `..` to reach `/` from any directory: relative, yet the same place.
  let relative = format!(
    "{}{}",
    "../".repeat(64),
    dir.strip_prefix("/").unwrap().display()
  );
  let env = env_of(&relative);
  assert!(!tool_available("later", &env));
  symlink(TRUE, dir.join("later")).unwrap();
  assert!(tool_available("later", &env));
}
