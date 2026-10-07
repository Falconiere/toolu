//! The setup half of `run` (`packages/toolu-core/src/ledger/ledger-run-context.ts`):
//! the flags, the resolved run context (repository, base, ledger file, branch
//! hash, prior entries) and the failure that ends a command with exit 2 after
//! its tagged lines.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::base_branch;
use toolu_state::time::iso_seconds;

use super::io::{
  LedgerOptions, ReadLedger, head_branch, ledger_path, project_root, read_ledger, write_ledger,
};
use super::jq::{JqError, number, string};
use super::model::entries_by_id;
use super::parse::parse_steps;

/// A failure that ends the command with exit 2 after printing its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandFail(pub Vec<String>);

impl CommandFail {
  /// One line.
  pub fn line(line: impl Into<String>) -> CommandFail {
    CommandFail(vec![line.into()])
  }
}

/// A jq-shaped computation whose error becomes the caller's tagged failure.
///
/// # Errors
/// `message` when `result` is a [`JqError`].
pub fn or_fail<T>(result: Result<T, JqError>, message: &str) -> Result<T, CommandFail> {
  result.map_err(|_jq| CommandFail::line(message))
}

/// `run`'s flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunFlags {
  /// `--step <id>`: run only this step.
  pub only_step: Option<String>,
  /// `--activity <label>`: what the running step is doing.
  pub activity: Option<String>,
  /// `--force`: re-run fresh steps.
  pub force: bool,
  /// `--verify`: judge every step on the whole branch hash.
  pub verify: bool,
}

impl RunFlags {
  /// The flags' own rules, with TypeScript's messages.
  ///
  /// # Errors
  /// An empty `--step` or `--activity`, or `--activity` without `--step`.
  pub fn check(&self) -> Result<(), CommandFail> {
    if self.only_step.as_deref() == Some("") {
      return Err(CommandFail::line("plan-ledger: --step requires an id"));
    }
    if self.activity.as_deref() == Some("") {
      return Err(CommandFail::line(
        "plan-ledger: --activity requires a label",
      ));
    }
    if self.activity.is_some() && self.only_step.is_none() {
      return Err(CommandFail::line("plan-ledger: --activity requires --step"));
    }
    Ok(())
  }

  /// The `--step` id, or `""`.
  pub fn step(&self) -> &str {
    self.only_step.as_deref().unwrap_or_default()
  }

  /// The `--activity` label, or `""`.
  pub fn activity(&self) -> &str {
    self.activity.as_deref().unwrap_or_default()
  }
}

/// Everything one `run` reads.
#[derive(Debug, Clone)]
pub struct RunContext<'a> {
  /// The flags.
  pub flags: &'a RunFlags,
  /// The plan doc as given.
  pub doc: String,
  /// The parsed steps.
  pub steps: Vec<Ordered>,
  /// The base branch.
  pub base: String,
  /// The project root.
  pub root: PathBuf,
  /// The ledger file.
  pub ledger_file: PathBuf,
  /// The branch hash.
  pub cur: String,
  /// The branch.
  pub branch: String,
  /// The prior ledger.
  pub prior: Option<ReadLedger>,
  /// The prior entries by id.
  pub existing: HashMap<String, Ordered>,
  /// `{id: scope hash}` once computed.
  pub scope: Ordered,
  /// Where the command runs.
  pub opts: &'a LedgerOptions,
  /// The raw `PLAN_LEDGER_STEP_TIMEOUT`.
  pub timeout: String,
}

fn need<T>(value: Option<T>, line: String) -> Result<T, CommandFail> {
  value.ok_or(CommandFail(vec![line]))
}

/// `PUSH_REVIEW_BASE`, else origin's HEAD branch of `root`, else `main`.
pub fn base_for(opts: &LedgerOptions, root: Option<&PathBuf>) -> String {
  match (opts.env().get("PUSH_REVIEW_BASE"), root) {
    (Some(base), _) => base.to_owned(),
    (None, Some(root)) => base_branch(opts.env(), Some(root), &opts.cwd),
    (None, None) => "main".to_owned(),
  }
}

/// Resolve the run context of `doc`.
///
/// # Errors
/// The tagged lines TypeScript prints for a plan, repository, hash, branch or
/// prior-ledger failure.
pub fn prepare<'a>(
  doc: &str,
  flags: &'a RunFlags,
  opts: &'a LedgerOptions,
) -> Result<RunContext<'a>, CommandFail> {
  let steps = parse_steps(doc, &opts.cwd).map_err(|failure| CommandFail::line(failure.message))?;
  let found = project_root(opts);
  let base = base_for(opts, found.as_ref());
  let root = need(found, "plan-ledger: not in a git repo".to_owned())?;
  let ledger_file = need(
    ledger_path(opts),
    "plan-ledger: cannot resolve ledger path".to_owned(),
  )?;
  let cur = need(
    diff_sha(opts.env(), &root, &base),
    format!("plan-ledger: git diff {base}...HEAD failed"),
  )?;
  let branch = need(
    head_branch(opts.env(), &root),
    "plan-ledger: cannot resolve HEAD branch".to_owned(),
  )?;
  let prior = read_ledger(&ledger_file);
  let corrupt = format!(
    "plan-ledger: corrupt prior ledger at {}",
    ledger_file.display()
  );
  let non_empty = std::fs::metadata(&ledger_file).is_ok_and(|meta| meta.len() > 0);
  if prior.is_none() && non_empty {
    return Err(CommandFail::line(corrupt));
  }
  let existing = match &prior {
    Some(read) => or_fail(entries_by_id(&read.value), &corrupt)?,
    None => HashMap::new(),
  };
  let timeout = opts
    .env()
    .get("PLAN_LEDGER_STEP_TIMEOUT")
    .unwrap_or("1800")
    .to_owned();
  Ok(RunContext {
    flags,
    doc: doc.to_owned(),
    steps,
    base,
    root,
    ledger_file,
    cur,
    branch,
    prior,
    existing,
    scope: Ordered::Object(Vec::new()),
    opts,
    timeout,
  })
}

impl RunContext<'_> {
  /// The run's clock, in the ledger's format.
  pub fn now(&self) -> String {
    iso_seconds((self.opts.now)())
  }

  /// The current time as `SystemTime`.
  pub fn clock(&self) -> SystemTime {
    (self.opts.now)()
  }

  /// A ledger document with `steps`, stamped `updated_at`.
  pub fn ledger_doc(&self, updated_at: &str, steps: Vec<Ordered>) -> Ordered {
    Ordered::Object(vec![
      ("version".to_owned(), number(1.0)),
      ("branch".to_owned(), string(&self.branch)),
      ("base_branch".to_owned(), string(&self.base)),
      ("plan_doc".to_owned(), string(&self.doc)),
      ("updated_at".to_owned(), string(updated_at)),
      ("summary".to_owned(), Ordered::Object(Vec::new())),
      ("next".to_owned(), Ordered::Null),
      ("steps".to_owned(), Ordered::Array(steps)),
    ])
  }

  /// Write `ledger`, failing with the write's line and `failure`.
  ///
  /// # Errors
  /// The write's tagged line, then `failure`.
  pub fn write_or_fail(&self, ledger: &Ordered, failure: &str) -> Result<(), CommandFail> {
    write_ledger(&self.ledger_file, ledger)
      .map_err(|line| CommandFail(vec![line, failure.to_owned()]))
  }
}

#[cfg(test)]
#[path = "tests/context_test.rs"]
mod tests;

/// The repository the ledger's unit tests run in.
#[cfg(test)]
#[path = "tests/repo_test.rs"]
pub(crate) mod test_repo;
