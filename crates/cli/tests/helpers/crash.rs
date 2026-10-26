//! A binary that passes the signature check and then dies by a signal or exits
//! outside the hook contract: an enforcing event blocks (AC-8), and a context
//! event reports a `systemMessage` (#642). Both name the hook, its event and the
//! status, without inferring the cause from the status.

use crate::sandbox::{Res, Run, STARTUP, Sandbox, script, system_message};

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

/// Codex shows a killed context hook only as "hook exited without a status code".
#[test]
fn a_killed_context_hook_reports_its_hook_and_status_without_failing() {
  let run = stand_in("kill -KILL $$", "SessionStart", "session-start").unwrap();
  assert_eq!(run.code, 0, "{run:?}");
  assert_eq!(
    system_message(&run).unwrap(),
    "toolu plugin: toolu hook session-start on SessionStart ended with status 137. See \
     docs/install.md."
  );
}

/// Codex drops a context hook's stderr, so a panic's exit 101 was anonymous there.
#[test]
fn a_panicking_context_hook_reports_its_hook_and_status_without_failing() {
  let run = stand_in(
    "echo \"thread 'main' panicked\" >&2; exit 101",
    "UserPromptSubmit",
    "user-prompt-submit",
  )
  .unwrap();
  assert_eq!(run.code, 0, "{run:?}");
  assert_eq!(
    system_message(&run).unwrap(),
    "toolu plugin: toolu hook user-prompt-submit on UserPromptSubmit ended with status 101. See \
     docs/install.md."
  );
  assert_eq!(run.stderr, "thread 'main' panicked\n");
}

#[test]
fn a_context_hook_passes_its_feedback_exit_through() {
  let run = stand_in(
    "echo '{\"systemMessage\":\"own\"}'; echo feedback >&2; exit 2",
    "PostToolUse",
    "post-tools",
  )
  .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(run.stdout, "{\"systemMessage\":\"own\"}\n");
  assert_eq!(run.stderr, "feedback\n");
}
