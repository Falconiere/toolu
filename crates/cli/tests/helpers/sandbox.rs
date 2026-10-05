//! A temporary `HOME` and plugin root where the real generated launcher runs
//! with `sh -c`, a reduced `PATH`, and only the environment a case names.

use std::error::Error;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::RwLock;

use toolu_protocol::launcher::{DEFAULT_TIMEOUT, Target, hook};

/// A fallible helper result.
pub(crate) type Res<T> = Result<T, Box<dyn Error>>;

/// The `toolu` binary cargo built for these tests.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// This binary's version.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The reduced `PATH` a host may give its hooks: no install directory on it.
pub(crate) const REDUCED_PATH: &str = "/usr/bin:/bin";

/// The startup payload of a session.
pub(crate) const STARTUP: &str = r#"{"hook_event_name":"SessionStart","source":"startup"}"#;

/// The fixed install directories the launcher searches before `PATH`.
const FIXED_DIRS: &[&str] = &[
  "/opt/homebrew/bin",
  "/usr/local/bin",
  "/home/linuxbrew/.linuxbrew/bin",
];

/// Writers of executables hold it exclusively, spawners shared: a child forked
/// while an executable is open for writing would make its `exec` fail with
/// `ETXTBSY` (Linux), so the launcher would skip a valid candidate.
static EXECUTABLES: RwLock<()> = RwLock::new(());

/// Spawn `command` while no executable is being written.
pub(crate) fn spawn(command: &mut Command) -> Res<Child> {
  let _shared = EXECUTABLES.read().map_err(|err| err.to_string())?;
  Ok(command.spawn()?)
}

/// Copy `from` to `to` while no child is being spawned.
pub(crate) fn copy_executable(from: &Path, to: &Path) -> Res<()> {
  let _exclusive = EXECUTABLES.write().map_err(|err| err.to_string())?;
  std::fs::copy(from, to)?;
  Ok(())
}

/// Write an executable script at `path` while no child is being spawned.
pub(crate) fn script(path: &Path, text: &str) -> Res<()> {
  let _exclusive = EXECUTABLES.write().map_err(|err| err.to_string())?;
  std::fs::write(path, text)?;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

/// What one launcher run printed and how it exited.
#[derive(Debug)]
pub(crate) struct Run {
  pub(crate) code: i32,
  pub(crate) stdout: String,
  pub(crate) stderr: String,
}

/// A temporary home (`<dir>/a home`) and plugin root (`<dir>/plugin root`),
/// both with a space, so quoting is exercised on every run.
pub(crate) struct Sandbox {
  _dir: tempfile::TempDir,
  pub(crate) home: PathBuf,
  pub(crate) root: PathBuf,
}

impl Sandbox {
  /// A fresh sandbox whose plugin manifest matches this binary. Fails when a
  /// real `toolu` sits in a fixed install directory, since it would win.
  pub(crate) fn new() -> Res<Self> {
    if let Some(found) = FIXED_DIRS
      .iter()
      .map(|dir| Path::new(dir).join("toolu"))
      .find(|found| found.exists())
    {
      return Err(
        format!(
          "{} would shadow the test binary; move it aside or run in a container \
         (docs/install.md)",
          found.display()
        )
        .into(),
      );
    }
    let dir = tempfile::tempdir()?;
    let home = dir.path().join("a home");
    let root = dir.path().join("plugin root");
    std::fs::create_dir_all(home.join(".local/bin"))?;
    std::fs::create_dir_all(&root)?;
    let sandbox = Self {
      _dir: dir,
      home,
      root,
    };
    sandbox.manifest(VERSION, "1")?;
    Ok(sandbox)
  }

  /// Write `.claude-plugin/plugin.json` with `version` and the raw JSON `protocol`.
  pub(crate) fn manifest(&self, version: &str, protocol: &str) -> Res<()> {
    let dir = self.root.join(".claude-plugin");
    std::fs::create_dir_all(&dir)?;
    let text = format!(r#"{{"name":"toolu","version":"{version}","hookProtocol":{protocol}}}"#);
    std::fs::write(dir.join("plugin.json"), text)?;
    Ok(())
  }

  /// `~/.local/bin/toolu`, where the installer's `--install-dir ~/.local/bin` puts it.
  pub(crate) fn local_bin(&self) -> PathBuf {
    self.home.join(".local/bin/toolu")
  }

  /// Copy the built binary to `~/.local/bin/toolu` and return its canonical path.
  pub(crate) fn install(&self) -> Res<PathBuf> {
    copy_executable(Path::new(TOOLU), &self.local_bin())?;
    Ok(std::fs::canonicalize(self.local_bin())?)
  }

  /// Run the generated toolu launcher for `event` and `name` with `payload` on
  /// stdin. Only `HOME`, `PATH`, `CLAUDE_PLUGIN_ROOT`, `extra`, and the coverage
  /// profile variable reach it.
  pub(crate) fn launch(
    &self,
    event: &str,
    name: &str,
    payload: &str,
    extra: &[(&str, &str)],
  ) -> Res<Run> {
    let target = Target {
      plugin: "toolu",
      event,
      name,
    };
    let mut sh = Command::new("/bin/sh");
    sh.arg("-c")
      .arg(hook(&target, DEFAULT_TIMEOUT)?.command)
      .env_clear()
      .env("HOME", &self.home)
      .env("PATH", REDUCED_PATH)
      .env("CLAUDE_PLUGIN_ROOT", &self.root)
      .envs(extra.iter().copied())
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
      sh.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = spawn(&mut sh)?;
    let stdin = child.stdin.take().ok_or("no stdin pipe")?;
    // A launcher that fails closed may exit before it reads the payload.
    match { stdin }.write_all(payload.as_bytes()) {
      Err(err) if err.kind() != std::io::ErrorKind::BrokenPipe => return Err(err.into()),
      Ok(()) | Err(_) => {}
    }
    let output = child.wait_with_output()?;
    Ok(Run {
      code: output.status.code().unwrap_or(-1),
      stdout: String::from_utf8(output.stdout)?,
      stderr: String::from_utf8(output.stderr)?,
    })
  }
}

/// The `systemMessage` of a run's single stdout JSON line.
pub(crate) fn system_message(run: &Run) -> Res<String> {
  if run.stdout.lines().count() != 1 {
    return Err(format!("expected one stdout line: {run:?}").into());
  }
  let json: serde_json::Value = serde_json::from_str(&run.stdout)?;
  let message = json
    .get("systemMessage")
    .and_then(serde_json::Value::as_str)
    .ok_or("no systemMessage")?;
  Ok(message.to_owned())
}

/// The first `program` on this test process's `PATH`; a missing one fails the test.
pub(crate) fn on_path(program: &str) -> Res<PathBuf> {
  let path = std::env::var_os("PATH").unwrap_or_default();
  std::env::split_paths(&path)
    .map(|dir| dir.join(program))
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| format!("{program} is not on PATH; these tests need it").into())
}
