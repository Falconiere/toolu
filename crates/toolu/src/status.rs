//! `toolu status`: the repository, gate and push-review snapshot (#445).

use std::path::Path;

use clap::{ArgMatches, Command};
use serde_json::{Value, json};
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::host::roots::Roots;

mod gate;
mod repo;

/// Show the repository, gate and push-review status.
pub fn command() -> Command {
  Command::new("status").about("Show the repository, gate and push-review status")
}

/// Print the same document [`Snapshot`] gives statusline.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let (roots, cwd) = crate::overlaid_roots(ctx);
  match Snapshot.snapshot(&roots, &cwd) {
    Ok(document) => Outcome::data(render(&document, ctx.json)),
    Err(error) => Outcome::failed(toolu_protocol::exit::Exit::Failure, error.to_string()),
  }
}

/// The hub's [`StatusSnapshot`]. `crates/cli` hands it to statusline.
#[derive(Debug, Clone, Copy, Default)]
pub struct Snapshot;

impl StatusSnapshot for Snapshot {
  fn snapshot(&self, roots: &Roots, dir: &Path) -> Result<Value, toolu_engine::LinkError> {
    Ok(document(roots, dir))
  }
}

fn document(roots: &Roots, dir: &Path) -> Value {
  let found = repo::inspect(roots.env(), dir);
  let project = roots.project_root(Some(dir));
  let state = roots.project_state_root(Some(dir), project.as_deref());
  let gate_path = state
    .as_ref()
    .map(|state| state.join("quality-gate-status.json"));
  let (push_review, waivers) = gate::review(state.as_deref(), &found.branch);
  json!({
    "namespace": "status",
    "host": roots.host().name(),
    "cwd": dir.display().to_string(),
    "repo_root": found.root,
    "branch": found.branch,
    "ahead": found.ahead,
    "behind": found.behind,
    "working_tree": {
      "staged": found.staged,
      "unstaged": found.unstaged,
      "untracked": found.untracked,
    },
    "gate": gate::read(gate_path.as_deref()),
    "push_review": push_review,
    "waivers": waivers,
  })
}

fn render(document: &Value, json_out: bool) -> String {
  if json_out {
    document.to_string()
  } else {
    serde_json::to_string_pretty(document).unwrap_or_else(|_err| document.to_string())
  }
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
