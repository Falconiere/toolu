//! `run_hook_io` on in-memory streams: the reply paths and the failure classes.

use std::io::{self, Write};
use std::process::ExitCode;

use super::{HookError, Io, Raw, Reply, run_hook_io};
use crate::decision::Decision;
use crate::event::HostEvent;
use crate::exit::Exit;
use crate::host::Host;
use crate::text::Text;

/// What one run left: exit code, stdout, stderr.
#[derive(Debug, PartialEq)]
struct Run {
  exit: ExitCode,
  stdout: String,
  stderr: String,
}

fn run<F>(event: HostEvent, host: Host, stdin: &[u8], hook: F) -> Run
where
  F: FnOnce(&str) -> Result<Reply, HookError>,
{
  let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
  let io = Io {
    stdin,
    stdout: &mut stdout,
    stderr: &mut stderr,
  };
  let exit = run_hook_io(io, event, host, hook);
  Run {
    exit,
    stdout: String::from_utf8(stdout).unwrap(),
    stderr: String::from_utf8(stderr).unwrap(),
  }
}

fn deny(reason: &str) -> Reply {
  Reply::from(Decision::Deny {
    reason: Text::new(reason).unwrap(),
  })
}

fn fails(_: &str) -> Result<Reply, HookError> {
  Err(HookError::new("boom"))
}

#[test]
fn the_hook_gets_the_payload_and_its_decision_is_encoded_for_the_host() {
  let got = run(
    HostEvent::ToolPre,
    Host::Claude,
    b"{\"tool_name\":\"Edit\"}",
    |payload| {
      assert_eq!(payload, "{\"tool_name\":\"Edit\"}");
      Ok(deny("protected file"))
    },
  );
  assert_eq!(got.exit, ExitCode::from(0));
  assert_eq!(
    got.stdout,
    "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\
     \"permissionDecisionReason\":\"protected file\"}}\n"
  );
  assert_eq!(got.stderr, "");
}

#[test]
fn an_opencode_decision_is_one_callback_line() {
  let got = run(HostEvent::ToolPre, Host::Opencode, b"{}", |_| {
    Ok(deny("no"))
  });
  assert_eq!(
    got.stdout,
    "{\"kind\":\"callback\",\"action\":\"throw\",\"message\":\"no\"}\n"
  );
  assert_eq!(got.exit, ExitCode::from(0));
}

#[test]
fn a_raw_reply_is_written_as_given_and_only_success_exits_0() {
  let raw = |exit| {
    Ok(Reply::Raw(Raw {
      stdout: "{\"a\": 1}\n".to_owned(),
      stderr: "note\n".to_owned(),
      exit,
    }))
  };
  for (exit, code) in [(Exit::Success, 0), (Exit::Blocked, 2), (Exit::Usage, 2)] {
    for event in [HostEvent::ToolPre, HostEvent::SessionStart] {
      let got = run(event, Host::Claude, b"", |_| raw(exit));
      assert_eq!(
        got,
        Run {
          exit: ExitCode::from(code),
          stdout: "{\"a\": 1}\n".to_owned(),
          stderr: "note\n".to_owned(),
        },
        "{exit:?} {event:?}"
      );
    }
  }
}

#[test]
fn an_internal_error_ends_by_the_event_class() {
  let blocked = |native: &str| format!("blocked: toolu {native} hook failed: boom\n");
  for (event, native) in [
    (HostEvent::ToolPre, "PreToolUse"),
    (HostEvent::ShellPre, "PreToolUse"),
    (HostEvent::PermissionEvaluate, "PermissionRequest"),
  ] {
    let got = run(event, Host::Claude, b"{}", fails);
    assert_eq!(
      (got.exit, got.stdout, got.stderr),
      (ExitCode::from(2), String::new(), blocked(native))
    );
  }
  for (event, native) in [
    (HostEvent::ToolPost, "PostToolUse"),
    (HostEvent::SessionUnload, "SessionEnd"),
  ] {
    let got = run(event, Host::Codex, b"{}", fails);
    let line = format!("toolu {native} hook failed: boom\n");
    assert_eq!(
      (got.exit, got.stdout, got.stderr),
      (ExitCode::from(2), String::new(), line)
    );
  }
  for (event, native) in [
    (HostEvent::SessionStart, "SessionStart"),
    (HostEvent::Prompt, "UserPromptSubmit"),
    (HostEvent::PreCompact, "PreCompact"),
  ] {
    let got = run(event, Host::Claude, b"{}", fails);
    let message = format!("{{\"systemMessage\":\"toolu {native} hook failed: boom\"}}\n");
    assert_eq!(
      (got.exit, got.stdout, got.stderr),
      (ExitCode::from(0), message, String::new())
    );
  }
}

#[test]
fn a_dispatcher_failure_keeps_todays_line() {
  let got = run(HostEvent::ToolPre, Host::Claude, b"{}", |_| {
    Err(HookError::in_part("dispatcher", "walk exploded"))
  });
  assert_eq!(
    got.stderr,
    "blocked: toolu PreToolUse dispatcher failed: walk exploded\n"
  );
  assert_eq!(got.exit, ExitCode::from(2));
}

#[test]
fn an_unreadable_payload_is_an_internal_error() {
  let got = run(HostEvent::ToolPre, Host::Claude, &[0xff, 0xfe], |_| {
    Ok(Reply::from(Decision::Allow))
  });
  assert!(
    got
      .stderr
      .starts_with("blocked: toolu PreToolUse hook failed: the hook payload could not be read: "),
    "{got:?}"
  );
  assert_eq!(got.exit, ExitCode::from(2));
}

#[test]
fn an_event_the_host_lacks_is_an_internal_error_named_by_its_slug() {
  let got = run(HostEvent::PermissionEvaluate, Host::Cursor, b"{}", |_| {
    Ok(deny("x"))
  });
  assert_eq!(
    got.stderr,
    "blocked: toolu permission/evaluate hook failed: cursor has no native event for \
     permission/evaluate\n"
  );
  assert_eq!(got.exit, ExitCode::from(2));
}

#[test]
fn an_opencode_context_failure_is_a_continue_callback() {
  let got = run(HostEvent::SessionStart, Host::Opencode, b"{}", fails);
  assert_eq!(
    got.stdout,
    "{\"kind\":\"callback\",\"action\":\"continue\",\"message\":\"toolu session.created hook \
     failed: boom\"}\n"
  );
  assert_eq!(got.exit, ExitCode::from(0));
}

#[test]
fn a_panic_is_caught_and_ends_by_the_event_class() {
  let got = run(HostEvent::ToolPre, Host::Claude, b"{}", |_| panic!("boom"));
  assert_eq!(
    got.stderr,
    "blocked: toolu PreToolUse hook panicked: boom\n"
  );
  assert_eq!(got.exit, ExitCode::from(2));
  let start = run(HostEvent::SessionStart, Host::Claude, b"{}", |_| {
    panic!("boom")
  });
  assert_eq!(
    start.stdout,
    "{\"systemMessage\":\"toolu SessionStart hook panicked: boom\"}\n"
  );
  assert_eq!(start.exit, ExitCode::from(0));
}

/// A stdout whose reader is gone.
struct Closed;

impl Write for Closed {
  fn write(&mut self, _: &[u8]) -> io::Result<usize> {
    Err(io::Error::from(io::ErrorKind::BrokenPipe))
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

fn closed<F>(event: HostEvent, hook: F) -> (ExitCode, String)
where
  F: FnOnce(&str) -> Result<Reply, HookError>,
{
  let mut stderr = Vec::new();
  let io = Io {
    stdin: &b"{}"[..],
    stdout: Closed,
    stderr: &mut stderr,
  };
  let exit = run_hook_io(io, event, Host::Claude, hook);
  (exit, String::from_utf8(stderr).unwrap())
}

#[test]
fn a_lost_write_blocks_enforcing_and_post_events_but_not_context_events() {
  let (exit, stderr) = closed(HostEvent::ToolPre, |_| Ok(deny("no")));
  assert_eq!(exit, ExitCode::from(2));
  assert!(
    stderr.starts_with("blocked: toolu PreToolUse hook could not write its output: "),
    "{stderr}"
  );
  let (exit, stderr) = closed(HostEvent::ToolPost, |_| Ok(deny("no")));
  assert_eq!(exit, ExitCode::from(2));
  assert!(
    stderr.starts_with("toolu PostToolUse hook could not write its output: "),
    "{stderr}"
  );
  let (exit, stderr) = closed(HostEvent::SessionStart, |_| Ok(deny("no")));
  assert_eq!((exit, stderr), (ExitCode::from(0), String::new()));
}

#[test]
fn nothing_to_write_cannot_be_lost() {
  let (exit, stderr) = closed(HostEvent::ToolPre, |_| Ok(Reply::from(Decision::Allow)));
  assert_eq!((exit, stderr), (ExitCode::from(0), String::new()));
}

#[test]
fn a_hook_error_names_its_part() {
  assert_eq!(HookError::new("x").to_string(), "hook failed: x");
  let err: &dyn std::error::Error = &HookError::in_part("dispatcher", "y");
  assert_eq!(err.to_string(), "dispatcher failed: y");
}
