use std::path::Path;

use super::{LayerTable, Role, forbidden_edge, may_build_binary};

const TABLE: &str = r#"{"core": [["protocol"], ["runtime"], ["shell", "http"]], "rules": ["ts-quality"], "hub": "toolu", "binary": "cli", "tooling": "xtask"}"#;

#[test]
fn roles_follow_the_directory() {
  let table = LayerTable::parse(TABLE).unwrap();
  assert_eq!(
    table.role(Path::new("crates/core/http")).unwrap(),
    Role::Core(2)
  );
  assert_eq!(
    table.role(Path::new("crates/ts-quality")).unwrap(),
    Role::Rule
  );
  assert_eq!(table.role(Path::new("crates/toolu")).unwrap(), Role::Hub);
  assert_eq!(table.role(Path::new("crates/cli")).unwrap(), Role::Cli);
  assert_eq!(
    table.role(Path::new("crates/xtask")).unwrap(),
    Role::Tooling
  );
  assert_eq!(table.role(Path::new("crates/jev")).unwrap(), Role::Plugin);
  assert!(
    table
      .role(Path::new("crates/core/extra"))
      .unwrap_err()
      .contains("no layer")
  );
  assert!(
    table
      .role(Path::new("tools/x"))
      .unwrap_err()
      .contains("is not a crates/<name>")
  );
  assert!(
    LayerTable::parse("{}")
      .unwrap_err()
      .starts_with("invalid layer table")
  );
}

#[test]
fn edges_and_binaries_follow_the_layer_rules() {
  assert_eq!(forbidden_edge(Role::Core(2), Role::Core(1)), None);
  assert!(forbidden_edge(Role::Core(1), Role::Core(1)).is_some());
  assert_eq!(forbidden_edge(Role::Hub, Role::Rule), None);
  assert!(forbidden_edge(Role::Plugin, Role::Plugin).is_some());
  assert!(forbidden_edge(Role::Plugin, Role::Tooling).is_some());
  assert!(forbidden_edge(Role::Cli, Role::Rule).is_some());
  assert_eq!(forbidden_edge(Role::Cli, Role::Hub), None);
  assert!(may_build_binary(Role::Cli) && may_build_binary(Role::Tooling));
  assert!(!may_build_binary(Role::Plugin));
  let shown: Vec<String> = [
    Role::Core(3),
    Role::Rule,
    Role::Hub,
    Role::Plugin,
    Role::Cli,
    Role::Tooling,
  ]
  .iter()
  .map(ToString::to_string)
  .collect();
  assert_eq!(
    shown,
    [
      "core layer 3",
      "rule crate",
      "hub plugin crate",
      "plugin crate",
      "cli crate",
      "tooling crate"
    ]
  );
}
