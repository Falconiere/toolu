use std::path::{Path, PathBuf};

use crate::Verdict;
use crate::options::Options;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn options(root: PathBuf) -> Options {
  Options {
    root,
    ..Options::default()
  }
}

#[test]
fn this_repository_is_packaged() {
  assert_eq!(super::run(&options(repo())).unwrap(), Verdict::Clean);
}

#[test]
fn a_mismatched_manifest_version_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  let manifest = "{\"name\":\"demo\",\"version\":\"9.9.9\",\"description\":\"Demo\"}\n";
  write(root, "package.json", "{\"version\":\"1.0.0\"}\n");
  write(
    root,
    ".claude-plugin/marketplace.json",
    "{\"plugins\":[]}\n",
  );
  write(
    root,
    ".agents/plugins/marketplace.json",
    "{\"plugins\":[]}\n",
  );
  write(
    root,
    "release-please-config.json",
    "{\"packages\":{\".\":{\"extra-files\":[]}}}\n",
  );
  write(root, "plugins/demo/.claude-plugin/plugin.json", manifest);
  write(root, "plugins/demo/.codex-plugin/plugin.json", manifest);
  let err = super::package(root).unwrap_err();
  assert!(
    err.contains("demo Claude manifest version differs from package.json"),
    "{err}"
  );
}

fn write(root: &Path, rel: &str, body: &str) {
  let path = root.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}
