//! Spawning the real `toolu` with an explicit environment, stdin and a deadline.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::RwLock;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::Res;

/// The `toolu` binary cargo built for these tests.
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// How long a hook may run before the case fails.
const DEADLINE: Duration = Duration::from_secs(60);

/// Writers of executables hold this exclusively, spawners shared: a child
/// forked while a script is open for writing would make its `exec` fail with
/// `ETXTBSY` on Linux.
static EXECUTABLES: RwLock<()> = RwLock::new(());

/// What one process printed and how it exited.
#[derive(Debug)]
pub(crate) struct Ran {
  /// The exit status; -1 when a signal ended it.
  pub(crate) code: i32,
  /// Standard output.
  pub(crate) stdout: String,
  /// Standard error.
  pub(crate) stderr: String,
}

/// Write an executable script at `path` while no child is being spawned.
pub(crate) fn write_script(path: &Path, text: &str) -> Res<()> {
  let _exclusive = EXECUTABLES.write().map_err(|err| err.to_string())?;
  std::fs::write(path, text)?;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

fn spawn(command: &mut Command) -> Res<Child> {
  let _shared = EXECUTABLES.read().map_err(|err| err.to_string())?;
  Ok(command.spawn()?)
}

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> JoinHandle<std::io::Result<Vec<u8>>> {
  std::thread::spawn(move || {
    let mut bytes = Vec::new();
    if let Some(mut stream) = stream {
      stream.read_to_end(&mut bytes)?;
    }
    Ok(bytes)
  })
}

fn collect(reader: JoinHandle<std::io::Result<Vec<u8>>>) -> Res<String> {
  let bytes = reader
    .join()
    .map_err(|panic| format!("output reader panicked: {panic:?}"))??;
  Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn wait(child: &mut Child) -> Res<i32> {
  let started = Instant::now();
  loop {
    if let Some(status) = child.try_wait()? {
      return Ok(status.code().unwrap_or(-1));
    }
    if started.elapsed() > DEADLINE {
      child.kill()?;
      child.wait()?;
      return Err(format!("toolu ran over {DEADLINE:?} and was killed").into());
    }
    std::thread::sleep(Duration::from_millis(2));
  }
}

/// Run the built `toolu` with `args` in `cwd` under exactly `env`, `stdin` on its input.
pub(crate) fn toolu(
  args: &[&str],
  cwd: &Path,
  env: &BTreeMap<String, String>,
  stdin: &str,
) -> Res<Ran> {
  let mut command = Command::new(TOOLU);
  command
    .args(args)
    .env_clear()
    .envs(env)
    .current_dir(cwd)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let mut child = spawn(&mut command)?;
  let stdout = drain(child.stdout.take());
  let stderr = drain(child.stderr.take());
  let mut input = child.stdin.take().ok_or("no stdin pipe")?;
  // A hook that ends without reading its input leaves a broken pipe; its status tells.
  match input.write_all(stdin.as_bytes()) {
    Err(err) if err.kind() != std::io::ErrorKind::BrokenPipe => return Err(err.into()),
    Ok(()) | Err(_) => {}
  }
  drop(input);
  let code = wait(&mut child)?;
  Ok(Ran {
    code,
    stdout: collect(stdout)?,
    stderr: collect(stderr)?,
  })
}
