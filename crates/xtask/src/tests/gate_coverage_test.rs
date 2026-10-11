use super::Row;
use crate::Verdict;
use crate::task_options::{test_options, test_root};

fn row(id: &str) -> Row {
  Row {
    id: id.to_owned(),
    source_path: "plugins/demo/hooks/hooks.json".to_owned(),
    plugin: "demo".to_owned(),
    kind: "hooks.json".to_owned(),
    event: "SessionStart".to_owned(),
    matcher: String::new(),
    command_or_module: "hooks/dist/demo.js".to_owned(),
    host_mechanism: "bun-bundle".to_owned(),
    parent_id: None,
    semantics: String::new(),
    classification: "port-native".to_owned(),
    support: "required".to_owned(),
    implementation_issue: Some(1),
    implementation_status: "done".to_owned(),
    limits: String::new(),
    bash_required: false,
  }
}

#[test]
fn this_repository_matches_inventory() {
  assert_eq!(
    super::run(&test_options(test_root())).unwrap(),
    Verdict::Clean
  );
}

#[test]
fn an_orphan_inventory_id_is_a_finding() {
  let errors = super::id_gaps(&[], &[row("gone")]);
  assert!(
    errors
      .iter()
      .any(|line| line.contains("orphan inventory id: gone")),
    "{errors:?}"
  );
}
