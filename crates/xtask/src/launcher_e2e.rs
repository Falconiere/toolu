//! `cargo xtask launcher-e2e --bin <dir>/toolu [--home DIR]`: the real generated
//! launcher finds a `toolu` installed in a fixed directory under a `PATH` that
//! lacks it, reports it at `SessionStart`, and fails closed on a protocol mismatch
//! (#412). The generated Muse resolver, run as Muse runs it (`/bin/sh
//! hooks/muse/<name>` with `MUSE_PLUGIN_ROOT`, #552), must do the same. CI runs it
//! at the Homebrew directory and `/usr/local/bin`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use toolu_protocol::launcher::muse::{DEFAULT_TIMEOUT_MS, hook as muse_hook};
use toolu_protocol::launcher::{DEFAULT_TIMEOUT, Target, hook};

use crate::options::Options;
use crate::{Verdict, output};

/// The directories the launcher searches before `PATH`, `~/.local/bin` last.
pub(crate) const SYSTEM_DIRS: &[&str] = &[
  "/opt/homebrew/bin",
  "/usr/local/bin",
  "/home/linuxbrew/.linuxbrew/bin",
];

const STARTUP: &str = r#"{"hook_event_name":"SessionStart","source":"startup"}"#;
static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

/// Which generated launcher runs the hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
  /// The `hooks.json` command under `/bin/sh -c`.
  Claude,
  /// The Muse resolver file under `/bin/sh`.
  Muse,
}

/// Run the end-to-end check for `--bin`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  run_inner(options, false)
}

fn run_inner(options: &Options, override_bin: bool) -> Result<Verdict, String> {
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
  if !override_bin {
    require_installed(bin, &home)?;
  }
  let version = probe(bin, "--version")?;
  let version = version
    .strip_prefix("toolu ")
    .ok_or("--version is not `toolu <version>`")?;
  let protocol: u32 = probe(bin, "--hook-protocol")?
    .parse()
    .map_err(|err| format!("--hook-protocol is not an integer: {err}"))?;
  let root = std::env::temp_dir().join(format!(
    "toolu-launcher-e2e-{}-{}",
    std::process::id(),
    NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
  ));
  let found = check(
    bin,
    Run {
      home: &home,
      root: &root,
      version,
      protocol,
      form: Form::Claude,
      bin: override_bin.then_some(bin),
    },
  );
  std::fs::remove_dir_all(&root).ok();
  Ok(output::findings("launcher-e2e", &found?))
}

/// The `SessionStart` and protocol-mismatch findings for an installed `bin`, for
/// the Claude launcher, then the Muse resolver (prefixed `Muse resolver: `).
fn check(bin: &Path, base: Run<'_>) -> Result<Vec<String>, String> {
  let canonical = std::fs::canonicalize(bin).map_err(|err| err.to_string())?;
  let mut found = Vec::new();
  for form in [Form::Claude, Form::Muse] {
    let run = Run { form, ..base };
    let form_found = check_form(&run, &canonical)?;
    found.extend(form_found.into_iter().map(|finding| match form {
      Form::Claude => finding,
      Form::Muse => format!("Muse resolver: {finding}"),
    }));
  }
  Ok(found)
}

/// One launcher form's run.
#[derive(Clone, Copy)]
struct Run<'r> {
  home: &'r Path,
  root: &'r Path,
  version: &'r str,
  protocol: u32,
  form: Form,
  bin: Option<&'r Path>,
}

fn check_form(run: &Run<'_>, canonical: &Path) -> Result<Vec<String>, String> {
  let (root, version, protocol) = (run.root, run.version, run.protocol);
  let mut found = Vec::new();
  manifest(root, version, protocol)?;
  let start = launch(run, "SessionStart")?;
  if let Some(finding) = session_finding(&start, version, canonical) {
    found.push(finding);
  }
  manifest(root, version, protocol + 1)?;
  let pre = launch(run, "PreToolUse")?;
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

/// The generated toolu launcher for `event`, run as a host would. `HOME` stays
/// the install home so `~/.local/bin` resolution works. The working directory,
/// the project variable and `TOOLU_CONFIG_DIR` stay on `root`, so session-start
/// cannot write the caller's project or config. Muse forwards no `TOOLU_CONFIG_DIR`;
/// the harness sets it for the same reason.
fn launch(run: &Run<'_>, event: &str) -> Result<Output, String> {
  let mut child = launch_command(run, event)?
    .spawn()
    .map_err(|err| format!("cannot run sh: {err}"))?;
  if let Some(mut stdin) = child.stdin.take() {
    // A launcher that fails closed may exit before it reads the payload.
    stdin.write_all(STARTUP.as_bytes()).ok();
  }
  child.wait_with_output().map_err(|err| err.to_string())
}

fn launch_command(run: &Run<'_>, event: &str) -> Result<Command, String> {
  let root = run.root;
  let config = root.join("config");
  std::fs::create_dir_all(&config).map_err(|err| err.to_string())?;
  let target = Target {
    plugin: "toolu",
    event,
    name: "session-start",
  };
  let mut command = Command::new("/bin/sh");
  command.env_clear();
  if let Some(bin) = run.bin {
    command.env("TOOLU_BIN", bin);
  }
  match run.form {
    Form::Claude => {
      command
        .arg("-c")
        .arg(hook(&target, DEFAULT_TIMEOUT)?.command)
        .env("CLAUDE_PLUGIN_ROOT", root)
        .env("CLAUDE_PROJECT_DIR", root);
    }
    Form::Muse => {
      let generated = muse_hook(&target, DEFAULT_TIMEOUT_MS)?;
      let resolver = root.join(&generated.resolver_path);
      let dir = resolver.parent().ok_or("the resolver has no directory")?;
      std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
      std::fs::write(&resolver, generated.resolver).map_err(|err| err.to_string())?;
      command
        .arg(&resolver)
        .env("MUSE_PLUGIN_ID", "toolu")
        .env("MUSE_PLUGIN_ROOT", root)
        .env("CLAUDE_PLUGIN_ROOT", root)
        .env("PLUGIN_ROOT", root);
    }
  }
  command
    .env("HOME", run.home)
    .env("PATH", "/usr/bin:/bin")
    .env("TOOLU_CONFIG_DIR", &config)
    .current_dir(root)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  Ok(command)
}

fn text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

/// `SessionStart` must exit 0 with a `systemMessage` that starts `Toolu is on!`
/// and names this binary on the native runtime line.
fn session_finding(start: &Output, version: &str, canonical: &Path) -> Option<String> {
  let runtime = format!("toolu runtime: native {version} at {}", canonical.display());
  let message = system_message(&text(&start.stdout));
  let started = start.status.code() == Some(0)
    && message
      .as_ref()
      .is_ok_and(|body| body.starts_with("Toolu is on!") && body.contains(&runtime));
  if started {
    return None;
  }
  Some(format!(
    "SessionStart: expected exit 0 and a systemMessage starting with \"Toolu is on!\" \
     and containing {runtime}, got {:?}: {}{}",
    start.status.code(),
    text(&start.stdout),
    text(&start.stderr)
  ))
}

fn system_message(stdout: &str) -> Result<String, String> {
  let json: serde_json::Value =
    serde_json::from_str(stdout.trim()).map_err(|err| err.to_string())?;
  json
    .get("systemMessage")
    .and_then(serde_json::Value::as_str)
    .map(ToOwned::to_owned)
    .ok_or_else(|| "no systemMessage".to_owned())
}

#[cfg(test)]
#[path = "tests/launcher_e2e_test.rs"]
mod tests;
