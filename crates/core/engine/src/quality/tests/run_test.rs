use std::path::PathBuf;

use serde_json::Map;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};

use super::{
  AstGrepScan, EditedFile, QualityFindings, QualityRule, last_edit_records,
  last_edit_records_probed, run_quality,
};

fn record(
  path: &str,
  operation: EditOperation,
  from: Option<&str>,
  moved_to: Option<&str>,
) -> EditRecord {
  EditRecord {
    path: path.to_owned(),
    operation,
    from: from.map(str::to_owned),
    moved_to: moved_to.map(str::to_owned),
  }
}

fn meta(record: &EditRecord) -> (&str, EditOperation, Option<&str>, Option<&str>) {
  (
    record.path.as_str(),
    record.operation,
    record.from.as_deref(),
    record.moved_to.as_deref(),
  )
}

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
  fn check(
    &self,
    _file: &EditedFile,
    _ctx: &RuleContext<'_>,
    scan: &AstGrepScan,
  ) -> QualityFindings {
    let errors = match scan {
      AstGrepScan::Ok { hits, .. } => hits
        .iter()
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
fn last_edit_records_mixed_ops_keep_last_occurrence_order_and_metadata() {
  let records = [
    record("A", EditOperation::Write, None, None),
    record("B", EditOperation::Write, None, None),
    record("A", EditOperation::Delete, None, None),
    record("C", EditOperation::Move, Some("c-src"), None),
    record("B", EditOperation::Update, None, None),
  ];
  let survivors = last_edit_records(&records);
  assert_eq!(
    survivors.iter().map(|row| meta(row)).collect::<Vec<_>>(),
    vec![
      ("A", EditOperation::Delete, None, None),
      ("C", EditOperation::Move, Some("c-src"), None),
      ("B", EditOperation::Update, None, None),
    ]
  );
}

#[test]
fn last_edit_records_empty_distinct_identical_and_path_aliases() {
  assert_eq!(last_edit_records(&[]).len(), 0);

  let distinct = [
    record("a", EditOperation::Write, None, None),
    record("b", EditOperation::Delete, None, None),
    record("c", EditOperation::Move, Some("from-c"), None),
  ];
  assert_eq!(
    last_edit_records(&distinct)
      .iter()
      .map(|row| meta(row))
      .collect::<Vec<_>>(),
    distinct.iter().map(meta).collect::<Vec<_>>()
  );

  let identical = [
    record("x", EditOperation::Write, None, None),
    record("x", EditOperation::Update, None, None),
    record("x", EditOperation::Delete, None, None),
  ];
  let survivors = last_edit_records(&identical);
  assert_eq!(survivors.len(), 1);
  assert_eq!(meta(survivors[0]), ("x", EditOperation::Delete, None, None));

  let aliases = [
    record("./a", EditOperation::Write, None, None),
    record("a", EditOperation::Update, None, None),
  ];
  assert_eq!(
    last_edit_records(&aliases)
      .iter()
      .map(|row| meta(row))
      .collect::<Vec<_>>(),
    vec![
      ("./a", EditOperation::Write, None, None),
      ("a", EditOperation::Update, None, None),
    ]
  );
}

#[test]
fn last_edit_records_probed_scales_with_one_probe_per_record() {
  for n in [100_usize, 1_000, 10_000] {
    let identical: Vec<EditRecord> = (0..n)
      .map(|index| {
        record(
          "same",
          if index + 1 == n {
            EditOperation::Delete
          } else {
            EditOperation::Write
          },
          None,
          None,
        )
      })
      .collect();
    let mut probes = 0_u64;
    let survivors = last_edit_records_probed(&identical, &mut probes);
    assert_eq!(probes, n as u64, "identical n={n}");
    assert_eq!(survivors.len(), 1);
    assert_eq!(
      meta(survivors[0]),
      ("same", EditOperation::Delete, None, None)
    );

    let distinct: Vec<EditRecord> = (0..n)
      .map(|index| record(&format!("p{index}"), EditOperation::Write, None, None))
      .collect();
    probes = 0;
    let survivors = last_edit_records_probed(&distinct, &mut probes);
    assert_eq!(probes, n as u64, "distinct n={n}");
    assert_eq!(survivors.len(), n);
    assert_eq!(survivors[0].path, "p0");
    assert_eq!(survivors[n - 1].path, format!("p{}", n - 1));
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
