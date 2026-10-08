//! Push-time acceptance-criterion coverage of a plan's spec.

use std::path::{Path, PathBuf};

use toolu_runtime::config::load::LoadedConfig;
use toolu_runtime::config::read::flag_true;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::telemetry::{TelemetryEvent, telemetry_append};

use crate::ledger::coverage::ac_coverage;
use crate::ledger::doc::{doc_field, is_specless};
use crate::ledger::jq::count;
use crate::ledger::parse::{is_file, resolve};
use crate::verdict::gates::field_or;

fn locate(cwd: &Path, root: &Path, path: &str) -> PathBuf {
  let here = resolve(cwd, path);
  if is_file(&here) {
    return here;
  }
  let there = root.join(path);
  if is_file(&there) { there } else { here }
}

/// Inputs to the optional AC coverage check.
pub(crate) struct AcCheck<'a> {
  pub(crate) ledger: &'a Ordered,
  pub(crate) current: &'a str,
  pub(crate) root: &'a Path,
  pub(crate) ctx: &'a RuleContext<'a>,
  pub(crate) config: &'a LoadedConfig,
  pub(crate) state: &'a mut StateCtx,
}

/// The uncovered AC lines, when config makes them a push stop.
pub(crate) fn ac_blockers(check: &mut AcCheck<'_>) -> Option<String> {
  let AcCheck {
    ledger,
    current,
    root,
    ctx,
    config,
    state,
  } = check;
  let plan_field = field_or(ledger, "plan_doc", "");
  if plan_field.is_empty() {
    return None;
  }
  let cwd = ctx.cwd.unwrap_or(ctx.project_root);
  let plan = locate(cwd, root, &plan_field);
  if !is_file(&plan) {
    return None;
  }
  let spec_field = doc_field(&plan, "Spec");
  let spec = if is_specless(&spec_field) {
    spec_field
  } else {
    locate(cwd, root, &spec_field).display().to_string()
  };
  let report = ac_coverage(ledger, current, &spec);
  if report.stdout.is_empty() {
    return None;
  }
  let uncovered: Vec<&str> = report
    .stdout
    .lines()
    .filter(|line| line.starts_with("  AC-") && line.contains(": UNCOVERED"))
    .collect();
  let event = TelemetryEvent::AcCoverage {
    covered: count(report.rows.len().saturating_sub(uncovered.len())),
    uncovered: count(uncovered.len()),
  };
  let _written = telemetry_append(state, root, &event);
  if uncovered.is_empty() || !flag_true(config, "planLedger", "blockOnUncoveredAcs") {
    return None;
  }
  Some(
    uncovered
      .into_iter()
      .map(|line| line.strip_prefix("  ").unwrap_or(line))
      .collect::<Vec<_>>()
      .join("\n"),
  )
}

#[cfg(test)]
#[path = "tests/plan_ledger_ac_test.rs"]
mod tests;
