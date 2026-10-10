use std::path::PathBuf;

use serde_json::Map;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};

use super::{AstGrepScan, EditedFile, QualityFindings, QualityRule, run_quality};

struct Rule;

impl QualityRule for Rule {
  fn language(&self) -> &'static str {
    "fixture"
  }
  fn extensions(&self) -> &[&str] {
    &["fx"]
  }
  fn source(&self) -> &'static str {
    "fixture-quality"
  }
  fn reason(&self) -> &'static str {
    "fixture failed"
  }
  fn project_enabled(&self, _ctx: &RuleContext<'_>) -> bool {
    true
  }
  fn skip_linked_worktrees(&self) -> bool {
    false
  }
  fn ast_rule_dirs(&self) -> Vec<PathBuf> {
    Vec::new()
  }
  fn check(
    &self,
    _file: &EditedFile,
    _ctx: &RuleContext<'_>,
    _scan: &AstGrepScan,
  ) -> QualityFindings {
    QualityFindings {
      errors: vec!["bad".to_owned()],
      advisories: Vec::new(),
    }
  }
}

#[test]
fn quality_run_last_record_wins_and_foreign_paths_are_ignored() {
  let temp = tempfile::tempdir().unwrap();
  std::fs::write(temp.path().join("a.fx"), "x\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: temp.path(),
    project_root: temp.path(),
    cwd: Some(temp.path()),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let records = [
    EditRecord {
      path: "a.fx".to_owned(),
      operation: EditOperation::Write,
      moved_to: None,
      from: None,
    },
    EditRecord {
      path: "a.txt".to_owned(),
      operation: EditOperation::Write,
      moved_to: None,
      from: None,
    },
    EditRecord {
      path: "a.fx".to_owned(),
      operation: EditOperation::Delete,
      moved_to: None,
      from: None,
    },
  ];
  let outcome = run_quality(&records, &ctx, &[&Rule]);
  assert_eq!(outcome.decisions, Vec::new());
  assert_eq!(outcome.warnings, Vec::<String>::new());
  assert!(
    !temp
      .path()
      .join(".claude/tmp/quality-gate-status.json")
      .exists()
  );
}
