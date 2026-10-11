#[test]
fn an_untracked_manifest_is_not_in_release_please() {
  let release = serde_json::json!({"packages": {".": {"extra-files": []}}});
  assert!(!super::release_tracks(
    &release,
    "plugins/demo/.claude-plugin/plugin.json",
    "json",
    "$.version"
  ));
}
