//! `toolu doctor`: installation, config, plugin and tool health (#445).

use clap::{ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::config::secrets::{self, Secrets};

mod binary;
mod checks;
mod config_check;
mod inventory;
mod registry_check;
mod skew_check;
mod tools;

/// Report binary, config, plugin and tool health.
pub fn command() -> Command {
  Command::new("doctor").about("Report binary, config, plugin and tool health")
}

/// Run every check and print one report. A failing check exits 1 with the report
/// still on stdout.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let (roots, cwd) = crate::overlaid_roots(ctx);
  let probed = binary::probe();
  let found = inventory::collect(&roots, &cwd);
  let secrets = secrets::load(&roots);
  let report = [
    binary::binary(&probed),
    binary::reachability(&probed),
    checks::runtime(roots.env()),
    checks::host(&roots, &cwd),
    config_check::check(&roots, &cwd, &secrets),
    inventory::plugins_check(&found),
    skew_check::check(&roots, &found),
    registry_check::check(&roots),
    tools::check(&found, roots.env()),
  ];
  outcome(&report, &secrets, ctx.json)
}

/// The redacted report. Stderr is the failure count only when a check failed.
fn outcome(
  report: &[checks::Check],
  secrets: &Result<Secrets, secrets::SecretError>,
  json: bool,
) -> Outcome {
  let fallback = Secrets::none();
  let redactor = secrets.as_ref().unwrap_or(&fallback);
  let body = checks::document(report);
  let failed = report
    .iter()
    .filter(|check| check.status == checks::Status::Fail)
    .count();
  let stdout = if json {
    secrets::redact_json(&body, redactor).to_string()
  } else {
    secrets::redact_text(&checks::text(report), redactor)
  };
  let stderr = (failed > 0).then(|| secrets::redact_text(&checks::failure_line(failed), redactor));
  Outcome {
    exit: if failed == 0 {
      Exit::Success
    } else {
      Exit::Failure
    },
    stdout: Some(stdout),
    stderr,
  }
}

#[cfg(test)]
#[path = "tests/doctor_test.rs"]
mod tests;
