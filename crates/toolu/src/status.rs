//! `toolu status`: the status snapshot, not ported yet (#445).

use std::path::Path;

use clap::{ArgMatches, Command};
use serde_json::Value;
use toolu_engine::LinkError;
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "status",
  about: "Show the repository, gate and plugin status",
  verbs: &[],
  issues: &[445],
};

/// The `toolu status` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu status` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

/// The hub's [`StatusSnapshot`], which `crates/cli` hands statusline. The
/// snapshot is not ported yet (#445).
#[derive(Debug, Clone, Copy, Default)]
pub struct Snapshot;

impl StatusSnapshot for Snapshot {
  fn snapshot(&self, _roots: &Roots, _dir: &Path) -> Result<Value, LinkError> {
    Err(LinkError::NotPorted { issue: 445 })
  }
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
