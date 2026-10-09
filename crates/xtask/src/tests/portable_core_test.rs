use crate::Verdict;
use crate::task_options::{test_options, test_root};

#[test]
fn the_committed_doc_passes() {
  let root = test_root();
  let doc = root.join("docs/portable-core.md");
  assert_eq!(super::problems(&root, &doc).unwrap(), None);
  assert_eq!(super::run(&test_options(root)).unwrap(), Verdict::Clean);
}

#[test]
fn a_missing_heading_is_a_finding() {
  let root = test_root();
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
