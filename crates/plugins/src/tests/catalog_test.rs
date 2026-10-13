use std::path::Path;

use toolu_protocol::exit::Exit;

use super::{load, parse};

fn repo(path: &str) -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../..")
    .join(path)
}

#[test]
fn the_built_in_catalog_is_the_repository_manifest() {
  let Ok(catalog) = load(None) else {
    panic!("the built-in catalog is invalid");
  };
  let mut names = catalog.names();
  names.sort();
  assert_eq!(
    names,
    [
      "ast-grep",
      "brainstorm",
      "delivery-flow",
      "epic-orchestrator",
      "jev",
      "pr-babysit",
      "python-quality",
      "rust-quality",
      "statusline",
      "toolu",
      "toolu-review",
      "ts-quality"
    ]
  );
  let from_file = load(Some(&repo(".claude-plugin/marketplace.json")));
  assert_eq!(from_file, Ok(catalog.clone()));
  let toolu = catalog
    .entry("toolu")
    .and_then(|entry| entry.description.clone());
  assert!(toolu.is_some_and(|text| !text.is_empty()));
}

#[test]
fn a_missing_file_fails_and_bad_files_are_usage_errors() {
  let Err(missing) = load(Some(&repo("no-such-manifest.json"))) else {
    panic!("a missing manifest loaded");
  };
  assert_eq!(missing.exit, Exit::Failure);
  assert!(
    missing
      .message
      .starts_with("cannot read marketplace manifest: ")
  );
  let Err(package) = load(Some(&repo("package.json"))) else {
    panic!("package.json loaded as a catalog");
  };
  assert_eq!(package.exit, Exit::Usage);
  assert!(
    package.message.contains("is malformed"),
    "{}",
    package.message
  );
  let Err(readme) = load(Some(&repo("README.md"))) else {
    panic!("README.md loaded as a catalog");
  };
  assert_eq!(readme.exit, Exit::Usage);
  assert!(
    readme.message.contains("is not valid JSON"),
    "{}",
    readme.message
  );
}

#[test]
fn each_field_of_a_plugin_is_checked() {
  let cases = [
    (
      r#"{"name":"","plugins":[{"name":"a"}]}"#,
      "\"name\" must be a non-empty string",
    ),
    (
      r#"{"name":"t","plugins":[]}"#,
      "\"plugins\" must be a non-empty array",
    ),
    (
      r#"{"name":"t","plugins":[{"name":""}]}"#,
      "plugins[0]: \"name\"",
    ),
    (
      r#"{"name":"t","plugins":[{"name":"a","description":1}]}"#,
      "\"description\"",
    ),
    (
      r#"{"name":"t","plugins":[{"name":"a","dependencies":{}}]}"#,
      "\"dependencies\"",
    ),
    (
      r#"{"name":"t","plugins":[{"name":"a","dependencies":["b"]}]}"#,
      "a dependency must",
    ),
  ];
  for (text, detail) in cases {
    let Err(fail) = parse(text, "x.json") else {
      panic!("{text} parsed");
    };
    assert_eq!(fail.exit, Exit::Usage);
    assert!(fail.message.contains(detail), "{text}: {}", fail.message);
    assert!(fail.message.contains("x.json"), "{}", fail.message);
  }
}
