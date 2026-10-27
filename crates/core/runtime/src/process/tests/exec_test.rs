use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use super::{DEFAULT_PATH, exec, resolve};
use crate::env::Env;
use crate::process::RunError;

fn argv(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn an_empty_argv_or_program_is_refused_before_anything_starts() {
  assert_eq!(exec(&[], None, None), RunError::EmptyArgv);
  assert_eq!(exec(&argv(&["", "x"]), None, None), RunError::EmptyArgv);
}

#[test]
fn a_program_missing_from_path_is_refused_by_name() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().to_str().unwrap();
  let env = Env::from_pairs([("PATH", path)]);
  assert_eq!(
    exec(&argv(&["toolu-no-such-program"]), None, Some(&env)),
    RunError::Spawn("toolu-no-such-program: not found on PATH".to_owned())
  );
}

#[test]
fn a_missing_directory_is_a_spawn_error_naming_the_program() {
  let dir = tempfile::tempdir().unwrap();
  let missing = dir.path().join("gone");
  let env = Env::from_pairs([("PATH", "/usr/bin:/bin")]);
  let err = exec(&argv(&["true"]), Some(Path::new(&missing)), Some(&env));
  let RunError::Spawn(message) = err else {
    panic!("expected a spawn error, got {err:?}");
  };
  assert!(message.starts_with("true: "), "{message}");
}

#[test]
fn resolve_searches_path_in_order_and_skips_what_cannot_run() {
  let dir = tempfile::tempdir().unwrap();
  let (first, second) = (dir.path().join("a"), dir.path().join("b"));
  std::fs::create_dir_all(&first).unwrap();
  std::fs::create_dir_all(&second).unwrap();
  std::fs::write(first.join("tool"), "data").unwrap();
  let script = second.join("tool");
  std::fs::write(&script, "#!/bin/sh\n").unwrap();
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = format!("{}:{}", first.display(), second.display());
  assert_eq!(resolve("tool", &path), Some(script));
  assert_eq!(resolve("absent", &path), None);
  assert_eq!(resolve("./x/y", ""), Some("./x/y".into()));
}

#[test]
fn an_unset_path_searches_the_default_one() {
  assert!(resolve("sh", DEFAULT_PATH).is_some());
  assert_eq!(resolve("toolu-no-such-program", DEFAULT_PATH), None);
}
