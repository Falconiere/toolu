use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::{decision::Decision, host::Host, normalized::NormalizedEvent};
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::CodeEditRules;
use crate::gate::Gate;

fn edit(path: &str) -> NormalizedEvent {
  serde_json::from_value(json!({
    "type":"tool/pre", "sessionId":"s", "cwd":"/tmp", "projectRoot":"/tmp",
    "worktree":"/tmp", "toolCallId":"c", "toolName":"Edit",
    "toolInput":{"file_path":path}
  }))
  .expect("normalized edit")
}

fn context<'a>(dir: &'a Path, env: &'a Env, raw: &'a Map<String, Value>) -> RuleContext<'a> {
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
fn first_matching_rule_supplies_docs_and_extra_docs() {
  let dir = tempfile::tempdir().expect("tempdir");
  fs::write(dir.path().join("code-edit-rules.json"), r#"{"rules":[{"match":"*.rs","docs":["a","b"],"when_path_matches":["src/*"],"extra_docs":["c"]},{"match":"*","docs":["later"]}]}"#).expect("rules");
  let path = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("TOOLU_SETTINGS_DIR", path.as_str()),
    ("HOME", path.as_str()),
  ]);
  let raw = Map::<String, Value>::new();
  let ctx = context(dir.path(), &env, &raw);
  let decision = CodeEditRules
    .run(&edit("src/x.rs"), &ctx)
    .expect("decision");
  assert_eq!(
    decision,
    Decision::Advisory {
      message: toolu_protocol::text::Text::new("File: src/x.rs\nApply these rules: a + b + c")
        .expect("text")
    }
  );
}

#[test]
fn malformed_and_partial_json_follow_lenient_field_semantics() {
  let dir = tempfile::tempdir().expect("tempdir");
  let file = dir.path().join("code-edit-rules.json");
  let path = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("TOOLU_SETTINGS_DIR", path.as_str()),
    ("HOME", path.as_str()),
  ]);
  let raw = Map::<String, Value>::new();
  let ctx = context(dir.path(), &env, &raw);
  fs::write(&file, "{broken").expect("malformed");
  assert_eq!(CodeEditRules.run(&edit("x.rs"), &ctx), Ok(Decision::Allow));
  fs::write(&file, r#"{"rules":[{"match":"*.py","note":1},{"match":"*.rs","docs":["rust",5,true,null],"x":true}]}"#).expect("partial");
  let decision = CodeEditRules.run(&edit("x.rs"), &ctx).expect("decision");
  assert_eq!(
    decision,
    Decision::Advisory {
      message: toolu_protocol::text::Text::new("File: x.rs\nApply these rules: rust + 5 + true + ")
        .expect("text")
    }
  );
}
