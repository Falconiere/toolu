use std::path::Path;

use serde_json::Map;
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session};
use toolu_protocol::text::Text;
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
  let text = |value: &str| Text::new(value).unwrap();
  let event = NormalizedEvent::SessionStart(Session {
    session_id: text("s"),
    cwd: text("/p"),
    project_root: text("/p"),
    worktree: text("/p"),
  });
  let env = Env::from_pairs([("HOME", "/h")]);
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: Path::new("/h/.claude"),
    project_root: Path::new("/p"),
    cwd: None,
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
