//! The other ledger commands (`packages/toolu-core/src/ledger/ledger-commands.ts`):
//! `status` (heal, recompute and rewrite without running checks, then the AC
//! report), `path`, `root`, `self-test`, and the `git is required` check every
//! command makes first.

use std::path::{Path, PathBuf};

use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{branch_slug, has_git};
use toolu_state::lock::token;

use super::context::{CommandFail, base_for, or_fail};
use super::coverage::{CoverageReport, ac_coverage};
use super::io::{
  CommandResult, LedgerOptions, Output, head_branch, ledger_path, project_root, read_ledger,
  write_ledger,
};
use super::jq::{NULL, alt, get, raw, string};
use super::model::{all_fresh, heal_orphans, orphan_cutoff, recompute, summary_line};
use super::parse::{doc_field, is_file, is_specless, parse_steps, resolve};

const DEFAULT_STUCK_SECONDS: i64 = 300;

/// A one-line exit-2 result.
pub fn fail(line: &str) -> CommandResult {
  let mut out = Output::default();
  out.stderr(line);
  out.result(2, None)
}

/// `plan-ledger: git is required` when `git --version` does not run.
pub fn require_git(opts: &LedgerOptions) -> Option<CommandResult> {
  (!has_git(opts.env())).then(|| fail("plan-ledger: git is required"))
}

/// `PL_STUCK_THRESHOLD` (seconds, default 300): how old a `running` step must
/// be to count as orphaned. Only `-?\d+` is read; anything else is the default.
fn stuck_threshold(opts: &LedgerOptions) -> i64 {
  let Some(value) = opts.env().get("PL_STUCK_THRESHOLD") else {
    return DEFAULT_STUCK_SECONDS;
  };
  let digits = value.strip_prefix('-').unwrap_or(value);
  if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
    return DEFAULT_STUCK_SECONDS;
  }
  value.parse().unwrap_or(if value.starts_with('-') {
    i64::MIN
  } else {
    i64::MAX
  })
}

/// The orphan cutoff for this command.
pub fn cutoff(opts: &LedgerOptions) -> String {
  orphan_cutoff((opts.now)(), stuck_threshold(opts))
}

/// Node's `path.join(root, path)`: `path` appended to `root` even when absolute.
pub fn node_join(root: &Path, path: &str) -> PathBuf {
  resolve(root, path.trim_start_matches('/'))
}

/// bash `[ -f P ] || { [ -n "$root" ] && [ -f "$root/P" ] && P="$root/P"; }`.
fn file_or_under_root(path: &str, cwd: &Path, root: Option<&Path>) -> Option<PathBuf> {
  let near = resolve(cwd, path);
  if is_file(&near) {
    return Some(near);
  }
  root
    .map(|root| node_join(root, path))
    .filter(|under| is_file(under))
}

/// The report-only AC coverage of `status`, resolved from the ledger's plan doc.
fn coverage_report(ledger: &Ordered, cur: &str, opts: &LedgerOptions) -> CoverageReport {
  let empty = string("");
  let plan_doc = raw(alt(get(ledger, "plan_doc").unwrap_or(&NULL), &empty));
  if plan_doc.is_empty() {
    return CoverageReport::default();
  }
  let root = project_root(opts);
  let Some(plan) = file_or_under_root(&plan_doc, &opts.cwd, root.as_deref()) else {
    return CoverageReport::default();
  };
  let spec_field = doc_field(&plan, "Spec");
  let spec = if is_specless(&spec_field) {
    spec_field
  } else {
    file_or_under_root(&spec_field, &opts.cwd, root.as_deref())
      .unwrap_or_else(|| resolve(&opts.cwd, &spec_field))
      .display()
      .to_string()
  };
  ac_coverage(ledger, cur, &spec)
}

fn status_inner(
  opts: &LedgerOptions,
  out: &mut Output,
) -> Result<(Ordered, CoverageReport), CommandFail> {
  let base = base_for(opts, project_root(opts).as_ref());
  let file = ledger_path(opts)
    .ok_or_else(|| CommandFail::line("plan-ledger: cannot resolve ledger path"))?;
  let read = read_ledger(&file)
    .ok_or_else(|| CommandFail::line(format!("plan-ledger: no ledger at {}", file.display())))?;
  let cur = diff_sha(opts.env(), &opts.cwd, &base)
    .ok_or_else(|| CommandFail::line(format!("plan-ledger: git diff {base}...HEAD failed")))?;
  let branch = head_branch(opts.env(), &opts.cwd)
    .ok_or_else(|| CommandFail::line("plan-ledger: cannot resolve HEAD branch"))?;
  let healed = or_fail(
    heal_orphans(&read.value, &cutoff(opts)),
    "plan-ledger: failed to heal orphaned running steps",
  )?;
  let empty = Ordered::Object(Vec::new());
  let ledger = or_fail(
    recompute(&healed, &cur, &empty, false),
    "plan-ledger: failed to recompute summary",
  )?;
  write_ledger(&file, &ledger)
    .map_err(|line| CommandFail(vec![line, "plan-ledger: ledger write failed".to_owned()]))?;
  if let Ok(line) = summary_line(&ledger, &branch_slug(&branch)) {
    out.stdout(&format!("{line}\n"));
  }
  let report = coverage_report(&ledger, &cur, opts);
  out.stdout(&report.stdout);
  for line in &report.stderr {
    out.stderr(line.clone());
  }
  Ok((ledger, report))
}

/// `toolu ledger status`: heal, recompute and rewrite the ledger without
/// running checks; the coverage rows ride along for `--json`.
pub fn ledger_status(opts: &LedgerOptions) -> (CommandResult, CoverageReport) {
  let mut out = Output::default();
  match status_inner(opts, &mut out) {
    Ok((ledger, report)) => {
      let exit = u8::from(!all_fresh(&ledger));
      (out.result(exit, Some(ledger)), report)
    }
    Err(CommandFail(lines)) => {
      for line in lines {
        out.stderr(line);
      }
      (out.result(2, None), CoverageReport::default())
    }
  }
}

const SELF_TEST_DOC: &str = "# Self-test Plan\n\n## Steps (machine-readable)\n\n```json\n[\n  { \"id\": \"s1\", \"title\": \"ok\", \"check\": \"true\" },\n  { \"id\": \"s2\", \"title\": \"fail\", \"check\": \"false\" }\n]\n```\n";

/// `toolu ledger self-test`: parse a two-step fixture and check the result.
pub fn ledger_self_test() -> CommandResult {
  let mut out = Output::default();
  let dir = std::env::temp_dir().join(format!("plan-ledger-self-test-{}", token()));
  let doc = dir.join("selftest-plan.md");
  let written = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&doc, SELF_TEST_DOC));
  let parsed = written.map_err(|err| err.to_string()).and_then(|()| {
    parse_steps(&doc.display().to_string(), &dir).map_err(|failure| failure.message)
  });
  let _removed = std::fs::remove_dir_all(&dir);
  let steps = match parsed {
    Ok(steps) => steps,
    Err(message) => {
      out.stderr(message);
      out.stderr("plan-ledger --self-test: parse failed");
      return out.result(1, None);
    }
  };
  let expected = steps.len() == 2
    && steps.first().and_then(|s| s.get("id")) == Some(&string("s1"))
    && steps.get(1).and_then(|s| s.get("check")) == Some(&string("false"));
  if !expected {
    out.stderr("plan-ledger --self-test: unexpected parse result");
    return out.result(1, None);
  }
  out.stdout("plan-ledger --self-test: ok\n");
  out.result(0, None)
}

/// `toolu ledger path`: the ledger file.
///
/// # Errors
/// `plan-ledger: cannot resolve ledger path`, exit 2.
pub fn ledger_path_command(opts: &LedgerOptions) -> Result<PathBuf, CommandResult> {
  ledger_path(opts).ok_or_else(|| fail("plan-ledger: cannot resolve ledger path"))
}

/// `toolu ledger root`: the project root.
///
/// # Errors
/// `plan-ledger: not in a git repo`, exit 2.
pub fn ledger_root_command(opts: &LedgerOptions) -> Result<PathBuf, CommandResult> {
  project_root(opts).ok_or_else(|| fail("plan-ledger: not in a git repo"))
}

#[cfg(test)]
#[path = "tests/commands_test.rs"]
mod tests;
