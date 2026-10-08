use std::path::PathBuf;

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
fn the_committed_doc_passes() {
  let root = repo();
  let doc = root.join("docs/portable-core.md");
  assert_eq!(super::problems(&root, &doc).unwrap(), None);
  assert_eq!(super::run(&options(root)).unwrap(), Verdict::Clean);
}

#[test]
fn a_missing_heading_is_a_finding() {
  let root = repo();
  let doc = std::fs::read_to_string(root.join("docs/portable-core.md")).unwrap();
  let kept = doc
    .lines()
    .filter(|line| *line != "## Pins")
    .collect::<Vec<_>>()
    .join("\n");
  let tmp = tempfile::tempdir().unwrap();
  let path = tmp.path().join("portable-core.md");
  std::fs::write(&path, kept).unwrap();
  assert_eq!(
    super::problems(&root, &path).unwrap().as_deref(),
    Some("missing heading: ## Pins")
  );
}
