//! Real-repository and process helpers for native statusline CLI tests.

use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Errors from a real subprocess or fixture file in these integration tests.
pub(crate) type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// The binary built for this integration test.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// Give direct and shim invocations the same isolated test environment.
pub(crate) fn isolated_env(command: &mut Command, home: &Path, cfg: &Path) {
  command
    .env_clear()
    .env("HOME", home)
    .env("CLAUDE_CONFIG_DIR", cfg)
    .env("TOOLU_HOST_OVERRIDE", "claude")
    .env("TYPESAFE_API_KEY", "test-key")
    .env(
      "TMPDIR",
      std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned()),
    );
  if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
}

/// Run `toolu` with an isolated home and Claude config root.
pub(crate) fn cmd(
  home: &Path,
  cfg: &Path,
  cwd: &Path,
  args: &[&str],
  input: Option<&str>,
) -> TestResult<Output> {
  let mut command = Command::new(TOOLU);
  let bin_dir = Path::new(TOOLU)
    .parent()
    .ok_or_else(|| std::io::Error::other("binary has no parent"))?;
  isolated_env(&mut command, home, cfg);
  command
    .args(args)
    .current_dir(cwd)
    .env("PATH", format!("{}:/usr/bin:/bin", bin_dir.display()));
  if let Some(text) = input {
    use std::io::Write as _;
    let mut child = command
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()?;
    let mut stdin = child
      .stdin
      .take()
      .ok_or_else(|| std::io::Error::other("toolu has no stdin"))?;
    // A hook that never reads its payload may exit first; that is not a failure.
    match stdin.write_all(text.as_bytes()) {
      Err(err) if err.kind() != std::io::ErrorKind::BrokenPipe => return Err(err.into()),
      Ok(()) | Err(_) => {}
    }
    drop(stdin);
    Ok(child.wait_with_output()?)
  } else {
    Ok(command.output()?)
  }
}

/// Decode the command's stdout; invalid UTF-8 will fail the expected text check.
pub(crate) fn text(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}
