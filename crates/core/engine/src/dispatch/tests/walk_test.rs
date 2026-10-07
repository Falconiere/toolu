use toolu_protocol::decision::Decision;
use toolu_protocol::event::HostEvent;
use toolu_protocol::host::Host;
use toolu_protocol::text::Text;

use super::{encoded, failed};
use crate::dispatch::Phase;

#[test]
fn codex_gets_codex_output_and_every_other_host_claudes() {
  let ask = Decision::Ask {
    reason: Text::new("confirm?").unwrap(),
  };
  let claude = encoded(Host::Claude, HostEvent::ToolPre, &ask);
  assert!(
    claude.contains("\"permissionDecision\":\"ask\""),
    "{claude}"
  );
  assert!(!claude.ends_with('\n'));
  for host in [Host::Cursor, Host::Hermes, Host::Opencode] {
    assert_eq!(encoded(host, HostEvent::ToolPre, &ask), claude, "{host:?}");
  }
  let codex = encoded(Host::Codex, HostEvent::ToolPre, &ask);
  assert!(codex.contains("\"permissionDecision\":\"deny\""), "{codex}");
  assert_eq!(
    encoded(Host::Claude, HostEvent::ToolPre, &Decision::Allow),
    ""
  );
}

#[test]
fn the_dispatchers_own_failure_blocks() {
  let pre = failed(Phase::Pre, "boom");
  assert_eq!(
    (pre.stderr.as_str(), pre.exit_code),
    ("blocked: toolu PreToolUse dispatcher failed: boom\n", 2)
  );
  assert_eq!(
    failed(Phase::Post, "x").stderr,
    "toolu PostToolUse dispatcher failed: x\n"
  );
}
