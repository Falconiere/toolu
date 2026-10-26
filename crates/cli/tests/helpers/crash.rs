//! A binary that passes the signature check and then dies by a signal or exits
//! outside the hook contract. An enforcing event blocks naming the hook, its event
//! and the status, without inferring the cause from it (AC-8, #642). A context
//! event `exec`s toolu, so its status reaches the host unchanged.

use crate::sandbox::{Res, Run, STARTUP, Sandbox, script};

/// Launch `name` on `event` against a stand-in that runs `body` after the signature check.
fn stand_in(body: &str, event: &str, name: &str) -> Res<Run> {
  let sandbox = Sandbox::new()?;
  let text =
    format!("#!/bin/sh\nif [ \"$1\" = --hook-protocol ]; then echo 1; exit 0; fi\n{body}\n");
  script(&sandbox.local_bin(), &text)?;
  sandbox.launch(event, name, STARTUP, &[])
}

/// The launcher's own line comes last; the shell may report the signal first.
fn last_line(run: &Run) -> &str {
  run.stderr.lines().last().unwrap_or_default()
}

#[test]
fn an_abort_blocks_with_its_hook_and_status() {
  let run = stand_in("kill -ABRT $$", "PreToolUse", "pre-tools").unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: toolu plugin: toolu hook pre-tools on PreToolUse ended with status 134, so the \
     action is blocked. See docs/install.md."
  );
}

#[test]
fn a_segfault_blocks_with_its_hook_and_status() {
  let run = stand_in("kill -SEGV $$", "PermissionRequest", "permission").unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: toolu plugin: toolu hook permission on PermissionRequest ended with status 139, so \
     the action is blocked. See docs/install.md."
  );
}

#[test]
fn a_noncontract_exit_blocks_with_its_hook_and_status() {
  let run = stand_in("exit 101", "PreToolUse", "pre-tools").unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: toolu plugin: toolu hook pre-tools on PreToolUse ended with status 101, so the \
     action is blocked. See docs/install.md."
  );
}

#[test]
fn a_regular_exit_in_the_signal_shaped_range_reports_only_its_status() {
  let run = stand_in("exit 130", "PreToolUse", "pre-tools").unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: toolu plugin: toolu hook pre-tools on PreToolUse ended with status 130, so the \
     action is blocked. See docs/install.md."
  );
}

#[test]
fn another_plugins_enforcing_crash_names_its_own_hook() {
  let sandbox = Sandbox::new().unwrap();
  let text = "#!/bin/sh\nif [ \"$1\" = --hook-protocol ]; then echo 1; exit 0; fi\nkill -ABRT $$\n";
  script(&sandbox.local_bin(), text).unwrap();
  let run = sandbox
    .launch_plugin("jev", "PreToolUse", "probe", STARTUP)
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: jev plugin: toolu jev hook probe on PreToolUse ended with status 134, so the action \
     is blocked. See docs/install.md."
  );
}

/// A context launcher `exec`s toolu, so a host timeout reaches toolu itself and a
/// signal reaches the host unchanged; toolu names its own panics and errors (`guard`).
#[test]
fn a_killed_context_hook_reaches_the_host_unchanged() {
  let run = stand_in("kill -KILL $$", "SessionStart", "session-start").unwrap();
  assert_eq!(run.code, -1, "{run:?}");
  assert_eq!(run.stdout, "");
}

#[test]
fn a_context_hook_status_and_output_pass_through_unchanged() {
  for status in [2, 101] {
    let body = format!("echo '{{\"systemMessage\":\"own\"}}'; echo detail >&2; exit {status}");
    let run = stand_in(&body, "PostToolUse", "post-tools").unwrap();
    assert_eq!(run.code, status, "{run:?}");
    assert_eq!(run.stdout, "{\"systemMessage\":\"own\"}\n");
    assert_eq!(run.stderr, "detail\n");
  }
}
