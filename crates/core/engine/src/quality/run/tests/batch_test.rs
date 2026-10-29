use std::path::Path;

use serde_json::Map;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::GateFile;

use super::super::tests::Inline;
use super::{QualityBatch, QualityBinding};

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

#[test]
fn gate_entries_are_written_only_when_each_path_is_reached() {
  let temp = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(temp.path()).unwrap();
  std::fs::write(root.join("a.ts"), "throw \"a\";\n").unwrap();
  std::fs::write(root.join("b.ts"), "export const b = 1;\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: &root,
    project_root: &root,
    cwd: Some(&root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let records = ["./a.ts", "b.ts"].map(|path| EditRecord {
    path: path.to_owned(),
    operation: EditOperation::Write,
    from: None,
    moved_to: None,
  });
  let inline = Inline("throw-test", "throw-test");
  let binding = [QualityBinding {
    spec: "x@t",
    name: "throw",
    rule: &inline,
  }];
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
