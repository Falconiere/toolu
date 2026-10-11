use super::{ChangedFile, classify, load_repo};
use std::path::PathBuf;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn docs_globs_match_a_markdown_file_and_not_the_rust_group() {
  let config = load_repo(&repo()).unwrap();
  let class = classify(
    &config,
    &[ChangedFile {
      path: "docs/install.md".to_owned(),
      lines: Vec::new(),
    }],
  );
  let on = |name: &str| {
    class
      .outputs
      .iter()
      .find(|(key, _)| key == name)
      .map(|(_, on)| *on)
  };
  assert_eq!(on("docs"), Some(true));
  assert_eq!(on("rust"), Some(false));
  assert_eq!(on("changed"), Some(true));
}

#[test]
fn a_changelog_edit_is_release_only() {
  let config = load_repo(&repo()).unwrap();
  let class = classify(
    &config,
    &[ChangedFile {
      path: "CHANGELOG.md".to_owned(),
      lines: vec!["+anything".to_owned()],
    }],
  );
  assert!(
    class.outputs.iter().all(|(_, on)| !on),
    "{:?}",
    class.reasons
  );
  assert!(class.reasons[0].contains("release-only"));
}
