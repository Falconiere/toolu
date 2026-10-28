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
fn only_the_project_file_can_opt_in() {
  let on = r#"{"codeReview":{"enabled":true}}"#;
  assert!(settings(None, Some(on)).enabled);
  assert!(!settings(Some(on), None).enabled);
  assert!(!settings(Some(on), Some(r#"{"codeReview":{"enabled":false}}"#)).enabled);
  assert!(!settings(None, Some(r#"{"codeReview":{"enabled":"yes"}}"#)).enabled);
  assert_eq!(settings(None, None), Settings::default());
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
  assert_eq!((user.enabled, user.effort), (false, Some(Effort::Small)));
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
