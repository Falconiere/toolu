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
    format!(
      "Toolu is on!\ntoolu runtime: native {VERSION} at {}",
      bin.display()
    )
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
      .starts_with("Toolu is on!\ntoolu runtime: native")
  );
  let blocked = sandbox
    .launch("PreToolUse", "no-such-hook", "{}", &[])
    .unwrap();
  assert_eq!(blocked.code, 2, "{blocked:?}");
  assert!(blocked.stderr.contains("has no hook no-such-hook"));
}

#[test]
fn check_binary_advises_the_installed_path_once_per_session() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  let input =
    r#"{"hook_event_name":"SessionStart","source":"startup","session_id":"native-check"}"#;
  let first = sandbox
    .launch("SessionStart", "check-binary", input, &[])
    .unwrap();
  assert_eq!(first.code, 0, "{first:?}");
  assert_eq!(
    system_message(&first).unwrap(),
    format!(
      "toolu: native binary for this session: '{}'. Use that absolute path for toolu commands.",
      sandbox.local_bin().display()
    )
  );
  let again = sandbox
    .launch("SessionStart", "check-binary", input, &[])
    .unwrap();
  assert_eq!(again.code, 0, "{again:?}");
  assert!(again.stdout.is_empty(), "{again:?}");
}
