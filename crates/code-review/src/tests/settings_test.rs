use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{Settings, load};
use crate::budget::Effort;

/// `home` and `project` config files under a sandbox, as the loader finds them.
fn settings(user: Option<&str>, project: Option<&str>) -> Settings {
  let dir = tempfile::tempdir().unwrap();
  let (home, root) = (dir.path().join("home"), dir.path().join("project"));
  let place = |path: &Path, text: &str| {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
  };
  user.inspect(|text| place(&home.join(".claude/toolu.config.json"), text));
  project.inspect(|text| place(&root.join(".claude/toolu.config.json"), text));
  let env = Env::from_pairs([
    ("HOME", home.display().to_string()),
    ("TOOLU_PROJECT_DIR", root.display().to_string()),
  ]);
  load(&Roots::new(env, Some(Host::Claude)), None)
}

#[test]
fn enabled_by_default_with_project_precedence() {
  let on = r#"{"codeReview":{"enabled":true}}"#;
  let off = r#"{"codeReview":{"enabled":false}}"#;
  assert!(settings(None, None).enabled);
  assert!(settings(None, Some("{}")).enabled);
  assert!(settings(Some(on), None).enabled);
  assert!(!settings(Some(off), None).enabled);
  assert!(!settings(None, Some(off)).enabled);
  assert!(!settings(Some(on), Some(off)).enabled);
  assert!(settings(Some(off), Some(on)).enabled);
  assert!(!settings(Some(off), Some(r#"{"codeReview":{"effort":"high"}}"#)).enabled);
  assert_eq!(settings(None, None), Settings::default());
}

#[test]
fn invalid_config_and_enablement_fail_closed_with_a_reason() {
  for invalid in [
    "{",
    "null",
    "false",
    "[]",
    r#"{"version":2}"#,
    r#"{"codeReview":null}"#,
    r#"{"codeReview":false}"#,
    r#"{"codeReview":{"enabled":"yes"}}"#,
    r#"{"codeReview":{"enabled":null}}"#,
  ] {
    for found in [settings(None, Some(invalid)), settings(Some(invalid), None)] {
      assert!(!found.enabled, "{invalid}");
      assert!(!found.notes.is_empty(), "{invalid}");
    }
  }
}

#[test]
fn config_paths_fail_closed_except_for_missing_files_and_valid_file_symlinks() {
  let dir = tempfile::tempdir().unwrap();
  let user = dir.path().join("user");
  std::fs::create_dir(&user).unwrap();
  let path = user.join("toolu.config.json");
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    ("TOOLU_USER_CONFIG_DIR", user.display().to_string()),
    (
      "TOOLU_PROJECT_DIR",
      dir.path().join("project").display().to_string(),
    ),
  ]);
  let roots = Roots::new(env, Some(Host::Claude));
  let disabled = || {
    let found = load(&roots, None);
    assert!(!found.enabled);
    assert!(
      !found.notes.is_empty(),
      "config path failure needs a reason"
    );
  };
  assert!(load(&roots, None).enabled);
  std::fs::create_dir(&path).unwrap();
  disabled();
  std::fs::remove_dir(&path).unwrap();
  let target = dir.path().join("target.json");
  std::os::unix::fs::symlink(&target, &path).unwrap();
  disabled();
  std::fs::write(&target, r#"{"codeReview":{"enabled":true}}"#).unwrap();
  assert!(load(&roots, None).enabled);
  std::fs::remove_file(&path).unwrap();
  std::fs::remove_dir(&user).unwrap();
  std::fs::write(&user, "not a directory").unwrap();
  disabled();
}

#[test]
fn an_invalid_envelope_fails_closed() {
  let found = settings(None, Some(r#"{"codeReview":{"enabled":true},"nope":1}"#));
  assert!(!found.enabled);
  assert!(
    found.notes[0].contains("unknown top-level key 'nope'"),
    "{:?}",
    found.notes
  );
}

#[test]
fn effort_comes_from_the_merged_config_and_a_bad_value_is_noted() {
  let user = settings(Some(r#"{"codeReview":{"effort":"small"}}"#), None);
  assert_eq!((user.enabled, user.effort), (true, Some(Effort::Small)));
  let both = settings(
    Some(r#"{"codeReview":{"effort":"small"}}"#),
    Some(r#"{"codeReview":{"effort":"xhigh"}}"#),
  );
  assert_eq!(both.effort, Some(Effort::Xhigh));
  let bad = settings(None, Some(r#"{"codeReview":{"effort":"huge"}}"#));
  assert_eq!(bad.effort, None);
  assert!(
    bad.notes[0].contains("codeReview.effort"),
    "{:?}",
    bad.notes
  );
}
