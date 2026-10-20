use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const MANIFEST: &str = "plugins/toolu/.muse-plugin/plugin.json";
const AGENTS: [&str; 5] = [
  "architect",
  "deep-explore",
  "implementer",
  "quick-task",
  "research-agent",
];

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A temp root holding copies of the real toolu agents, Muse manifest,
/// `package.json` and `release-please-config.json`.
fn copy_of_repo() -> tempfile::TempDir {
  let tmp = tempfile::tempdir().unwrap();
  let mut files = vec![
    MANIFEST.to_owned(),
    "package.json".to_owned(),
    "release-please-config.json".to_owned(),
  ];
  files.extend(AGENTS.map(|agent| format!("plugins/toolu/agents/{agent}.md")));
  for rel in files {
    let body = std::fs::read_to_string(repo().join(&rel)).unwrap();
    write(tmp.path(), &rel, &body);
  }
  tmp
}

fn check(root: &Path) -> Result<usize, String> {
  let version = read(root, "package.json")["version"]
    .as_str()
    .unwrap()
    .to_owned();
  let release = read(root, "release-please-config.json");
  super::check_muse(root, &version, &release)
}

fn edit_manifest(root: &Path, change: impl FnOnce(&mut Value)) {
  let mut manifest = read(root, MANIFEST);
  change(&mut manifest);
  write(
    root,
    MANIFEST,
    &serde_json::to_string_pretty(&manifest).unwrap(),
  );
}

fn commands(manifest: &mut Value) -> &mut Vec<Value> {
  manifest["capabilities"]["commands"].as_array_mut().unwrap()
}

fn assert_err(root: &Path, needle: &str) {
  let err = check(root).unwrap_err();
  assert!(err.contains(needle), "{err}");
}

#[test]
fn the_real_toolu_bundle_maps_every_agent() {
  let tmp = copy_of_repo();
  assert_eq!(check(tmp.path()), Ok(5));
}

#[test]
fn this_repository_maps_every_agent() {
  assert_eq!(check(&repo()), Ok(5));
}

#[test]
fn an_edited_agent_body_needs_no_muse_update() {
  let tmp = copy_of_repo();
  let path = tmp.path().join("plugins/toolu/agents/implementer.md");
  let body = std::fs::read_to_string(&path).unwrap();
  std::fs::write(&path, format!("{body}\nOne more rule.\n")).unwrap();
  assert_eq!(check(tmp.path()), Ok(5));
}

#[test]
fn a_renamed_agent_is_drift() {
  let tmp = copy_of_repo();
  let dir = tmp.path().join("plugins/toolu/agents");
  std::fs::rename(dir.join("implementer.md"), dir.join("builder.md")).unwrap();
  assert_err(
    tmp.path(),
    "command implementer points at agents/implementer.md, which is not an agent source",
  );
  edit_manifest(tmp.path(), |manifest| {
    commands(manifest).retain(|entry| entry["id"] != "implementer");
  });
  assert_err(
    tmp.path(),
    "agents/builder.md has no Muse command {\"id\":\"builder\",\"path\":\"agents/builder.md\"}",
  );
}

#[test]
fn a_removed_entry_is_drift() {
  let tmp = copy_of_repo();
  edit_manifest(tmp.path(), |manifest| {
    commands(manifest).retain(|entry| entry["id"] != "implementer");
  });
  assert_err(tmp.path(), "agents/implementer.md has no Muse command");
}

#[test]
fn a_stale_entry_is_drift() {
  let tmp = copy_of_repo();
  edit_manifest(tmp.path(), |manifest| {
    commands(manifest).push(json!({"id": "ghost", "path": "agents/ghost.md"}));
  });
  assert_err(
    tmp.path(),
    "command ghost points at agents/ghost.md, which is not an agent source",
  );
}

#[test]
fn an_entry_named_apart_from_its_agent_is_drift() {
  let tmp = copy_of_repo();
  edit_manifest(tmp.path(), |manifest| {
    for entry in commands(manifest) {
      if entry["id"] == "architect" {
        entry["id"] = json!("design");
      }
    }
  });
  assert_err(
    tmp.path(),
    "command design must be named architect after agents/architect.md",
  );
}

#[test]
fn an_agents_capability_is_refused() {
  let tmp = copy_of_repo();
  edit_manifest(tmp.path(), |manifest| {
    manifest["capabilities"]["agents"] = json!([]);
  });
  assert_err(tmp.path(), "Muse rejects the agents capability");
}

#[test]
fn unknown_top_level_keys_are_refused() {
  for key in ["agents", "author"] {
    let tmp = copy_of_repo();
    edit_manifest(tmp.path(), |manifest| manifest[key] = json!([]));
    assert_err(
      tmp.path(),
      &format!("top-level {key} is not a Muse manifest field"),
    );
  }
}

#[test]
fn agents_without_a_manifest_are_refused() {
  let tmp = copy_of_repo();
  std::fs::remove_file(tmp.path().join(MANIFEST)).unwrap();
  assert_err(
    tmp.path(),
    "plugins/toolu has agents/ but no .muse-plugin/plugin.json",
  );
}

#[test]
fn a_manifest_version_apart_from_package_json_is_refused() {
  let tmp = copy_of_repo();
  edit_manifest(tmp.path(), |manifest| manifest["version"] = json!("0.0.1"));
  assert_err(tmp.path(), "version differs from package.json");
}

#[test]
fn an_untracked_manifest_is_refused() {
  let tmp = copy_of_repo();
  let mut release = read(tmp.path(), "release-please-config.json");
  release["packages"]["."]["extra-files"]
    .as_array_mut()
    .unwrap()
    .retain(|entry| entry["path"] != MANIFEST);
  write(
    tmp.path(),
    "release-please-config.json",
    &release.to_string(),
  );
  assert_err(tmp.path(), &format!("release-please is missing {MANIFEST}"));
}

#[test]
fn invalid_json_and_non_array_commands_are_refused() {
  let tmp = copy_of_repo();
  write(tmp.path(), MANIFEST, "{");
  assert_err(tmp.path(), &format!("invalid JSON: {MANIFEST}"));
  let version = read(&repo(), "package.json")["version"].clone();
  let manifest = json!({"version": version, "capabilities": {"commands": {}}});
  write(tmp.path(), MANIFEST, &manifest.to_string());
  assert_err(tmp.path(), "capabilities.commands is not an array");
}

#[test]
fn plugins_without_agents_and_commands_outside_agents_pass() {
  let tmp = copy_of_repo();
  write(tmp.path(), "plugins/jev/README.md", "# jev\n");
  write(tmp.path(), "plugins/statusline/agents/.gitkeep", "");
  edit_manifest(tmp.path(), |manifest| {
    commands(manifest).push(json!({"id": "doctor", "path": "commands/doctor.md"}));
  });
  assert_eq!(check(tmp.path()), Ok(5));
}

fn read(root: &Path, rel: &str) -> Value {
  serde_json::from_str(&std::fs::read_to_string(root.join(rel)).unwrap()).unwrap()
}

fn write(root: &Path, rel: &str, body: &str) {
  let path = root.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}
