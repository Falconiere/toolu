//! `cargo xtask launcher-e2e --bin <dir>/toolu [--home DIR]`: the real generated
//! launcher finds a `toolu` installed in a fixed directory under a `PATH` that
//! lacks it, reports it at `SessionStart`, and fails closed on a protocol mismatch
//! (#412). CI runs it at the Homebrew directory and `/usr/local/bin`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use toolu_protocol::launcher::{DEFAULT_TIMEOUT, Target, hook};

use crate::options::Options;
use crate::{Verdict, output};

/// The directories the launcher searches before `PATH`, `~/.local/bin` last.
const SYSTEM_DIRS: &[&str] = &[
  "/opt/homebrew/bin",
  "/usr/local/bin",
  "/home/linuxbrew/.linuxbrew/bin",
];

const STARTUP: &str = r#"{"hook_event_name":"SessionStart","source":"startup"}"#;

/// Run the end-to-end check for `--bin`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let bin = options
    .bin
    .as_deref()
    .ok_or("launcher-e2e needs --bin <dir>/toolu")?;
  let home = match &options.home {
    Some(home) => home.clone(),
    None => std::env::var_os("HOME")
      .map(PathBuf::from)
      .ok_or("HOME is not set")?,
  };
  require_installed(bin, &home)?;
  let version = probe(bin, "--version")?;
  let version = version
    .strip_prefix("toolu ")
    .ok_or("--version is not `toolu <version>`")?;
  let protocol: u32 = probe(bin, "--hook-protocol")?
    .parse()
    .map_err(|err| format!("--hook-protocol is not an integer: {err}"))?;
  let root = std::env::temp_dir().join(format!("toolu-launcher-e2e-{}", std::process::id()));
  let found = check(bin, &home, &root, version, protocol);
  std::fs::remove_dir_all(&root).ok();
  Ok(output::findings("launcher-e2e", &found?))
}

/// The `SessionStart` and protocol-mismatch findings for an installed `bin`.
fn check(
  bin: &Path,
  home: &Path,
  root: &Path,
  version: &str,
  protocol: u32,
) -> Result<Vec<String>, String> {
  let canonical = std::fs::canonicalize(bin).map_err(|err| err.to_string())?;
  let mut found = Vec::new();
  manifest(root, version, protocol)?;
  let start = launch(home, root, "SessionStart")?;
  let expected = format!(
    "{{\"systemMessage\":\"toolu runtime: native {version} at {}\"}}",
    canonical.display()
  );
  if start.status.code() != Some(0) || text(&start.stdout).trim_end() != expected {
    found.push(format!(
      "SessionStart: expected exit 0 and {expected}, got {:?}: {}{}",
      start.status.code(),
      text(&start.stdout),
      text(&start.stderr)
    ));
  }
  manifest(root, version, protocol + 1)?;
  let pre = launch(home, root, "PreToolUse")?;
  let refusal = format!(
    "hook protocol {} needs a newer toolu - toolu {version} speaks protocol {protocol}",
    protocol + 1
  );
  let stderr = text(&pre.stderr);
  if pre.status.code() != Some(2) || !stderr.starts_with("blocked: ") || !stderr.contains(&refusal)
  {
    found.push(format!(
      "PreToolUse with hookProtocol {}: expected exit 2 and `blocked: … {refusal}`, got {:?}: {}",
      protocol + 1,
      pre.status.code(),
      text(&pre.stderr)
    ));
  }
  Ok(found)
}

/// `bin` must be `toolu` directly inside a fixed directory, with no `toolu` in
/// an earlier one: that one would win.
fn require_installed(bin: &Path, home: &Path) -> Result<(), String> {
  let local = home.join(".local/bin");
  let mut dirs: Vec<PathBuf> = SYSTEM_DIRS.iter().map(PathBuf::from).collect();
  dirs.push(local);
  let parent = bin.parent().unwrap_or(Path::new(""));
  let at = dirs
    .iter()
    .position(|dir| dir == parent)
    .filter(|_| bin.file_name().is_some_and(|name| name == "toolu"))
    .ok_or_else(|| {
      format!(
        "{} is not toolu in a fixed install directory",
        bin.display()
      )
    })?;
  if !bin.is_file() {
    return Err(format!("{} does not exist", bin.display()));
  }
  let earlier = dirs.iter().take(at).map(|dir| dir.join("toolu"));
  if let Some(shadow) = earlier.into_iter().find(|candidate| candidate.exists()) {
    return Err(format!(
      "{} would win over {}",
      shadow.display(),
      bin.display()
    ));
  }
  Ok(())
}

fn probe(bin: &Path, flag: &str) -> Result<String, String> {
  let output = Command::new(bin)
    .arg(flag)
    .stdin(Stdio::null())
    .output()
    .map_err(|err| format!("cannot run {} {flag}: {err}", bin.display()))?;
  Ok(text(&output.stdout).trim().to_owned())
}

fn manifest(root: &Path, version: &str, protocol: u32) -> Result<(), String> {
  let dir = root.join(".claude-plugin");
  std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
  let json = format!(r#"{{"name":"toolu","version":"{version}","hookProtocol":{protocol}}}"#);
  std::fs::write(dir.join("plugin.json"), json).map_err(|err| err.to_string())
}

/// The generated toolu launcher for `event`, run as a host would, with only
/// `HOME`, `PATH=/usr/bin:/bin` and `CLAUDE_PLUGIN_ROOT`.
fn launch(home: &Path, root: &Path, event: &str) -> Result<Output, String> {
  let target = Target {
    plugin: "toolu",
    event,
    name: "session-start",
  };
  let command = hook(&target, DEFAULT_TIMEOUT)?.command;
  let mut child = Command::new("/bin/sh")
    .arg("-c")
    .arg(command)
    .env_clear()
    .env("HOME", home)
    .env("PATH", "/usr/bin:/bin")
    .env("CLAUDE_PLUGIN_ROOT", root)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| format!("cannot run sh: {err}"))?;
  if let Some(mut stdin) = child.stdin.take() {
    // A launcher that fails closed may exit before it reads the payload.
    stdin.write_all(STARTUP.as_bytes()).ok();
  }
  child.wait_with_output().map_err(|err| err.to_string())
}

fn text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
#[path = "tests/launcher_e2e_test.rs"]
mod tests;
