//! `run_hook`: the main of every native hook (#413). It reads the payload, calls
//! the hook, encodes the reply for the host and writes it. A panic, an internal
//! error or a lost write ends by the event's class, never with exit 101:
//!
//! - **enforcing** (`tool/pre`, `shell/pre`, `permission/evaluate`): `blocked: <line>`
//!   on stderr, exit 2;
//! - **post** (`tool/post`, `session/unload`): `<line>` on stderr, exit 2, as
//!   `plugins/toolu/hooks/src/pre-tools/hook-main.ts` does for `PostToolUse`;
//! - **context** (`session/start`, `prompt`, `pre_compact`): `{"systemMessage":"<line>"}`
//!   on stdout (a continue callback for `OpenCode`), exit 0.
//!
//! Host detection lives in `toolu-runtime`, so the caller passes the host in.

use std::fmt;
use std::io::{Read, Write};
use std::process::ExitCode;

use serde_json::Value;

use crate::decision::Decision;
use crate::encode::{Callback, CallbackAction, Encoded, encode};
use crate::event::HostEvent;
use crate::exit::Exit;
use crate::host::Host;
use crate::native::native_event;
use crate::stdin::read_all;

mod panic;

/// What a hook returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
  /// A decision to encode for the host and event.
  Decision(Decision),
  /// Output a dispatcher already merged, written as given.
  Raw(Raw),
}

/// Output written as given; any exit but [`Exit::Success`] ends as 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
  /// The stdout bytes.
  pub stdout: String,
  /// The stderr bytes.
  pub stderr: String,
  /// The hook's own outcome.
  pub exit: Exit,
}

/// An internal error: `toolu <event> <part> failed: <message>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookError {
  part: String,
  message: String,
}

/// The streams `run_hook_io` uses.
#[derive(Debug)]
pub struct Io<R, O, E> {
  /// Where the payload comes from.
  pub stdin: R,
  /// Where the encoded reply goes.
  pub stdout: O,
  /// Where failure lines go.
  pub stderr: E,
}

/// The output and outcome of one run, before it is written.
struct Output {
  stdout: String,
  stderr: String,
  exit: Exit,
}

/// How a failure ends on an event.
#[derive(Clone, Copy)]
enum Class {
  Enforcing,
  Post,
  Context,
}

impl From<Decision> for Reply {
  fn from(decision: Decision) -> Reply {
    Reply::Decision(decision)
  }
}

impl HookError {
  /// A failure of the hook itself: `hook failed: <message>`.
  pub fn new(message: impl Into<String>) -> HookError {
    HookError::in_part("hook", message)
  }

  /// A failure of one part, such as the dispatcher: `<part> failed: <message>`.
  pub fn in_part(part: impl Into<String>, message: impl Into<String>) -> HookError {
    HookError {
      part: part.into(),
      message: message.into(),
    }
  }
}

impl fmt::Display for HookError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{} failed: {}", self.part, self.message)
  }
}

impl std::error::Error for HookError {}

/// Run `hook` for `event` on `host` over the real standard streams.
pub fn run_hook<F>(event: HostEvent, host: Host, hook: F) -> ExitCode
where
  F: FnOnce(&str) -> Result<Reply, HookError>,
{
  let io = Io {
    stdin: std::io::stdin().lock(),
    stdout: std::io::stdout().lock(),
    stderr: std::io::stderr().lock(),
  };
  run_hook_io(io, event, host, hook)
}

/// Run `hook` for `event` on `host` over `io`: only exit 0 or 2.
pub fn run_hook_io<R, O, E, F>(io: Io<R, O, E>, event: HostEvent, host: Host, hook: F) -> ExitCode
where
  R: Read,
  O: Write,
  E: Write,
  F: FnOnce(&str) -> Result<Reply, HookError>,
{
  let Io {
    stdin,
    mut stdout,
    mut stderr,
  } = io;
  let output = match panic::catch(|| render(stdin, event, host, hook)) {
    Ok(Ok(output)) => output,
    Ok(Err(tail)) => failure(event, host, &tail),
    Err(message) => failure(event, host, &format!("hook panicked: {message}")),
  };
  let exit = match write(&mut stdout, &output.stdout) {
    Ok(()) => output.exit,
    Err(err) => lost(event, host, &err, &mut stderr),
  };
  // A closed stderr has nowhere left to report to.
  stderr.write_all(output.stderr.as_bytes()).ok();
  ExitCode::from(if exit == Exit::Success { 0 } else { 2 })
}

/// The hook's output, or the tail of its failure line.
fn render<F>(stdin: impl Read, event: HostEvent, host: Host, hook: F) -> Result<Output, String>
where
  F: FnOnce(&str) -> Result<Reply, HookError>,
{
  let payload = read_all(stdin)
    .map_err(|err| format!("hook failed: the hook payload could not be read: {err}"))?;
  match hook(&payload).map_err(|err| err.to_string())? {
    Reply::Raw(raw) => Ok(Output {
      stdout: raw.stdout,
      stderr: raw.stderr,
      exit: raw.exit,
    }),
    Reply::Decision(decision) => {
      let encoded = encode(host, event, &decision).map_err(|err| format!("hook failed: {err}"))?;
      let stdout = match encoded {
        Encoded::Command(stdout) => stdout,
        Encoded::Callback(callback) => format!("{}\n", callback.json()),
      };
      Ok(Output {
        stdout,
        stderr: String::new(),
        exit: Exit::Success,
      })
    }
  }
}

fn class(event: HostEvent) -> Class {
  match event {
    HostEvent::ToolPre | HostEvent::ShellPre | HostEvent::PermissionEvaluate => Class::Enforcing,
    HostEvent::ToolPost | HostEvent::SessionUnload => Class::Post,
    HostEvent::SessionStart | HostEvent::Prompt | HostEvent::PreCompact => Class::Context,
  }
}

/// `toolu <native event> <tail>`.
fn line(event: HostEvent, host: Host, tail: &str) -> String {
  let native = native_event(host, event).unwrap_or(event.slug());
  format!("toolu {native} {tail}")
}

/// The output of a failure on `event`, by its class.
fn failure(event: HostEvent, host: Host, tail: &str) -> Output {
  let line = line(event, host, tail);
  let (stdout, stderr, exit) = match class(event) {
    Class::Enforcing => (String::new(), format!("blocked: {line}\n"), Exit::Blocked),
    Class::Post => (String::new(), format!("{line}\n"), Exit::Blocked),
    Class::Context if host == Host::Opencode => {
      let callback = Callback {
        action: CallbackAction::Continue,
        message: Some(line),
      };
      (
        format!("{}\n", callback.json()),
        String::new(),
        Exit::Success,
      )
    }
    Class::Context => (
      format!("{{\"systemMessage\":{}}}\n", Value::from(line)),
      String::new(),
      Exit::Success,
    ),
  };
  Output {
    stdout,
    stderr,
    exit,
  }
}

fn write(stream: &mut impl Write, text: &str) -> std::io::Result<()> {
  stream.write_all(text.as_bytes())?;
  stream.flush()
}

/// A stdout write failed: enforcing and post events block, context events cannot report.
fn lost(event: HostEvent, host: Host, err: &std::io::Error, stderr: &mut impl Write) -> Exit {
  let line = line(
    event,
    host,
    &format!("hook could not write its output: {err}"),
  );
  let text = match class(event) {
    Class::Enforcing => format!("blocked: {line}\n"),
    Class::Post => format!("{line}\n"),
    Class::Context => return Exit::Success,
  };
  stderr.write_all(text.as_bytes()).ok();
  Exit::Blocked
}

#[cfg(test)]
#[path = "tests/hook_test.rs"]
mod tests;
