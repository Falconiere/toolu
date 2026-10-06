//! Validate the GitHub workflows against the Rust CI and release contract.

use std::process::Command;

use crate::Verdict;
use crate::options::Options;

/// Run the Bun YAML check against this workspace (or a fixture root).
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let status = Command::new("bun")
    .arg("tooling/src/check-workflows.ts")
    .current_dir(&options.root)
    .status()
    .map_err(|err| format!("cannot run Bun workflow check: {err}"))?;
  Ok(if status.success() {
    Verdict::Clean
  } else {
    Verdict::Findings
  })
}

#[cfg(test)]
#[path = "tests/check_workflows_test.rs"]
mod tests;
