//! Real subprocesses with an explicit environment, and the real git repositories the cases need.

use std::io::{ErrorKind, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::golden_statusline_sandbox::Sandbox;
use crate::golden_statusline_value::TestResult;
use crate::golden_statusline_value::bad;

/// Environment overrides in order; `None` unsets the key in the child.
pub(crate) type EnvPatch = Vec<(String, Option<String>)>;

/// One process to run.
pub(crate) struct Spec<'a> {
  pub(crate) program: &'a str,
  pub(crate) args: Vec<String>,
  pub(crate) cwd: &'a Path,
  pub(crate) env: EnvPatch,
  pub(crate) stdin: &'a str,
}

/// What a process printed and how it ended.
pub(crate) struct Ran {
  pub(crate) code: Option<i32>,
  pub(crate) stdout: String,
  pub(crate) stderr: String,
}

/// Host-session variables, git redirections and real credentials never reach a child.
fn withheld(key: &str) -> bool {
  ["CLAUDE_", "CODEX_", "TOOLU_", "CURSOR_", "GIT_", "HERMES_"]
    .iter()
    .any(|prefix| key.starts_with(prefix))
    || ["CLAUDECODE", "PLUGIN_ROOT", "TYPESAFE_API_KEY"].contains(&key)
}

/// Owned strings for a literal argument list.
pub(crate) fn strs(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

/// Set `key` in a patch, replacing an earlier entry.
pub(crate) fn set(env: &mut EnvPatch, key: &str, value: Option<String>) {
  env.retain(|(name, _)| name != key);
  env.push((key.to_owned(), value));
}

/// Run the process, feed it `stdin`, and capture its streams.
pub(crate) fn exec(spec: &Spec<'_>) -> TestResult<Ran> {
  let mut command = Command::new(spec.program);
  command.args(&spec.args).current_dir(spec.cwd).env_clear();
  for (key, value) in std::env::vars().filter(|(key, _)| !withheld(key)) {
    command.env(key, value);
  }
  for (key, value) in &spec.env {
    match value {
      Some(value) => command.env(key, value),
      None => command.env_remove(key),
    };
  }
  let mut child = command
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  let mut stdin = child
    .stdin
    .take()
    .ok_or_else(|| bad("child has no stdin"))?;
  match stdin.write_all(spec.stdin.as_bytes()) {
    Err(error) if error.kind() != ErrorKind::BrokenPipe => return Err(error.into()),
    Ok(()) | Err(_) => {}
  }
  drop(stdin);
  let out = child.wait_with_output()?;
  Ok(Ran {
    code: out.status.code(),
    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
    stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
  })
}

/// `git -C cwd <args>` with a throwaway identity and the sandbox HOME; a failure is an error.
pub(crate) fn git(sb: &Sandbox, cwd: &Path, args: &[String]) -> TestResult<()> {
  let mut full = strs(&[
    "-C",
    &cwd.to_string_lossy(),
    "-c",
    "user.email=t@t",
    "-c",
    "user.name=t",
    "-c",
    "commit.gpgsign=false",
  ]);
  full.extend(args.iter().cloned());
  let ran = exec(&Spec {
    program: "git",
    args: full,
    cwd: &sb.root,
    env: vec![(
      "HOME".to_owned(),
      Some(sb.home.to_string_lossy().into_owned()),
    )],
    stdin: "",
  })?;
  if ran.code == Some(0) {
    return Ok(());
  }
  Err(bad(format!("git {}: {}", args.join(" "), ran.stderr)))
}

/// A repository with one empty commit on `branch`.
pub(crate) fn repo(sb: &Sandbox, dir: &Path, branch: &str) -> TestResult<()> {
  std::fs::create_dir_all(dir)?;
  git(sb, dir, &strs(&["init", "-q", "-b", branch]))?;
  git(
    sb,
    dir,
    &strs(&["commit", "-q", "--allow-empty", "-m", "init"]),
  )
}

/// `<root>/repo` on `branch`, pushed to and tracking a bare `<root>/remote.git`.
pub(crate) fn with_remote(sb: &Sandbox, branch: &str) -> TestResult<()> {
  let remote = sb.root.join("remote.git");
  let dir = sb.root.join("repo");
  std::fs::create_dir_all(&remote)?;
  git(sb, &remote, &strs(&["init", "-q", "--bare"]))?;
  repo(sb, &dir, branch)?;
  git(
    sb,
    &dir,
    &strs(&["remote", "add", "origin", &remote.to_string_lossy()]),
  )?;
  git(sb, &dir, &strs(&["push", "-q", "-u", "origin", branch]))
}
