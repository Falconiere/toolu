use serde_json::Map;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::{AstGrepScan, scan_inline};
use crate::quality::EditedFile;

#[test]
fn quality_scan_real_ast_grep_returns_a_tagged_excerpt() {
  let temp = tempfile::tempdir().unwrap();
  let path = temp.path().join("a.ts");
  std::fs::write(&path, "throw \"boom\";\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = RuleContext {
    host: toolu_protocol::host::Host::Claude,
    env: &env,
    config_root: temp.path(),
    project_root: temp.path(),
    cwd: Some(temp.path()),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let file = EditedFile {
    path: "a.ts".to_owned(),
    absolute: path,
    removed: false,
  };
  let rules = "id: throw-string\nlanguage: ts\nseverity: warning\nmessage: x\nrule:\n  pattern: 'throw \"$S\"'";
  let result = scan_inline(&[file], rules, &ctx);
  let AstGrepScan::Ok { hits, empty: false } = result else {
    panic!("real ast-grep output was not parsed: {result:?}");
  };
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0].rule_id, "throw-string");
  assert_eq!(hits[0].excerpt, "a.ts:1:throw \"boom\";");
}
