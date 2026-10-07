//! The engine's results as `Outcome`s: the TypeScript CLI's bytes (less the
//! one trailing newline `crates/cli` adds back), exit 0/1/2 as success, failure
//! and blocked, and under `--json` one document per verb.

use std::path::PathBuf;

use toolu_engine::ledger::coverage::{AcCoverage, CoverageReport};
use toolu_engine::ledger::io::CommandResult;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::json::ordered::Ordered;

/// The ledger CLIs' exit code under the #442 names.
pub fn exit_of(code: u8) -> Exit {
  match code {
    0 => Exit::Success,
    1 => Exit::Failure,
    _ => Exit::Blocked,
  }
}

/// Printed text without the newline `emit` adds back; `None` when nothing was printed.
fn trimmed(text: &str) -> Option<String> {
  (!text.is_empty()).then(|| text.strip_suffix('\n').unwrap_or(text).to_owned())
}

/// `result` as the CLI prints it, or with `document` as its data.
pub fn reported(result: &CommandResult, document: Option<Ordered>) -> Outcome {
  Outcome {
    exit: exit_of(result.exit),
    stdout: document.map_or_else(|| trimmed(&result.stdout), |doc| Some(doc.to_text(false))),
    stderr: trimmed(&result.stderr),
  }
}

fn object(entries: Vec<(&str, Ordered)>) -> Ordered {
  Ordered::Object(
    entries
      .into_iter()
      .map(|(key, value)| (key.to_owned(), value))
      .collect(),
  )
}

/// The summary line, when the CLI printed one.
fn summary(stdout: &str) -> Ordered {
  stdout
    .lines()
    .next()
    .filter(|line| line.starts_with("plan-ledger "))
    .map_or(Ordered::Null, |line| Ordered::String(line.to_owned()))
}

/// `run`: under `--json`, `{summary, ledger}` once a ledger was written.
pub fn ran(result: &CommandResult, json: bool) -> Outcome {
  let document = result.ledger.clone().filter(|_| json).map(|ledger| {
    object(vec![
      ("summary", summary(&result.stdout)),
      ("ledger", ledger),
    ])
  });
  reported(result, document)
}

fn row(coverage: &AcCoverage) -> Ordered {
  object(vec![
    ("id", Ordered::String(coverage.id.clone())),
    ("covered", Ordered::Bool(coverage.covered)),
    (
      "steps",
      Ordered::Array(
        coverage
          .steps
          .iter()
          .cloned()
          .map(Ordered::String)
          .collect(),
      ),
    ),
  ])
}

/// `status`: under `--json`, `{summary, ledger, ac_coverage}`.
pub fn statused((result, report): (CommandResult, CoverageReport), json: bool) -> Outcome {
  let document = result.ledger.clone().filter(|_| json).map(|ledger| {
    let rows = Ordered::Array(report.rows.iter().map(row).collect());
    object(vec![
      ("summary", summary(&result.stdout)),
      ("ledger", ledger),
      ("ac_coverage", rows),
    ])
  });
  reported(&result, document)
}

/// `path` and `root`: the path, or `{<key>: path}` under `--json`.
pub fn located(found: Result<PathBuf, CommandResult>, key: &str, json: bool) -> Outcome {
  match found {
    Ok(path) => {
      let text = path.display().to_string();
      let data = if json {
        object(vec![(key, Ordered::String(text))]).to_text(false)
      } else {
        text
      };
      Outcome::data(data)
    }
    Err(failed) => reported(&failed, None),
  }
}

/// `self-test`: `{"ok": true}` under `--json` when it passes.
pub fn self_tested(result: &CommandResult, json: bool) -> Outcome {
  let document = (json && result.exit == 0).then(|| object(vec![("ok", Ordered::Bool(true))]));
  reported(result, document)
}

/// `verdict`: the report as one compact document under `--json`.
pub fn verdict(result: &CommandResult, json: bool) -> Outcome {
  let document = result.ledger.clone().filter(|_| json);
  reported(result, document)
}

#[cfg(test)]
#[path = "tests/outcome_test.rs"]
mod tests;
