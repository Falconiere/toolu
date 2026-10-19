//! The generated Muse resolver (#546) driving the real `toolu` binary as Muse
//! runs it: `/bin/sh hooks/muse/<name>` with Muse's variables. A missing binary
//! blocks enforcing events and advises context events, and the #411 skew check
//! reads the packaged `.claude-plugin/plugin.json` as on Claude Code.

use toolu_protocol::HOOK_PROTOCOL;

use crate::sandbox::{STARTUP, Sandbox, VERSION, system_message};

const INSTALLER: &str = "curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash";
const BREW: &str = "brew install falconiere/tap/toolu";
const MISSING: &str = "toolu plugin: no usable native toolu";

#[test]
fn muse_enforcing_events_block_without_a_binary() {
  let sandbox = Sandbox::new().unwrap();
  for event in ["PreToolUse", "PermissionRequest"] {
    let run = sandbox.launch_muse(event, "pre-tools", "{}").unwrap();
    assert_eq!(run.code, 2, "{event}: {run:?}");
    assert_eq!(run.stdout, "", "{event}");
    assert!(
      run.stderr.starts_with(&format!("blocked: {MISSING}")),
      "{event}"
    );
    assert!(
      run.stderr.contains(INSTALLER) && run.stderr.contains(BREW),
      "{event}"
    );
  }
}

#[test]
fn muse_context_events_advise_without_a_binary() {
  let sandbox = Sandbox::new().unwrap();
  for event in [
    "SessionStart",
    "UserPromptSubmit",
    "PreCompact",
    "PostToolUse",
    "SessionEnd",
  ] {
    let run = sandbox
      .launch_muse(event, "session-start", STARTUP)
      .unwrap();
    assert_eq!(run.code, 0, "{event}: {run:?}");
    assert_eq!(run.stderr, "", "{event}");
    let message = system_message(&run).unwrap();
    assert!(message.starts_with(MISSING), "{event}: {message}");
    assert!(message.contains(INSTALLER), "{event}");
  }
}

#[test]
fn muse_runs_the_installed_binary_and_passes_its_status_through() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  let start = sandbox
    .launch_muse("SessionStart", "session-start", STARTUP)
    .unwrap();
  assert_eq!(start.code, 0, "{start:?}");
  assert!(!system_message(&start).unwrap().contains("hook protocol"));
  let blocked = sandbox
    .launch_muse("PreToolUse", "no-such-hook", "{}")
    .unwrap();
  assert_eq!(blocked.code, 2, "{blocked:?}");
  assert!(
    blocked.stderr.contains("has no hook no-such-hook"),
    "{blocked:?}"
  );
}

#[test]
fn muse_a_skewed_packaged_manifest_blocks_and_advises_as_on_claude() {
  for protocol in [HOOK_PROTOCOL + 1, HOOK_PROTOCOL - 1] {
    let sandbox = Sandbox::new().unwrap();
    sandbox.install().unwrap();
    sandbox.manifest(VERSION, &protocol.to_string()).unwrap();
    let pre = sandbox
      .launch_muse("PreToolUse", "session-start", STARTUP)
      .unwrap();
    assert_eq!(pre.code, 2, "{protocol}: {pre:?}");
    assert_eq!(pre.stdout, "", "{protocol}");
    let expected = format!("toolu plugin: hook protocol {protocol} ");
    assert!(
      pre.stderr.starts_with(&format!("blocked: {expected}")),
      "{pre:?}"
    );
    let start = sandbox
      .launch_muse("SessionStart", "session-start", STARTUP)
      .unwrap();
    assert_eq!(start.code, 0, "{protocol}: {start:?}");
    assert!(
      system_message(&start).unwrap().starts_with(&expected),
      "{start:?}"
    );
  }
}
