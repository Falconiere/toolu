use std::path::Path;

use super::{DATA_DIR, Folders, limits, limits_at, load, rules};

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn write(root: &Path, rel: &str, text: &str) {
  let path = root.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

#[test]
fn the_repository_data_loads() {
  let repo = Path::new(REPO);
  let rules = rules(repo).unwrap();
  assert_eq!(rules.coverage.default, 85.0);
  assert_eq!(rules.inventory.kinds[0].kind, "xtask-task");
  let limits = limits(repo).unwrap();
  assert_eq!(
    (limits.file, limits.function, limits.impl_block),
    (300, 50, 200)
  );
  let folders: Folders = load(repo, "folders.json").unwrap();
  assert!(folders.crates.contains(&"xtask".to_owned()));
}

#[test]
fn an_unknown_key_anywhere_is_rejected() {
  let dir = tempfile::tempdir().unwrap();
  write(
    dir.path(),
    &format!("{DATA_DIR}/folders.json"),
    "{\"root\":[],\"crates\":[],\"core\":[],\"ignore\":[\"crates/x\"]}",
  );
  let err = load::<Folders>(dir.path(), "folders.json").unwrap_err();
  assert!(err.contains("unknown field `ignore`"), "{err}");
  let text = std::fs::read_to_string(Path::new(REPO).join(DATA_DIR).join("rules.json")).unwrap();
  let nested = text.replacen("\"maxSrcDepth\"", "\"exempt\": [],\n    \"maxSrcDepth\"", 1);
  write(dir.path(), &format!("{DATA_DIR}/rules.json"), &nested);
  assert!(
    rules(dir.path())
      .unwrap_err()
      .contains("unknown field `exempt`")
  );
}

#[test]
fn a_wrong_version_missing_files_and_missing_limits_fail_closed() {
  let dir = tempfile::tempdir().unwrap();
  let text = std::fs::read_to_string(Path::new(REPO).join(DATA_DIR).join("rules.json")).unwrap();
  write(
    dir.path(),
    &format!("{DATA_DIR}/rules.json"),
    &text.replacen("\"version\": 1", "\"version\": 2", 1),
  );
  assert!(
    rules(dir.path())
      .unwrap_err()
      .ends_with("unsupported version 2")
  );
  assert!(
    load::<Folders>(dir.path(), "folders.json")
      .unwrap_err()
      .starts_with("cannot read")
  );
  assert!(
    limits(dir.path())
      .unwrap_err()
      .starts_with("cannot read .claude/toolu.config.json")
  );
  write(dir.path(), "a.json", "{\"lang\":{}}");
  assert!(
    limits_at(dir.path(), Path::new("a.json"))
      .unwrap_err()
      .ends_with("has no lang.rust limits")
  );
  write(dir.path(), "b.json", "{");
  assert!(
    limits_at(dir.path(), Path::new("b.json"))
      .unwrap_err()
      .starts_with("b.json:")
  );
  write(
    dir.path(),
    "c.json",
    "{\"lang\":{\"rust\":{\"maxFileLines\":\"x\"}}}",
  );
  assert!(
    limits_at(dir.path(), Path::new("c.json"))
      .unwrap_err()
      .starts_with("c.json lang.rust:")
  );
}
