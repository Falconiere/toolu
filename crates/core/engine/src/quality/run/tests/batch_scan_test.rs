use serde_json::Map;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::super::tests::Inline;
use super::{AstGrepScan, EditedFile, retry_file};

#[test]
fn retry_file_preserves_real_rule_hits() {
  let temp = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(temp.path()).unwrap();
  let absolute = root.join("a.ts");
  std::fs::write(&absolute, "throw \"x\";\n").unwrap();
  let file = EditedFile {
    path: "a.ts".to_owned(),
    absolute,
    removed: false,
  };
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
  let inline = Inline("throw-test", "throw-test");
  let AstGrepScan::Ok { hits, .. } = retry_file(&file, &[&inline], &ctx) else {
    panic!("real scan failed");
  };
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0].rule_id, "throw-test");
}
