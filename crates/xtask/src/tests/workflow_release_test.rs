use serde_json::json;

use super::{draft_access, release};
use crate::workflow_ci::Docs;
use crate::workflow_yaml::Findings;

#[test]
fn an_unreadable_release_config_is_an_error() {
  let root = tempfile::tempdir().unwrap();
  let mut found = Findings::default();
  let err = release(root.path(), &Docs(&[]), &mut found).unwrap_err();
  assert!(
    err.starts_with("release-please-config.json: cannot read or parse"),
    "{err}"
  );
}

#[test]
fn the_smoke_test_never_reads_the_draft_and_the_upload_verifies_it() {
  let artifact =
    json!({"uses": "actions/download-artifact@v4", "with": {"name": "native-release"}});
  let finalize = json!({"jobs": {"smoke-test": {"steps": [artifact]}}});
  let native = json!({"jobs": {"upload": {"steps": [
    {"run": "gh release upload \"$TAG\" dist/x --clobber"},
    {"run": "gh release download \"$TAG\" -D roundtrip\ncmp \"$file\" \"roundtrip/x\""}
  ]}}});
  let mut found = Findings::default();
  draft_access(&native, &finalize, &mut found);
  assert_eq!(found.0, Vec::<String>::new());

  let finalize = json!({"jobs": {"smoke-test": {
    "env": {"GH_TOKEN": "${{ github.token }}"},
    "steps": [{"run": "gh release download \"$TAG\" -p \"$TARBALL\" -D dist"}]}}});
  let native =
    json!({"jobs": {"upload": {"steps": [{"run": "gh release upload \"$TAG\" dist/x"}]}}});
  let mut found = Findings::default();
  draft_access(&native, &finalize, &mut found);
  assert_eq!(
    found.0,
    [
      "release-finalize.yml must smoke-test the run's native-release artifact without a token",
      "release-native.yml upload must verify the draft holds the packaged bytes",
    ]
  );
}

#[test]
fn empty_release_workflows_name_every_broken_link() {
  let root = tempfile::tempdir().unwrap();
  let config = json!({"packages": {".": {"draft": true, "force-tag-creation": true}}});
  std::fs::write(
    root.path().join("release-please-config.json"),
    config.to_string(),
  )
  .unwrap();
  let mut found = Findings::default();
  release(root.path(), &Docs(&[]), &mut found).unwrap();
  for line in [
    "release-please.yml must chain native release",
    "release-native.yml lacks x86_64-apple-darwin on macos-15-intel",
    "release-native.yml dry run must skip release writes",
    "release-finalize.yml must check four archives",
    "npm-publish.yml prereleases must leave the latest dist-tag untouched",
  ] {
    assert!(found.0.contains(&line.to_owned()), "{line}: {:?}", found.0);
  }
  assert!(
    !found
      .0
      .contains(&"release-please must create a draft and a real tag".to_owned())
  );
}
