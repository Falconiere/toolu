//! The `SessionStart` summary: the format in one line, then, on a feature
//! branch, what the branch has done so far, so the agent starts from the
//! branch's intent instead of rereading its diff. Silent outside a repository
//! and when `gitBetter.sessionContext` is `false`.

use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::context::{render_hook_output, session_context};

use toolu_state::git::{base_branch, current_branch};

use crate::context::gather;
use crate::settings;
use crate::views::context_text;

/// The commits the summary reads.
const SESSION_LIMIT: usize = 10;

/// The format, as the agent should keep it in mind.
pub(crate) const FORMAT: &str = "git-better: commit as `type(scope): description` with an \
`Intent:` trailer saying why, in the final trailer paragraph; name new branches \
`<type>/[<ref>-]<slug>`. `toolu git-better check` validates a message or branch first; \
`toolu git-better context`, `log` and `pr` read the branch.";

/// The summary for the repository at `dir`, or `None` when there is nothing to say.
pub(crate) fn summary(env: &Env, dir: &Path) -> Option<String> {
  let roots = Roots::new(env.clone(), None);
  let settings = settings::load(&roots, Some(dir), None);
  if !settings.session_context {
    return None;
  }
  let branch = current_branch(env, dir);
  if branch.is_empty() {
    return None;
  }
  if branch == base_branch(env, None, dir) {
    return Some(FORMAT.to_owned());
  }
  let context = gather(env, dir, &settings, None, SESSION_LIMIT).ok()?;
  if context.on_base {
    return Some(FORMAT.to_owned());
  }
  Some(format!("{FORMAT}\n{}", context_text(&context)))
}

/// The `session-start` hook for the current directory.
pub fn session_start(env: &Env) -> Outcome {
  let dir = toolu_runtime::invocation::current_dir().unwrap_or_default();
  let value = summary(env, &dir).and_then(|text| session_context("SessionStart", &text));
  Outcome {
    exit: Exit::Success,
    stdout: value.map(|value| {
      render_hook_output(&value, false)
        .trim_end_matches('\n')
        .to_owned()
    }),
    stderr: None,
  }
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
