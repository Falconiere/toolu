use std::path::Path;

use serde_json::{Map, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::Gate;

/// A gate that only implements `run`.
struct Plain;

impl Gate for Plain {
  fn name(&self) -> &'static str {
    "plain"
  }

  fn run(&self, _event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Result<Decision, String> {
    Err("from run".to_owned())
  }
}

#[test]
fn by_default_run_warning_is_run_with_no_warnings() {
  let event: NormalizedEvent =
    serde_json::from_value(json!({"type": "session/start", "sessionId": "s", "cwd": "/c", "projectRoot": "/c", "worktree": "/c"}))
      .unwrap();
  let env = Env::from_pairs([("CODEX_HOME", "/codex")]);
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Codex,
    env: &env,
    config_root: Path::new("/codex"),
    project_root: Path::new("/c"),
    cwd: Some(Path::new("/c")),
    raw: &raw,
    edit: None,
  };
  let mut warnings = vec!["kept".to_owned()];
  assert_eq!(
    Plain.run_warning(&event, &ctx, &mut warnings),
    Err("from run".to_owned())
  );
  assert_eq!(warnings, ["kept"], "nothing added, nothing taken");
}
