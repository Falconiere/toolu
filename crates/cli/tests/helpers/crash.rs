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
  assert!(last_line(&run).starts_with("blocked: toolu plugin: toolu ended with status 134"));
}

#[test]
fn a_segfault_blocks_with_its_status() {
  let run = dies_by("SEGV", "PermissionRequest").unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert!(last_line(&run).starts_with("blocked: toolu plugin: toolu ended with status 139"));
}
