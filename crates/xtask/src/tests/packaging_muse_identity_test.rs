use std::path::Path;

use serde_json::json;

fn write(root: &Path, rel: &str, body: &str) {
  let path = root.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}

fn run(root: &Path) -> Result<(), String> {
  let claude = json!({"name": "demo", "description": "Demo"});
  super::check(root, "plugins/demo", "demo", &claude)
}

#[test]
fn a_matching_manifest_passes() {
  let tmp = tempfile::tempdir().unwrap();
  let body = "{\"name\":\"demo\",\"description\":\"Demo\"}\n";
  write(tmp.path(), "plugins/demo/.muse-plugin/plugin.json", body);
  assert_eq!(run(tmp.path()), Ok(()));
}

#[test]
fn a_missing_manifest_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let err = run(tmp.path()).unwrap_err();
  assert!(err.contains("demo is missing its Muse manifest"), "{err}");
}

#[test]
fn a_changed_description_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let body = "{\"name\":\"demo\",\"description\":\"Other\"}\n";
  write(tmp.path(), "plugins/demo/.muse-plugin/plugin.json", body);
  let err = run(tmp.path()).unwrap_err();
  assert!(err.contains("description differs"), "{err}");
}

#[test]
fn a_changed_name_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let body = "{\"name\":\"other\",\"description\":\"Demo\"}\n";
  write(tmp.path(), "plugins/demo/.muse-plugin/plugin.json", body);
  let err = run(tmp.path()).unwrap_err();
  assert!(err.contains("name differs"), "{err}");
}
