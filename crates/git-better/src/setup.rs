//! `toolu git-better setup`: add `refs/notes/git-better` to this repository's
//! `notes.rewriteRef`, so `git commit --amend` and `git rebase` copy each
//! commit's provenance note to its rewritten commit. Pushing is unchanged.

use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};

use crate::cli::env_of;
use crate::gitio::git;
use crate::note::NOTES_REF;

/// Run `setup` in the repository of the current directory.
pub(crate) fn setup(ctx: &Ctx) -> Outcome {
  let env = env_of(ctx);
  let cwd = std::env::current_dir().unwrap_or_default();
  let get = ["config", "--local", "--get-all", "notes.rewriteRef"];
  let present =
    git(&env, &cwd, &get, &[]).is_ok_and(|refs| refs.lines().any(|line| line.trim() == NOTES_REF));
  if !present {
    let add = ["config", "--local", "--add", "notes.rewriteRef", NOTES_REF];
    if let Err(why) = git(&env, &cwd, &add, &[]) {
      return Outcome::failed(Exit::Failure, why);
    }
  }
  let text = if ctx.json {
    json!({
      "namespace": crate::PLUGIN,
      "verb": "setup",
      "rewriteRef": NOTES_REF,
      "changed": !present,
    })
    .to_string()
  } else if present {
    format!("notes.rewriteRef already includes {NOTES_REF}")
  } else {
    format!("notes.rewriteRef now includes {NOTES_REF}: amend and rebase carry its notes")
  };
  Outcome::data(text)
}
