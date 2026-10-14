//! Run one case's hook with the built `toolu` and mask the sandbox and plugin paths.

use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::golden_lifecycle_cases::{Captured, Case, Res};
use crate::golden_lifecycle_sandbox::{EXECUTABLES, Prepared, prepare};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
const TIMEOUT: Duration = Duration::from_secs(30);

/// Host-session variables that must never leak from the developer's own agent.
const HOST_PREFIXES: [&str; 4] = ["CLAUDE_", "CODEX_", "TOOLU_", "CURSOR_"];

fn is_host_key(key: &str) -> bool {
  key == "PLUGIN_ROOT" || HOST_PREFIXES.iter().any(|prefix| key.starts_with(prefix))
}

fn event(hook: &str) -> Res<&'static str> {
  match hook {
    "session-start" => Ok("SessionStart"),
    "user-prompt-submit" => Ok("UserPromptSubmit"),
    "pre-compact" => Ok("PreCompact"),
    other => Err(format!("unknown lifecycle hook: {other}").into()),
  }
}

type Reader = thread::JoinHandle<std::io::Result<Vec<u8>>>;

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> Reader {
  thread::spawn(move || {
    let mut bytes = Vec::new();
    if let Some(mut stream) = stream {
      stream.read_to_end(&mut bytes)?;
    }
    Ok(bytes)
  })
}

fn join_text(handle: Reader) -> Res<String> {
  let bytes = handle.join().map_err(|_panic| "reader thread panicked")??;
  Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn spawn_hook(command: &mut Command) -> Res<Child> {
  let _shared = EXECUTABLES.read().map_err(|err| err.to_string())?;
  Ok(command.spawn()?)
}

fn command(case: &Case, prepared: &Prepared, plugin: &Path) -> Res<Command> {
  let mut command = Command::new(TOOLU);
  command
    .args([
      "hook",
      &case.hook,
      "--event",
      event(&case.hook)?,
      "--plugin-root",
    ])
    .arg(plugin)
    .current_dir(&prepared.cwd)
    .env_clear()
    .envs(std::env::vars_os().filter(|(key, _)| !is_host_key(&key.to_string_lossy())))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  for (key, value) in &prepared.env {
    match value {
      Some(value) => command.env(key, value),
      None => command.env_remove(key),
    };
  }
  Ok(command)
}

fn feed(child: &mut Child, stdin: &str) -> Res<()> {
  if let Some(mut pipe) = child.stdin.take() {
    match pipe.write_all(stdin.as_bytes()) {
      Ok(()) => {}
      // The hook may exit without reading; its exit status still tells the story.
      Err(err) if err.kind() == ErrorKind::BrokenPipe => {}
      Err(err) => return Err(err.into()),
    }
  }
  Ok(())
}

fn wait(child: &mut Child, name: &str) -> Res<ExitStatus> {
  let started = Instant::now();
  loop {
    if let Some(status) = child.try_wait()? {
      return Ok(status);
    }
    if started.elapsed() > TIMEOUT {
      child.kill()?;
      child.wait()?;
      return Err(format!("{name}: timed out after {TIMEOUT:?}").into());
    }
    thread::sleep(Duration::from_millis(5));
  }
}

/// Run `case`'s hook in a fresh sandbox; paths of the sandbox and `plugin` are masked.
pub(crate) fn run_case(case: &Case, plugin: &Path) -> Res<Captured> {
  let prepared = prepare(case, plugin)?;
  let mut command = command(case, &prepared, plugin)?;
  let mut child = spawn_hook(&mut command)?;
  let (out, err) = (drain(child.stdout.take()), drain(child.stderr.take()));
  feed(&mut child, &case.stdin)?;
  let status = wait(&mut child, &case.name)?;
  let mask = |text: String| {
    text
      .replace(&prepared.root.display().to_string(), "<SANDBOX>")
      .replace(&plugin.display().to_string(), "<PLUGIN>")
  };
  Ok(Captured {
    stdout: mask(join_text(out)?),
    stderr: mask(join_text(err)?),
    exit_code: status.code().unwrap_or(-1),
  })
}
