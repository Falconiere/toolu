use std::os::unix::fs::symlink;
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

struct Inline(&'static str, &'static str);

impl QualityRule for Inline {
  fn language(&self) -> &'static str {
    "ts"
  }
  fn extensions(&self) -> &[&str] {
    &["ts"]
  }
  fn source(&self) -> &'static str {
    self.1
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
    format!(
      "id: {}\nlanguage: ts\nseverity: warning\nmessage: x\nrule:\n  pattern: 'throw \"$S\"'",
      self.0
    )
  }
  fn check(
    &self,
    _file: &EditedFile,
    _ctx: &RuleContext<'_>,
    scan: &AstGrepScan,
  ) -> QualityFindings {
    let errors = match scan {
      AstGrepScan::Ok { hits, .. } => hits
        .iter()
        .filter(|hit| hit.rule_id == self.0)
        .map(|hit| format!("{} {}", hit.rule_id, hit.excerpt))
        .collect(),
      other @ (AstGrepScan::Missing | AstGrepScan::Failed(_)) => vec![format!("{other:?}")],
    };
    QualityFindings {
      errors,
      advisories: Vec::new(),
    }
  }
}

/// A Claude Code context rooted at `dir`.
pub(super) fn context<'a>(
  dir: &'a std::path::Path,
  env: &'a Env,
  raw: &'a Map<String, serde_json::Value>,
) -> RuleContext<'a> {
  RuleContext {
    host: Host::Claude,
    env,
    config_root: dir,
    project_root: dir,
    cwd: Some(dir),
    plugin_root: None,
    raw,
    edit: None,
  }
}

fn write(path: &str) -> EditRecord {
  EditRecord {
    path: path.to_owned(),
    operation: EditOperation::Write,
    moved_to: None,
    from: None,
  }
}

#[test]
fn inline_rules_join_the_one_scan() {
  let temp = tempfile::tempdir().unwrap();
  std::fs::write(temp.path().join("a.ts"), "throw \"boom\";\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(temp.path(), &env, &raw);
  let records = [EditRecord {
    path: "a.ts".to_owned(),
    operation: EditOperation::Write,
    moved_to: None,
    from: None,
  }];
  let outcome = run_quality(
    &records,
    &ctx,
    &[&Inline("throw-string", "inline-quality"), &Rule],
  );
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
fn batch_keeps_exact_paths_order_and_each_rules_hits() {
  let temp = tempfile::tempdir().unwrap();
  std::fs::create_dir(temp.path().join("nested")).unwrap();
  let body = "throw \"first\";\nthrow \"last\";\n";
  std::fs::write(temp.path().join("a.ts"), body).unwrap();
  std::fs::write(temp.path().join("nested/a.ts"), "throw \"nested\";\n").unwrap();
  std::fs::write(temp.path().join("ba.ts"), "const ok = 1;\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(temp.path(), &env, &raw);
  let records: Vec<_> = ["a.ts", "nested/a.ts", "ba.ts"]
    .into_iter()
    .map(write)
    .collect();
  let outcome = run_quality(
    &records,
    &ctx,
    &[
      &Inline("throw-first", "first-quality"),
      &Inline("throw-second", "second-quality"),
    ],
  );
  let messages: Vec<_> = outcome
    .decisions
    .iter()
    .filter_map(|decision| {
      if let toolu_protocol::decision::Decision::Advisory { message } = decision {
        Some(message.as_str())
      } else {
        None
      }
    })
    .collect();
  assert_eq!(outcome.decisions.len(), 6);
  assert_eq!(messages.len(), 4);
  assert!(
    messages[0].contains("throw-first a.ts:1:throw \"first\";\nthrow-first a.ts:2:throw \"last\";")
  );
  assert!(
    messages[1]
      .contains("throw-second a.ts:1:throw \"first\";\nthrow-second a.ts:2:throw \"last\";")
  );
  assert!(messages[2].contains("throw-first nested/a.ts:1:throw \"nested\";"));
  assert!(messages[3].contains("throw-second nested/a.ts:1:throw \"nested\";"));
  assert!(
    outcome.decisions[4..]
      .iter()
      .all(|decision| matches!(decision, toolu_protocol::decision::Decision::Allow))
  );
}

#[test]
fn failed_batch_scan_reaches_each_rule_and_keeps_gate_failure() {
  let temp = tempfile::tempdir().unwrap();
  std::fs::write(temp.path().join("a.ts"), "throw \"boom\";\n").unwrap();
  let bin = temp.path().join("bin");
  std::fs::create_dir(&bin).unwrap();
  let tool = bin.join("ast-grep");
  let process = Env::process();
  let path = format!("{}:{}", bin.display(), process.get("PATH").unwrap());
  let env = process.with("PATH", &path);
  let raw = Map::new();
  let ctx = context(temp.path(), &env, &raw);
  let records = [write("a.ts")];
  for (target, stage) in [
    ("/usr/bin/false", "stage: AstGrep"),
    ("/bin/echo", "stage: Parse"),
  ] {
    symlink(target, &tool).unwrap();
    let outcome = run_quality(
      &records,
      &ctx,
      &[
        &Inline("throw-first", "first-quality"),
        &Inline("throw-second", "second-quality"),
      ],
    );
    assert_eq!(outcome.decisions.len(), 2);
    for decision in &outcome.decisions {
      let toolu_protocol::decision::Decision::Advisory { message } = decision else {
        panic!("expected failure advisory: {decision:?}");
      };
      assert!(message.as_str().contains(stage), "{message:?}");
    }
    assert!(
      temp
        .path()
        .join(".claude/tmp/quality-gate-status.json")
        .exists()
    );
    std::fs::remove_file(&tool).unwrap();
  }
}

#[test]
fn quality_run_last_record_wins_and_foreign_paths_are_ignored() {
  let temp = tempfile::tempdir().unwrap();
  std::fs::write(temp.path().join("a.fx"), "x\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(temp.path(), &env, &raw);
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
