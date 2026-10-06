use std::path::PathBuf;

use super::Env;

#[test]
fn an_empty_value_reads_as_unset() {
  let env = Env::from_pairs([("SET", "x"), ("EMPTY", "")]);
  assert_eq!(env.get("SET"), Some("x"));
  assert_eq!(env.get("EMPTY"), None);
  assert_eq!(env.get("ABSENT"), None);
}

#[test]
fn vars_keeps_empty_values_for_a_child() {
  let env = Env::from_pairs([("B", ""), ("A", "1")]);
  let vars: Vec<(&str, &str)> = env.vars().collect();
  assert_eq!(vars, [("A", "1"), ("B", "")]);
}

#[test]
fn with_sets_or_replaces_one_variable() {
  let env = Env::from_pairs([("A", "1")]).with("A", "2").with("B", "3");
  assert_eq!(env.get("A"), Some("2"));
  assert_eq!(env.get("B"), Some("3"));
}

#[test]
fn home_is_home_then_the_os_home_directory() {
  let env = Env::from_pairs([("HOME", "/home/u")]);
  assert_eq!(env.home(), PathBuf::from("/home/u"));
  let unset = Env::from_pairs([("HOME", "")]);
  assert_eq!(unset.home(), std::env::home_dir().unwrap_or_default());
}

#[test]
fn the_process_snapshot_holds_the_real_path() {
  let env = Env::process();
  let path = std::env::var("PATH").ok().filter(|path| !path.is_empty());
  assert_eq!(env.get("PATH").map(str::to_owned), path);
}
