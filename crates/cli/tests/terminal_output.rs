//! At a terminal, `toolu status` and `toolu plugins list` draw rails, in color
//! unless `NO_COLOR` is set: the built binary writes to a real pty.

use std::fs::File;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Stdio};

use nix::pty::openpty;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// What `toolu <args>` writes to a terminal for stdout, run in `dir` with
/// `home/bin` first on `PATH`.
fn on_terminal(dir: &Path, home: &Path, args: &[&str], no_color: bool) -> Res<String> {
  let pty = openpty(None, None)?;
  let slave = File::from(pty.slave);
  let mut master = File::from(pty.master);
  let mut command = Command::new(TOOLU);
  command
    .args(args)
    .current_dir(dir)
    .env("HOME", home)
    .env(
      "PATH",
      format!("{}:/usr/bin:/bin", home.join("bin").display()),
    )
    .env("TERM", "xterm-256color")
    .env_remove("NO_COLOR")
    .env_remove("CLAUDE_PROJECT_DIR")
    .env_remove("TOOLU_PROJECT_DIR")
    .stdin(Stdio::null())
    .stdout(Stdio::from(slave.try_clone()?))
    .stderr(Stdio::piped());
  if no_color {
    command.env("NO_COLOR", "1");
  }
  let child = command.spawn()?;
  // The command keeps its copy of the slave; both must close for the read to end.
  drop(command);
  drop(slave);
  // The pty buffer is small; read it while the child writes, until it closes.
  let reader = std::thread::spawn(move || {
    let mut screen = Vec::new();
    let mut chunk = [0_u8; 4096];
    while let Ok(read @ 1..) = master.read(&mut chunk) {
      screen.extend_from_slice(chunk.get(..read).unwrap_or_default());
    }
    screen
  });
  let output = child.wait_with_output()?;
  if !output.status.success() {
    return Err(
      format!(
        "toolu {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
      )
      .into(),
    );
  }
  let screen = reader.join().map_err(|_panic| "the pty reader panicked")?;
  Ok(String::from_utf8_lossy(&screen).replace('\r', ""))
}

/// `shown` without its ANSI escapes.
fn bare(shown: &str) -> String {
  regex::Regex::new("\x1b\\[[0-9;]*m")
    .map(|escape| escape.replace_all(shown, "").into_owned())
    .unwrap_or_default()
}

fn repository(root: &Path) -> Res<std::path::PathBuf> {
  let repo = root.join("repo");
  std::fs::create_dir_all(&repo)?;
  for args in [
    &["init", "-q", "-b", "main"][..],
    &["commit", "-q", "--allow-empty", "-m", "init"][..],
  ] {
    let status = Command::new("git")
      .args([
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "-c",
        "commit.gpgsign=false",
      ])
      .args(args)
      .current_dir(&repo)
      .env_remove("GIT_DIR")
      .env_remove("GIT_WORK_TREE")
      .status()?;
    if !status.success() {
      return Err(format!("git {}", args.join(" ")).into());
    }
  }
  Ok(repo)
}

#[test]
fn status_at_a_terminal_is_a_colored_table_and_no_color_keeps_the_rails() {
  let root = tempfile::tempdir().unwrap();
  let repo = repository(root.path()).unwrap();
  let shown = on_terminal(&repo, root.path(), &["--host", "claude", "status"], false).unwrap();
  let table = bare(&shown);
  assert!(table.contains("┌  toolu status\n"), "{shown:?}");
  assert!(
    table.contains("│  branch        main  ↑0 ↓0\n"),
    "{shown:?}"
  );
  assert!(shown.contains("\x1b[2m"), "no color in {shown:?}");
  let plain = on_terminal(&repo, root.path(), &["--host", "claude", "status"], true).unwrap();
  assert!(plain.contains("│  host          claude\n"), "{plain:?}");
  assert!(!plain.contains('\x1b'), "NO_COLOR still painted: {plain:?}");
}

#[test]
fn a_plugin_list_at_a_terminal_draws_rails_with_colored_glyphs() {
  let home = tempfile::tempdir().unwrap();
  let bin = home.path().join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let stub = bin.join("claude");
  std::fs::write(
    &stub,
    "#!/bin/sh\ncase \"$*\" in\n\"plugin list --json\") echo '[{\"id\":\"jev@toolu\",\"version\":\"8.5.0\"}]' ;;\nesac\n",
  )
  .unwrap();
  std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
  let shown = on_terminal(
    home.path(),
    home.path(),
    &["plugins", "list", "--host", "claude"],
    false,
  )
  .unwrap();
  assert!(bare(&shown).contains("◇  claude\n"), "{shown:?}");
  assert!(shown.contains("\x1b[32m✔\x1b[0m jev"), "{shown:?}");
}
