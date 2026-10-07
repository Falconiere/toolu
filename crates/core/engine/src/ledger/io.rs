//! Ledger file I/O and location (`packages/toolu-core/src/ledger/ledger-io.ts`):
//! reading and writing the per-branch ledger, where it lives, and the git
//! questions that decide that. A write produces `jq .` bytes through a
//! `<file>.tmp.<pid>` stage and a rename, so the file keeps the umask mode a
//! shell redirect gives it.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, run};
use toolu_state::git::branch_slug;

use super::jq::parse_json;

/// Where a command runs and what it can see.
#[derive(Debug, Clone)]
pub struct LedgerOptions {
  /// The environment and host.
  pub roots: Roots,
  /// The directory the command was started in.
  pub cwd: PathBuf,
  /// The one clock for every timestamp and the orphan cutoff.
  pub now: fn() -> SystemTime,
}

impl LedgerOptions {
  /// The environment.
  pub fn env(&self) -> &Env {
    self.roots.env()
  }
}

/// A ledger as read: the parsed document, and its text as `$(cat)` captured it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadLedger {
  /// The document.
  pub value: Ordered,
  /// The file's text without its trailing newlines.
  pub text: String,
}

/// `pl_read_ledger FILE`: `None` for an absent or empty file, text that is not
/// JSON, or a document `jq -e .` rejects (`null`, `false`).
pub fn read_ledger(file: &Path) -> Option<ReadLedger> {
  if std::fs::metadata(file).ok()?.len() == 0 {
    return None;
  }
  let bytes = std::fs::read(file).ok()?;
  let text = String::from_utf8_lossy(&bytes).into_owned();
  let value = parse_json(&text)?;
  if matches!(value, Ordered::Null | Ordered::Bool(false)) {
    return None;
  }
  Some(ReadLedger {
    value,
    text: text.trim_end_matches('\n').to_owned(),
  })
}

/// `pl_write_ledger FILE JSON`.
///
/// # Errors
/// The tagged line bash printed when the directory, the stage or the rename fails.
pub fn write_ledger(file: &Path, ledger: &Ordered) -> Result<(), String> {
  let dir = file.parent().unwrap_or_else(|| Path::new("."));
  std::fs::create_dir_all(dir).ok().ok_or_else(|| {
    format!(
      "plan-ledger-parse: cannot create ledger dir: {}",
      dir.display()
    )
  })?;
  let mut tmp = file.as_os_str().to_owned();
  tmp.push(format!(".tmp.{}", std::process::id()));
  let tmp = PathBuf::from(tmp);
  let body = format!("{}\n", jq_text(ledger, true));
  if std::fs::write(&tmp, body).is_err() {
    let _removed = std::fs::remove_file(&tmp);
    return Err(format!(
      "plan-ledger-parse: failed to stage ledger to {}",
      tmp.display()
    ));
  }
  if std::fs::rename(&tmp, file).is_err() {
    let _removed = std::fs::remove_file(&tmp);
    return Err(format!(
      "plan-ledger-parse: atomic mv failed for {}",
      file.display()
    ));
  }
  Ok(())
}

/// `detect_project_root`: the git toplevel of the invocation directory.
pub fn project_root(opts: &LedgerOptions) -> Option<PathBuf> {
  toolu_runtime::git::toplevel(opts.env(), &opts.cwd)
}

/// `git rev-parse --abbrev-ref HEAD` in `cwd`, or `None` when it exits non-zero
/// (outside a repository, or an unborn HEAD).
pub fn head_branch(env: &Env, cwd: &Path) -> Option<String> {
  let mut spec = Spec::new(["git", "rev-parse", "--abbrev-ref", "HEAD"]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  let output = run(&spec).ok().filter(|output| output.exit_code == 0)?;
  Some(output.stdout.trim_end_matches('\n').to_owned())
}

/// `toolu_project_state_root ROOT`: `<root>/<host dir>/tmp`.
pub fn state_root_for(root: &Path, roots: &Roots) -> PathBuf {
  roots
    .project_state_root(None, Some(root))
    .unwrap_or_else(|| root.join("tmp"))
}

/// `toolu_project_state_dir NAME ROOT`.
pub fn state_dir_for(name: &str, root: &Path, roots: &Roots) -> PathBuf {
  state_root_for(root, roots).join(name)
}

/// `pl_ledger_path`: `<root>/<host dir>/tmp/plan-ledger/<branch slug>.json`.
pub fn ledger_path(opts: &LedgerOptions) -> Option<PathBuf> {
  let root = project_root(opts)?;
  let branch = head_branch(opts.env(), &opts.cwd)?;
  Some(
    state_dir_for("plan-ledger", &root, &opts.roots).join(format!("{}.json", branch_slug(&branch))),
  )
}

/// A command's result: exactly the text the TypeScript CLI prints, its exit
/// code, and the ledger it wrote (for `--json`).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
  /// 0 all fresh or done, 1 not all fresh or refused, 2 an error.
  pub exit: u8,
  /// Standard output.
  pub stdout: String,
  /// Standard error, one line per diagnostic, each ending in a newline.
  pub stderr: String,
  /// The ledger written, when the command wrote one.
  pub ledger: Option<Ordered>,
}

/// A command's output as it accumulates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
  stdout: String,
  stderr: Vec<String>,
}

impl Output {
  /// Appends `text` to standard output.
  pub fn stdout(&mut self, text: &str) {
    self.stdout.push_str(text);
  }

  /// Appends one diagnostic line.
  pub fn stderr(&mut self, line: impl Into<String>) {
    self.stderr.push(line.into());
  }

  /// The result with `exit`.
  pub fn result(self, exit: u8, ledger: Option<Ordered>) -> CommandResult {
    let mut stderr = String::new();
    for line in &self.stderr {
      stderr.push_str(line);
      stderr.push('\n');
    }
    CommandResult {
      exit,
      stdout: self.stdout,
      stderr,
      ledger,
    }
  }
}

#[cfg(test)]
#[path = "tests/io_test.rs"]
mod tests;
