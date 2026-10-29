use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::GateFile;

use super::super::tests::Inline;
use super::{AstGrepScan, EditedFile, QualityBatch, QualityBinding, scan_batch_with_fallback};

fn gate_paths(gate: &Path) -> Vec<String> {
  let GateRead::Ok(GateFile::Failing {
    entries: Some(entries),
    ..
  }) = read_gate_file(gate)
  else {
    panic!("missing gate");
  };
  entries.into_iter().map(|(path, _)| path).collect()
}

fn context<'a>(root: &'a Path, env: &'a Env, raw: &'a Map<String, Value>) -> RuleContext<'a> {
  RuleContext {
    host: Host::Claude,
    env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw,
    edit: None,
  }
}

fn record(path: String) -> EditRecord {
  EditRecord {
    path,
    operation: EditOperation::Write,
    from: None,
    moved_to: None,
  }
}

fn binding(inline: &Inline) -> [QualityBinding<'_>; 1] {
  [QualityBinding {
    spec: "x@t",
    name: "throw",
    rule: inline,
  }]
}

#[test]
fn gate_entries_are_written_only_when_each_path_is_reached() {
  let temp = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(temp.path()).unwrap();
  std::fs::write(root.join("a.ts"), "throw \"a\";\n").unwrap();
  std::fs::write(root.join("b.ts"), "export const b = 1;\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(&root, &env, &raw);
  let records = ["./a.ts", "b.ts"].map(|path| record(path.to_owned()));
  let inline = Inline("throw-test", "throw-test");
  let binding = binding(&inline);
  let mut batch = QualityBatch::new(&records);
  let gate = root.join(".claude/tmp/quality-gate-status.json");
  assert!(!gate.exists());
  let (first, warnings) = batch.decide(0, (0, &[0], &binding), &ctx);
  assert_eq!(warnings, Vec::<String>::new());
  assert!(matches!(
    first,
    toolu_protocol::decision::Decision::Advisory { .. }
  ));
  assert_eq!(gate_paths(&gate), ["./a.ts"]);
  std::fs::write(root.join("b.ts"), "throw \"b\";\n").unwrap();
  batch.invalidate();
  let (second, warnings) = batch.decide(1, (0, &[0], &binding), &ctx);
  assert_eq!(warnings, Vec::<String>::new());
  assert!(matches!(
    second,
    toolu_protocol::decision::Decision::Advisory { .. }
  ));
  assert_eq!(gate_paths(&gate), ["./a.ts", "b.ts"]);
}

#[test]
fn a_changed_file_is_rescanned_for_each_reached_path_spelling() {
  let temp = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(temp.path()).unwrap();
  std::fs::write(root.join("first.ts"), "export const first = 1;\n").unwrap();
  std::fs::write(root.join("same.ts"), "export const same = 1;\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(&root, &env, &raw);
  let absolute = root.join("same.ts").to_string_lossy().into_owned();
  let records = [
    "first.ts".to_owned(),
    "same.ts".to_owned(),
    absolute.clone(),
  ]
  .map(record);
  let inline = Inline("throw-test", "throw-test");
  let binding = binding(&inline);
  let mut batch = QualityBatch::new(&records);
  let _ = batch.decide(0, (0, &[0], &binding), &ctx);
  std::fs::write(root.join("same.ts"), "throw \"new\";\n").unwrap();
  batch.invalidate();
  assert!(matches!(
    batch.decide(1, (0, &[0], &binding), &ctx).0,
    toolu_protocol::decision::Decision::Advisory { .. }
  ));
  assert!(matches!(
    batch.decide(2, (0, &[0], &binding), &ctx).0,
    toolu_protocol::decision::Decision::Advisory { .. }
  ));
  let gate = root.join(".claude/tmp/quality-gate-status.json");
  assert_eq!(gate_paths(&gate), ["same.ts", absolute.as_str()]);
}

#[test]
fn combined_scan_overflow_retries_each_file() {
  let temp = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(temp.path()).unwrap();
  let source = "throw \"x\";\n".repeat(4000);
  let files: Vec<EditedFile> = ["a.ts", "b.ts"]
    .into_iter()
    .map(|path| {
      let absolute = root.join(path);
      std::fs::write(&absolute, &source).unwrap();
      EditedFile {
        path: path.to_owned(),
        absolute,
        removed: false,
      }
    })
    .collect();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(&root, &env, &raw);
  let inline = Inline("throw-test", "throw-test");
  let (scan, _, fallback) = scan_batch_with_fallback(&[&inline], &files, &ctx);
  assert!(super::super::super::scan::output_limited(&scan));
  assert_eq!(fallback.len(), 2);
  for file in &files {
    let Some(AstGrepScan::Ok { hits, .. }) = fallback.get(&file.path) else {
      panic!("file scan did not recover: {}", file.path);
    };
    assert_eq!(hits.len(), 4000);
  }
}
