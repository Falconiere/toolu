//! The Muse fail-open guard (#544). Muse ignores a hook that exits 2 with JSON
//! on stdout or with a blank stderr, and it reads context only at exit 0. So
//! no path through `run_hook_io` may end that way on Muse. That covers every
//! decision, a panic, an internal error, an unreadable payload, a lost stdout
//! write and every `Raw` shape, on all 8 host events.

use std::io::{self, Read, Write};
use std::process::ExitCode;

use serde_json::json;
use toolu_protocol::decision::{Decision, GateClass};
use toolu_protocol::encode::degrade_ask;
use toolu_protocol::event::HostEvent;
use toolu_protocol::exit::Exit;
use toolu_protocol::hook::{HookError, Io, Raw, Reply, run_hook_io};
use toolu_protocol::host::Host;

/// A stream that is already closed: reads and writes fail.
struct Closed;

impl Read for Closed {
  fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
    Err(io::Error::other("closed"))
  }
}

impl Write for Closed {
  fn write(&mut self, _: &[u8]) -> io::Result<usize> {
    Err(io::Error::other("closed"))
  }
  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

type Hook = Box<dyn Fn() -> Result<Reply, HookError>>;

/// One path: its label, its stdin and stdout breakage, and the hook.
struct Path {
  label: String,
  stdin_closed: bool,
  stdout_closed: bool,
  hook: Hook,
}

fn path(label: impl Into<String>, hook: Hook) -> Path {
  Path {
    label: label.into(),
    stdin_closed: false,
    stdout_closed: false,
    hook,
  }
}

/// A helper's result; the `#[test]` functions unwrap it.
type Res<T> = Result<T, String>;

/// Every decision kind, both ask classes included.
fn decision_paths(event: HostEvent) -> Res<Vec<Path>> {
  let ask = json!({ "kind": "ask", "reason": "confirm" });
  let failure = json!({ "kind": "runtime_failure", "reason": "c", "code": "nonzero" });
  let rows = [
    ("allow", json!({ "kind": "allow" }), None),
    ("ask", ask.clone(), None),
    ("guardrail ask", ask.clone(), Some(GateClass::Guardrail)),
    ("judgement ask", ask, Some(GateClass::Judgement)),
    ("deny", json!({ "kind": "deny", "reason": "no" }), None),
    (
      "advice",
      json!({ "kind": "advisory", "message": "m" }),
      None,
    ),
    (
      "post block",
      json!({ "kind": "post_block", "reason": "l" }),
      None,
    ),
    ("runtime failure", failure, None),
  ];
  let mut paths = Vec::new();
  for (label, doc, class) in rows {
    let decision: Decision = serde_json::from_value(doc).map_err(|err| err.to_string())?;
    let decision = match class {
      Some(class) => degrade_ask(Host::Muse, event, decision, class),
      None => decision,
    };
    paths.push(path(
      label,
      Box::new(move || Ok(Reply::from(decision.clone()))),
    ));
  }
  Ok(paths)
}

/// Every `Raw` shape: exit 0, 2 or 64, stdout empty or JSON, stderr blank or text.
fn raw_paths() -> Vec<Path> {
  let mut paths = Vec::new();
  for exit in [Exit::Success, Exit::Blocked, Exit::Usage] {
    for stdout in ["", "{\"decision\":\"block\",\"reason\":\"x\"}\n"] {
      for stderr in ["", " \n", "reason\n"] {
        let raw = Raw {
          stdout: stdout.to_owned(),
          stderr: stderr.to_owned(),
          exit,
        };
        let label = format!("raw {exit:?} {stdout:?} {stderr:?}");
        paths.push(path(label, Box::new(move || Ok(Reply::Raw(raw.clone())))));
      }
    }
  }
  paths
}

/// Every path on `event`; `panic` is the test's own panicking hook.
fn every_path(event: HostEvent, panic: Hook) -> Res<Vec<Path>> {
  let mut paths = decision_paths(event)?;
  let mut unreadable = path(
    "unreadable stdin",
    Box::new(|| Ok(Reply::from(Decision::Allow))),
  );
  unreadable.stdin_closed = true;
  let deny = json!({ "kind": "deny", "reason": "n" });
  let deny: Decision = serde_json::from_value(deny).map_err(|err| err.to_string())?;
  let mut lost = path(
    "lost stdout",
    Box::new(move || Ok(Reply::from(deny.clone()))),
  );
  lost.stdout_closed = true;
  paths.extend([
    path("panic", panic),
    path("error", Box::new(|| Err(HookError::new("boom")))),
    unreadable,
    lost,
  ]);
  paths.extend(raw_paths());
  Ok(paths)
}

/// The exit code, stdout and stderr `path` leaves on `event`.
fn run(event: HostEvent, path: &Path) -> (ExitCode, String, String) {
  let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
  let stdin: Box<dyn Read> = if path.stdin_closed {
    Box::new(Closed)
  } else {
    Box::new(&b"{}"[..])
  };
  let out: Box<dyn Write + '_> = if path.stdout_closed {
    Box::new(Closed)
  } else {
    Box::new(&mut stdout)
  };
  let io = Io {
    stdin,
    stdout: out,
    stderr: &mut stderr,
  };
  let code = run_hook_io(io, event, Host::Muse, |_| (path.hook)());
  let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
  (code, text(&stdout), text(&stderr))
}

#[test]
fn no_muse_path_exits_2_with_stdout_or_a_blank_stderr() {
  let mut checked = 0;
  for event in HostEvent::ALL {
    for path in every_path(event, Box::new(|| panic!("boom"))).unwrap() {
      let (code, stdout, stderr) = run(event, &path);
      let label = format!("{event:?} {}", path.label);
      if code == ExitCode::from(2) {
        assert_eq!(stdout, "", "{label}");
        assert!(!stderr.trim().is_empty(), "{label}");
      }
      checked += 1;
    }
  }
  assert_eq!(checked, 8 * 30);
}

#[test]
fn every_muse_path_on_a_context_event_exits_0() {
  for event in [
    HostEvent::SessionStart,
    HostEvent::Prompt,
    HostEvent::PreCompact,
  ] {
    for path in every_path(event, Box::new(|| panic!("boom"))).unwrap() {
      let (code, _, stderr) = run(event, &path);
      assert_eq!(code, ExitCode::from(0), "{event:?} {}", path.label);
      // A successful raw reply is written as given; every other path renders its own output.
      if !path.stdout_closed && !path.label.starts_with("raw Success") {
        assert_eq!(stderr, "", "{event:?} {}", path.label);
      }
    }
  }
}

#[test]
fn a_failure_on_an_enforcing_event_blocks_with_a_stderr_line() {
  for (event, native) in [
    (HostEvent::ToolPre, "PreToolUse"),
    (HostEvent::PermissionEvaluate, "PermissionRequest"),
  ] {
    let paths = every_path(event, Box::new(|| panic!("boom"))).unwrap();
    for (label, tail) in [
      ("panic", "hook panicked: boom"),
      ("error", "hook failed: boom"),
      (
        "unreadable stdin",
        "hook failed: the hook payload could not be read: closed",
      ),
      ("lost stdout", "hook could not write its output: closed"),
    ] {
      let path = paths.iter().find(|p| p.label == label).unwrap();
      let (code, stdout, stderr) = run(event, path);
      assert_eq!(code, ExitCode::from(2), "{event:?} {label}");
      assert_eq!(stdout, "", "{event:?} {label}");
      assert_eq!(
        stderr,
        format!("blocked: toolu {native} {tail}\n"),
        "{label}"
      );
    }
  }
}
