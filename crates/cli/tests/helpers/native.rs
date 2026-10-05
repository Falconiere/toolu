//! The binary in `~/.local/bin` runs under a `PATH` without it (AC-4).

use crate::sandbox::{STARTUP, Sandbox, VERSION, system_message};

#[test]
fn session_start_reports_the_native_runtime_at_its_path() {
  let sandbox = Sandbox::new().unwrap();
  let bin = sandbox.install().unwrap();
  let run = sandbox
    .launch("SessionStart", "session-start", STARTUP, &[])
    .unwrap();
  assert_eq!(run.code, 0, "{run:?}");
  assert_eq!(
    system_message(&run).unwrap(),
    format!("toolu runtime: native {VERSION} at {}", bin.display())
  );
}

#[test]
fn an_enforcing_event_runs_the_hook_and_passes_its_status_through() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  let ran = sandbox
    .launch("PreToolUse", "session-start", STARTUP, &[])
    .unwrap();
  assert_eq!(ran.code, 0, "{ran:?}");
  assert!(
    system_message(&ran)
      .unwrap()
      .starts_with("toolu runtime: native")
  );
  let blocked = sandbox
    .launch("PreToolUse", "pre-tools", "{}", &[])
    .unwrap();
  assert_eq!(blocked.code, 2, "{blocked:?}");
  assert!(blocked.stderr.contains("has no hook pre-tools"));
}
