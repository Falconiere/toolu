//! `run_hook` in a real process (AC-5, AC-6). This test binary has no libtest
//! harness: run plainly it is the parent, and it runs itself as a child with
//! `TOOLU_PROTOCOL_CHILD=<mode>`, real stdin, stdout and stderr, and real exit
//! codes. A panicking hook exits 2 before and after a tool and 0 with a
//! `systemMessage` at session start; a write to a closed stdout follows the
//! event's class; a panic outside `run_hook` is still Rust's (101).
//!
//! `main` is not test code to clippy, so it unwraps nothing and returns errors.

use std::error::Error;
use std::io::{Read, Write};
use std::process::{Command, ExitCode, Stdio};

use toolu_protocol::decision::Decision;
use toolu_protocol::event::HostEvent;
use toolu_protocol::hook::{HookError, Reply, run_hook};
use toolu_protocol::host::Host;
use toolu_protocol::text::Text;

const MODE: &str = "TOOLU_PROTOCOL_CHILD";

type Res<T> = Result<T, Box<dyn Error>>;

fn main() -> ExitCode {
  if let Ok(mode) = std::env::var(MODE) {
    return child(&mode);
  }
  match parent() {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      std::io::stderr()
        .write_all(format!("run_hook: {err}\n").as_bytes())
        .ok();
      ExitCode::FAILURE
    }
  }
}

/// A decision of `kind` whose reason or message is `text`.
fn decided(kind: &str, text: &str) -> Result<Reply, HookError> {
  let text = Text::new(text).map_err(|err| HookError::new(err.to_string()))?;
  Ok(Reply::from(match kind {
    "advise" => Decision::Advisory { message: text },
    "block" => Decision::Block { reason: text },
    _ => Decision::Deny { reason: text },
  }))
}

/// The child: one `run_hook` call on the real streams.
fn child(mode: &str) -> ExitCode {
  let (event, kind) = match mode {
    "panic-pre" => (HostEvent::ToolPre, "panic"),
    "panic-post" => (HostEvent::ToolPost, "panic"),
    "panic-start" => (HostEvent::SessionStart, "panic"),
    "block" => (HostEvent::ToolPost, "block"),
    "advise" => (HostEvent::SessionStart, "advise"),
    _ => (HostEvent::ToolPre, "deny"),
  };
  let code = run_hook(event, Host::Claude, |payload| {
    assert!(kind != "panic", "boom");
    decided(kind, &format!("protected file {}", payload.len()))
  });
  assert!(mode != "outside", "boom outside");
  code
}

/// What a child left behind.
struct Run {
  code: Option<i32>,
  stdout: String,
  stderr: String,
}

/// Run the child in `mode` on `{}`; with `close` its stdout reader is gone first.
fn spawn(mode: &str, close: bool) -> Res<Run> {
  let mut child = Command::new(std::env::current_exe()?)
    .env(MODE, mode)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  let stdout = child.stdout.take().ok_or("no stdout")?;
  let reader = if close {
    drop(stdout);
    None
  } else {
    Some(stdout)
  };
  let mut stdin = child.stdin.take().ok_or("no stdin")?;
  stdin.write_all(b"{}")?;
  drop(stdin);
  let mut out = String::new();
  if let Some(mut reader) = reader {
    reader.read_to_string(&mut out)?;
  }
  let mut err = String::new();
  child
    .stderr
    .take()
    .ok_or("no stderr")?
    .read_to_string(&mut err)?;
  let status = child.wait()?;
  Ok(Run {
    code: status.code(),
    stdout: out,
    stderr: err,
  })
}

fn expect(mode: &str, run: &Run, code: i32, stdout: &str, stderr: &str) -> Res<()> {
  if run.code == Some(code) && run.stdout == stdout && run.stderr == stderr {
    return Ok(());
  }
  Err(
    format!(
      "{mode}: got {:?} {:?} {:?}, want {code} {stdout:?} {stderr:?}",
      run.code, run.stdout, run.stderr
    )
    .into(),
  )
}

fn expect_lost(mode: &str, run: &Run, code: i32, prefix: &str) -> Res<()> {
  if run.code == Some(code) && run.stderr.starts_with(prefix) {
    return Ok(());
  }
  Err(
    format!(
      "{mode}: got {:?} {:?}, want {code} {prefix:?}…",
      run.code, run.stderr
    )
    .into(),
  )
}

fn parent() -> Res<()> {
  let pre = spawn("panic-pre", false)?;
  expect(
    "panic-pre",
    &pre,
    2,
    "",
    "blocked: toolu PreToolUse hook panicked: boom\n",
  )?;
  let post = spawn("panic-post", false)?;
  expect(
    "panic-post",
    &post,
    2,
    "",
    "toolu PostToolUse hook panicked: boom\n",
  )?;
  let start = spawn("panic-start", false)?;
  let message = "{\"systemMessage\":\"toolu SessionStart hook panicked: boom\"}\n";
  expect("panic-start", &start, 0, message, "")?;
  let deny = spawn("deny", false)?;
  let encoded = "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\
                 \"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"protected file 2\"}}\n";
  expect("deny", &deny, 0, encoded, "")?;
  let lost = spawn("deny", true)?;
  expect_lost(
    "deny closed",
    &lost,
    2,
    "blocked: toolu PreToolUse hook could not write its output: ",
  )?;
  let lost = spawn("block", true)?;
  expect_lost(
    "block closed",
    &lost,
    2,
    "toolu PostToolUse hook could not write its output: ",
  )?;
  let lost = spawn("advise", true)?;
  expect("advise closed", &lost, 0, "", "")?;
  let outside = spawn("outside", false)?;
  if outside.code != Some(101) || !outside.stderr.contains("boom outside") {
    return Err(format!("outside: got {:?} {:?}", outside.code, outside.stderr).into());
  }
  Ok(())
}
