//! A binary that passes the signature check and then dies by a signal still
//! blocks an enforcing event (AC-8).

use crate::sandbox::{Res, Run, STARTUP, Sandbox, script};

fn dies_by(signal: &str, event: &str) -> Res<Run> {
  let sandbox = Sandbox::new()?;
  let text = format!(
    "#!/bin/sh\nif [ \"$1\" = --hook-protocol ]; then echo 1; exit 0; fi\nkill -{signal} $$\n"
  );
  script(&sandbox.local_bin(), &text)?;
  sandbox.launch(event, "pre-tools", STARTUP, &[])
}

/// The launcher's own line comes last; the shell may report the signal first.
fn last_line(run: &Run) -> &str {
  run.stderr.lines().last().unwrap_or_default()
}

#[test]
fn an_abort_blocks_with_its_status() {
  let run = dies_by("ABRT", "PreToolUse").unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: toolu plugin: toolu may have terminated on signal 6 (status 134), so the action is blocked. See docs/install.md."
  );
}

#[test]
fn a_segfault_blocks_with_its_status() {
  let run = dies_by("SEGV", "PermissionRequest").unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: toolu plugin: toolu may have terminated on signal 11 (status 139), so the action is blocked. See docs/install.md."
  );
}

#[test]
fn a_noncontract_exit_blocks_without_a_signal_hint() {
  let sandbox = Sandbox::new().unwrap();
  script(
    &sandbox.local_bin(),
    "#!/bin/sh\nif [ \"$1\" = --hook-protocol ]; then echo 1; exit 0; fi\nexit 101\n",
  )
  .unwrap();
  let run = sandbox
    .launch("PreToolUse", "pre-tools", STARTUP, &[])
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(
    last_line(&run),
    "blocked: toolu plugin: toolu exited with status 101, so the action is blocked. See docs/install.md."
  );
}
