use std::fs;
use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::McpBlocker;
use crate::gate::Gate;

fn tool(name: &str) -> NormalizedEvent {
  let t = |value| Text::new(value).expect("nonempty");
  let session = Session {
    session_id: t("s"),
    cwd: t("/tmp"),
    project_root: t("/tmp"),
    worktree: t("/tmp"),
  };
  NormalizedEvent::ToolPre {
    session,
    tool: Tool {
      call_id: t("c"),
      name: t(name),
      input: Map::new(),
    },
  }
}

fn context<'a>(dir: &'a Path, env: &'a Env, raw: &'a Map<String, Value>) -> RuleContext<'a> {
  RuleContext {
    host: Host::Codex,
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
fn a_listed_server_denies_on_codex_with_the_redirect_hint() {
  let dir = tempfile::tempdir().expect("tempdir");
  fs::write(dir.path().join("mcp-blocklist.txt"), "example -> use CLI\n").expect("blocklist");
  let path = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("TOOLU_SETTINGS_DIR", path.as_str()),
    ("HOME", path.as_str()),
  ]);
  let raw = Map::new();
  let ctx = context(dir.path(), &env, &raw);
  let decision = McpBlocker
    .run(&tool("mcp__example_long__search"), &ctx)
    .expect("decision");
  match decision {
    Decision::Deny { reason } => {
      assert!(
        reason
          .as_str()
          .contains("listed in settings/mcp-blocklist.txt")
      );
      assert!(reason.as_str().contains("Use instead: use CLI"));
    }
    Decision::Allow
    | Decision::Ask { .. }
    | Decision::Advisory { .. }
    | Decision::Block { .. }
    | Decision::RuntimeFailure { .. } => panic!("listed server was allowed"),
  }
  assert_eq!(
    McpBlocker.run(&tool("mcp__other__search"), &ctx),
    Ok(Decision::Allow)
  );
}

#[test]
fn invalid_config_still_blocks_an_explicit_false_server() {
  let dir = tempfile::tempdir().expect("tempdir");
  fs::write(
    dir.path().join("toolu.config.json"),
    r#"{"version":2,"mcp":{"example":false}}"#,
  )
  .expect("config");
  let path = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("TOOLU_USER_CONFIG_DIR", path.as_str()),
    ("HOME", path.as_str()),
  ]);
  let raw = Map::new();
  let ctx = context(dir.path(), &env, &raw);
  let decision = McpBlocker
    .run(&tool("mcp__example__search"), &ctx)
    .expect("decision");
  assert!(matches!(decision, Decision::Deny { .. }));
}
