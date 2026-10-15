//! The `SessionStart` hook: retire the `<config root>/jev/jev.sh` link older versions
//! published (#440; the agent runs `toolu jev`), then state the Jev mandate. The helpers
//! here are shared with the prompt and binary-check hooks.

use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::startup::context::{render_hook_output, session_context};
use toolu_runtime::startup::retire::{Retired, retire};

const CREDENTIALS: &str = "The Jev hook did not receive TYPESAFE_API_KEY. Before reporting Jev \
unavailable, check whether TYPESAFE_API_KEY is set in the command environment without printing \
its value; hook and command environments can differ. If absent there too, state the limitation \
once per task and use an explicit evidence fallback. Never invent a Jev result or read \
credentials from .env. ";

/// The hook payload when it is a JSON document; unreadable input is a plain start.
pub(crate) fn payload(stdin: Option<&str>) -> Option<Ordered> {
  Ordered::parse(stdin?).ok()
}

/// A hook that has nothing to say.
pub(crate) fn silent() -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: None,
  }
}

/// `text` as the hook's `additionalContext` for `event`. The binary adds the line feed
/// that `render_hook_output` ends with, so it is dropped here.
pub(crate) fn context_outcome(event: &str, text: &str) -> Outcome {
  match session_context(event, text) {
    Some(value) => {
      let line = render_hook_output(&value, false);
      Outcome::data(line.trim_end_matches('\n').to_owned())
    }
    None => silent(),
  }
}

/// Where the agent finds the CLI syntax: the plugin's skill file.
pub(crate) fn skill_reference(plugin_root: &Path) -> String {
  format!("{}/skills/jev/SKILL.md", plugin_root.display())
}

/// Hooks and agent commands can receive different environments. Never expose the key.
pub(crate) fn credential_notice(env: &Env) -> &'static str {
  match env.get("TYPESAFE_API_KEY") {
    Some(key) if !key.is_empty() => "",
    Some(_) | None => CREDENTIALS,
  }
}

fn mandate(roots: &Roots, plugin_root: &Path) -> String {
  format!(
    concat!(
      "{}Jev is mandatory on every task containing semantic decisions. After initial exploration, ",
      "identify useful judgments over supplied evidence; you MUST call toolu jev before the ",
      "decision it informs. The command does not load a project .env file. Reassess after ",
      "new evidence, failed hypotheses, or changed requirements. Batch independent questions in ",
      "one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a ",
      "task has no semantic decision, say so in one sentence rather than skipping silently. ",
      "Syntax and linked examples: {}. Keep exact rules, tests, and code verification ",
      "deterministic. On service failure, state the limitation and use an explicit evidence ",
      "fallback. Jev never replaces tests or authorization."
    ),
    credential_notice(roots.env()),
    skill_reference(plugin_root),
  )
}

/// Retire the old link and state the mandate.
pub fn session_start(env: &Env, plugin_root: &Path) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  let mut outcome = context_outcome("SessionStart", &mandate(&roots, plugin_root));
  if let Retired::Failed(line) = retire(&roots, "jev", "jev", "jev.sh", None) {
    outcome.stderr = Some(line);
  }
  outcome
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
