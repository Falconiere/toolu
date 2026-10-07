//! One doctor check and the report folded from the list.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};

/// How serious a check's result is. Warn does not change the process exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Status {
  /// The check passed.
  Ok,
  /// Something needs attention and the command still exits 0.
  Warn,
  /// The command exits 1.
  Fail,
}

impl Status {
  /// The report's `status` string.
  pub(super) fn as_str(self) -> &'static str {
    match self {
      Status::Ok => "ok",
      Status::Warn => "warn",
      Status::Fail => "fail",
    }
  }
}

/// One row of the doctor report.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Check {
  /// Stable id, in report order.
  pub(super) id: &'static str,
  /// Ok, warn or fail.
  pub(super) status: Status,
  /// The one-line explanation.
  pub(super) summary: String,
  /// A fix, when the check has one.
  pub(super) hint: Option<String>,
  /// Machine-readable facts. Always a JSON object.
  pub(super) details: Value,
}

impl Check {
  /// A check with `details`.
  pub(super) fn new(
    id: &'static str,
    status: Status,
    summary: impl Into<String>,
    hint: Option<String>,
    details: Value,
  ) -> Check {
    Check {
      id,
      status,
      summary: summary.into(),
      hint,
      details,
    }
  }

  fn json(&self) -> Value {
    json!({
      "id": self.id,
      "status": self.status.as_str(),
      "summary": self.summary,
      "hint": self.hint,
      "details": self.details,
    })
  }
}

/// `toolu doctor: <n> check(s) failed`.
pub(super) fn failure_line(failed: usize) -> String {
  let checks = if failed == 1 { "check" } else { "checks" };
  format!("toolu doctor: {failed} {checks} failed")
}

/// Text report: one line per check, then `  fix: <hint>` when there is a hint.
pub(super) fn text(checks: &[Check]) -> String {
  let mut lines = Vec::new();
  for check in checks {
    lines.push(format!(
      "{} {}: {}",
      check.status.as_str(),
      check.id,
      check.summary
    ));
    if let Some(hint) = &check.hint {
      lines.push(format!("  fix: {hint}"));
    }
  }
  lines.join("\n")
}

/// The `--json` document before redaction.
pub(super) fn document(checks: &[Check]) -> Value {
  json!({
    "namespace": "doctor",
    "ok": !checks.iter().any(|check| check.status == Status::Fail),
    "checks": checks.iter().map(Check::json).collect::<Vec<_>>(),
  })
}

const BUN_HINT: &str = "install Bun 1.4.x";
const BUN_TIMEOUT: Duration = Duration::from_secs(1);

/// `bun --version` from `TOOLU_BUN`, else `PATH`. Missing or too slow warns.
pub(super) fn runtime(env: &Env) -> Check {
  let program = env.get("TOOLU_BUN").unwrap_or("bun");
  let mut spec = Spec::new([program, "--version"]);
  spec.timeout = BUN_TIMEOUT;
  spec.env = Some(env.clone());
  spec.max_output_bytes = 256;
  let version = run(&spec)
    .ok()
    .filter(|output| output.exit_code == 0 && !output.timed_out && !output.truncated);
  match version {
    Some(output) => {
      let version = output.stdout.lines().next().unwrap_or("").trim();
      Check::new(
        "runtime",
        Status::Ok,
        format!("bun {version}"),
        None,
        json!({ "version": version }),
      )
    }
    None => Check::new(
      "runtime",
      Status::Warn,
      "Bun is missing",
      Some(BUN_HINT.to_owned()),
      json!({}),
    ),
  }
}

/// Host, config root and project root. A detection warning warns.
pub(super) fn host(roots: &Roots, cwd: &Path) -> Check {
  let config = roots.config_root();
  let project = roots.project_root(Some(cwd));
  let project_text = project
    .as_ref()
    .map_or_else(|| "none".to_owned(), |path| path.display().to_string());
  let mut summary = format!(
    "{} config {} project {project_text}",
    roots.host().name(),
    config.display()
  );
  if let Some(warning) = roots.warning() {
    summary.push_str("; ");
    summary.push_str(warning);
  }
  Check::new(
    "host",
    if roots.warning().is_some() {
      Status::Warn
    } else {
      Status::Ok
    },
    summary,
    None,
    json!({
      "host": roots.host().name(),
      "configRoot": config.display().to_string(),
      "projectRoot": project.map(|path| path.display().to_string()),
    }),
  )
}

#[cfg(test)]
#[path = "tests/checks_test.rs"]
mod tests;
