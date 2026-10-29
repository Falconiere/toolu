use serde_json::Map;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::{AstGrepHit, AstGrepScan, partition_hits, scan_inline};
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

#[test]
fn partition_keeps_exact_relative_and_absolute_paths_in_hit_order() {
  let hit = |file: &str, line| AstGrepHit {
    rule_id: "r".to_owned(),
    file: file.to_owned(),
    line,
    excerpt: format!("{file}:{line}:hit"),
    text: "hit".to_owned(),
    first: true,
  };
  let mut scan = AstGrepScan::Ok {
    hits: vec![
      hit("src/a.rs", 1),
      hit("/repo/src/a.rs", 2),
      hit("src/b/a.rs", 3),
      hit("src/a.rs", 4),
    ],
    empty: false,
  };
  let groups = partition_hits(&mut scan);
  assert_eq!(
    groups["src/a.rs"]
      .iter()
      .map(|hit| hit.line)
      .collect::<Vec<_>>(),
    [1, 4]
  );
  assert_eq!(groups["/repo/src/a.rs"][0].line, 2);
  assert_eq!(groups["src/b/a.rs"][0].line, 3);
  assert!(!groups.contains_key("a.rs"));
  assert_eq!(
    scan,
    AstGrepScan::Ok {
      hits: Vec::new(),
      empty: false
    }
  );
}
