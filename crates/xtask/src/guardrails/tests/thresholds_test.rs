use super::check;
use crate::guardrails::tests::{context, tree};

const LANG: &str =
  "{\"lang\":{\"rust\":{\"maxFileLines\":300,\"maxFnLines\":50,\"maxImplLines\":200}}}";

#[test]
fn agreeing_numbers_pass() {
  let fine = tree(&[
    ("clippy.toml", "too-many-lines-threshold = 50\n"),
    (".codex/toolu.config.json", LANG),
  ]);
  assert_eq!(check(&context(&fine.workspace)).unwrap(), Vec::new());
  let no_copy = tree(&[("clippy.toml", "too-many-lines-threshold = 50\n")]);
  assert_eq!(check(&context(&no_copy.workspace)).unwrap(), Vec::new());
}

#[test]
fn a_number_changed_alone_fails() {
  let raised = tree(&[
    ("clippy.toml", "too-many-lines-threshold = 60\n"),
    (".codex/toolu.config.json", &LANG.replace("300", "400")),
  ]);
  let found = check(&context(&raised.workspace)).unwrap();
  let paths: Vec<&str> = found.iter().map(|f| f.path.as_str()).collect();
  assert_eq!(paths, ["clippy.toml", ".codex/toolu.config.json"]);
  assert!(found[0].message.starts_with(
    "too-many-lines-threshold is Some(60), .claude/toolu.config.json maxFnLines is 50"
  ));
}

#[test]
fn unreadable_configs_are_setup_errors() {
  let missing = tree(&[]);
  assert!(
    check(&context(&missing.workspace))
      .unwrap_err()
      .starts_with("cannot read clippy.toml")
  );
  let invalid = tree(&[("clippy.toml", "= 1\n")]);
  assert!(
    check(&context(&invalid.workspace))
      .unwrap_err()
      .starts_with("clippy.toml:")
  );
  let bad_copy = tree(&[
    ("clippy.toml", "too-many-lines-threshold = 50\n"),
    (".codex/toolu.config.json", "{}"),
  ]);
  assert!(
    check(&context(&bad_copy.workspace))
      .unwrap_err()
      .contains("has no lang.rust limits")
  );
}
