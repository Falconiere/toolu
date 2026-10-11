//! `toolu status`: the repository, gate and push-review snapshot (#445).

use std::path::Path;

use clap::{ArgMatches, Command};
use serde_json::{Value, json};
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::host::roots::Roots;

mod cache;
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

  fn snapshot_cached(&self, roots: &Roots, dir: &Path) -> Result<Value, toolu_engine::LinkError> {
    Ok(document_with(roots, dir, &repo::inspect_cached(roots, dir)))
  }

  fn refresh_cache(&self, roots: &Roots, dir: &Path) -> Result<(), toolu_engine::LinkError> {
    let toolu_runtime::git::Discovery::Repo(found) = toolu_runtime::git::discover(roots.env(), dir)
    else {
      return Err(toolu_engine::LinkError::Failed(
        "statusline: no supported worktree".to_owned(),
      ));
    };
    cache::refresh(roots, &found).map_err(toolu_engine::LinkError::Failed)
  }
}

fn document(roots: &Roots, dir: &Path) -> Value {
  document_with(roots, dir, &repo::inspect(roots.env(), dir))
}

fn document_with(roots: &Roots, dir: &Path, found: &repo::Repo) -> Value {
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
