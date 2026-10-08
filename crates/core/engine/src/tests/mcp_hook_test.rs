use super::*;

#[test]
fn a_non_tool_event_is_allowed() {
  use std::path::Path;
  use toolu_protocol::host::Host;
  use toolu_protocol::normalized::Session;
  use toolu_protocol::text::Text;
  use toolu_runtime::env::Env;
  let env = Env::process();
  let raw = serde_json::Map::new();
  let cwd = Path::new(".");
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: cwd,
    project_root: cwd,
    cwd: Some(cwd),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let session = Session {
    session_id: Text::new("s").unwrap(),
    cwd: Text::new(".").unwrap(),
    project_root: Text::new(".").unwrap(),
    worktree: Text::new(".").unwrap(),
  };
  let (decision, warnings) = check(&NormalizedEvent::SessionStart(session), &ctx);
  assert_eq!(decision.unwrap(), Decision::Allow);
  assert_eq!(warnings, Vec::<String>::new());
}
