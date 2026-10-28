//! Spawning the native `toolu` the way a host spawns a hook: an explicit environment
//! (the developer's own agent session never leaks in), stdin written then closed, and
//! stdout, stderr and the exit code captured.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::golden_gates_model::{Capture, Host, REPO, Res};
use crate::golden_gates_sandbox::Sandbox;

/// How long a hook may run before the case fails.
const DEADLINE: Duration = Duration::from_secs(30);

/// The binary under test.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// Environment overrides; `None` unsets the key.
pub(crate) type EnvPatch = BTreeMap<String, Option<String>>;

/// The toolu plugin's root.
pub(crate) fn plugin_root() -> PathBuf {
  Path::new(REPO).join("plugins").join("toolu")
}

/// A plugin's root.
pub(crate) fn plugin_dir(plugin: &str) -> PathBuf {
  Path::new(REPO).join("plugins").join(plugin)
}

/// Host-session variables that must never leak from the developer's own agent.
fn host_key(key: &str) -> bool {
  key == "PLUGIN_ROOT"
    || key.starts_with("GIT_")
    || ["CLAUDE_", "CODEX_", "TOOLU_", "CURSOR_", "OPENCODE"]
      .iter()
      .any(|prefix| key.starts_with(prefix))
}

/// The parent environment without host-session keys, then `patch` applied.
fn child_env(patch: &EnvPatch) -> BTreeMap<OsString, OsString> {
  let mut env: BTreeMap<OsString, OsString> = std::env::vars_os()
    .filter(|(key, _)| !host_key(&key.to_string_lossy()))
    .collect();
  for (key, value) in patch {
    match value {
      Some(value) => env.insert(key.into(), value.into()),
      None => env.remove(&OsString::from(key)),
    };
  }
  env
}

fn set(env: &mut EnvPatch, key: &str, value: &Path) {
  env.insert(key.to_owned(), Some(value.to_string_lossy().into_owned()));
}

/// The environment `host` puts on a hook process, rooted in the sandbox, then `extra`.
/// The host is detected as in a real session, so `TOOLU_HOST_OVERRIDE` is unset.
pub(crate) fn pretool_env(sb: &Sandbox, host: Host, extra: EnvPatch) -> EnvPatch {
  let plugin = plugin_root();
  let mut env = EnvPatch::new();
  set(&mut env, "HOME", &sb.home);
  env.insert("TOOLU_HOST_OVERRIDE".to_owned(), None);
  set(&mut env, "CLAUDE_PLUGIN_ROOT", &plugin);
  match host {
    Host::Claude => {
      set(&mut env, "CLAUDE_PROJECT_DIR", &sb.project);
      set(&mut env, "CLAUDE_CONFIG_DIR", &sb.home.join(".claude"));
    }
    Host::Codex => {
      set(&mut env, "PLUGIN_ROOT", &plugin);
      set(&mut env, "CODEX_HOME", &sb.codex_home);
    }
  }
  env.extend(extra);
  env
}

/// A path as an environment value.
pub(crate) fn lossy(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

/// What a spawn needs besides its arguments.
pub(crate) struct Call<'a> {
  /// The working directory.
  pub(crate) cwd: &'a Path,
  /// The environment.
  pub(crate) env: &'a EnvPatch,
  /// Standard input.
  pub(crate) stdin: &'a str,
}

/// Spawn `toolu` with `args` and capture its result.
pub(crate) fn spawn(args: &[&str], call: &Call<'_>) -> Res<Capture> {
  let mut command = Command::new(TOOLU);
  command
    .args(args)
    .current_dir(call.cwd)
    .env_clear()
    .envs(child_env(call.env))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let mut child = command.spawn()?;
  let mut stdin = child.stdin.take().ok_or("no stdin pipe")?;
  let body = call.stdin.as_bytes().to_vec();
  // A child that exits without reading closes the pipe; its exit status tells the story.
  let writer = thread::spawn(move || match stdin.write_all(&body) {
    Err(error) if error.kind() != ErrorKind::BrokenPipe => Err(error),
    _ => Ok(()),
  });
  let (stdout, stderr) = (drain(child.stdout.take()), drain(child.stderr.take()));
  let status = wait(&mut child, args)?;
  writer.join().map_err(|_panic| "stdin writer panicked")??;
  let code = status
    .code()
    .or_else(|| status.signal().map(|signal| 128 + signal))
    .ok_or("no exit status")?;
  Ok(Capture {
    stdout: join_text(stdout)?,
    stderr: join_text(stderr)?,
    exit_code: i64::from(code),
    files: None,
  })
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

/// Wait for `child`, killing it once it has run past `DEADLINE`.
fn wait(child: &mut Child, args: &[&str]) -> Res<ExitStatus> {
  let started = Instant::now();
  loop {
    if let Some(status) = child.try_wait()? {
      return Ok(status);
    }
    if started.elapsed() > DEADLINE {
      child.kill()?;
      child.wait()?;
      return Err(format!("toolu {args:?} ran over {DEADLINE:?} and was killed").into());
    }
    thread::sleep(Duration::from_millis(5));
  }
}

/// Run the native hook `entry` the way `hooks.json` runs it for `PreToolUse`.
pub(crate) fn run_hook(entry: &str, call: &Call<'_>) -> Res<Capture> {
  let root = plugin_root().to_string_lossy().into_owned();
  let args = [
    "hook",
    entry,
    "--event",
    "PreToolUse",
    "--plugin-root",
    root.as_str(),
  ];
  spawn(&args, call)
}

/// `text` with the sandbox root written `$ROOT`.
pub(crate) fn normalize(text: &str, root: &Path) -> String {
  text.replace(root.to_string_lossy().as_ref(), "$ROOT")
}

/// `capture` with its streams normalized.
pub(crate) fn normalized(capture: Capture, root: &Path) -> Capture {
  Capture {
    stdout: normalize(&capture.stdout, root),
    stderr: normalize(&capture.stderr, root),
    ..capture
  }
}
