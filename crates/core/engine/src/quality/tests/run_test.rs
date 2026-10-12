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

struct Inline;

impl QualityRule for Inline {
  fn language(&self) -> &'static str {
    "ts"
  }
  fn extensions(&self) -> &[&str] {
    &["ts"]
  }
  fn source(&self) -> &'static str {
    "inline-quality"
  }
  fn reason(&self) -> &'static str {
    "inline failed"
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
  fn ast_rules_inline(&self, _ctx: &RuleContext<'_>) -> String {
    "id: throw-string\nlanguage: ts\nseverity: warning\nmessage: x\nrule:\n  pattern: 'throw \"$S\"'"
      .to_owned()
  }
  fn check(&self, _file: &EditedFile, _ctx: &RuleContext<'_>, scan: &AstGrepScan) -> QualityFindings {
    let errors = match scan {
      AstGrepScan::Ok { hits, .. } => hits
        .iter()
        .map(|hit| format!("{} {}", hit.rule_id, hit.excerpt))
        .collect(),
      other => vec![format!("{other:?}")],
    };
    QualityFindings {
      errors,
      advisories: Vec::new(),
    }
  }
}

#[test]
fn inline_rules_join_the_one_scan() {
  let temp = tempfile::tempdir().unwrap();
  std::fs::write(temp.path().join("a.ts"), "throw \"boom\";\n").unwrap();
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
  let records = [EditRecord {
    path: "a.ts".to_owned(),
    operation: EditOperation::Write,
    moved_to: None,
    from: None,
  }];
  let outcome = run_quality(&records, &ctx, &[&Inline, &Rule]);
  let [toolu_protocol::decision::Decision::Advisory { message }] = outcome.decisions.as_slice()
  else {
    panic!("one advisory expected: {:?}", outcome.decisions);
  };
  assert_eq!(
    message.as_str(),
    "QUALITY VIOLATION — fix before proceeding:\nthrow-string a.ts:1:throw \"boom\";\n"
  );
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
