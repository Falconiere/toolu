//! The `SessionStart` hook (port of `plugins/jev/hooks/src/session-start.ts`): link the
//! `scripts/jev.sh` shim at `<config root>/jev/jev.sh`, a stable path the agent's shell can
//! expand, then state the Jev mandate. The helpers here are shared with the prompt and
//! binary-check hooks.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::startup::context::{render_hook_output, session_context};
use toolu_runtime::startup::publish::{PublishOptions, Published, publish};
use toolu_runtime::startup::report::HelperStatus;

const UNAVAILABLE: &str = "Jev unavailable: published wrapper is not executable. Repair the Jev \
plugin installation. Until then, state the limitation once per task and use an explicit \
reasoning/evidence fallback; never invent a Jev result. Do not read credentials from .env.";

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

/// `true` when the path has any execute bit, following links.
pub(crate) fn executable(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

fn quote(path: &Path) -> String {
  format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

/// What the agent runs: `toolu jev` for the published symlink, the quoted path for a
/// user's own file, which brings its own interpreter.
pub(crate) fn invocation(wrapper: &Path) -> String {
  let ours = std::fs::symlink_metadata(wrapper).is_ok_and(|meta| meta.file_type().is_symlink());
  if ours {
    "toolu jev".to_owned()
  } else {
    quote(wrapper)
  }
}

/// Where the agent finds the CLI syntax: the native skill on `OpenCode`, else the plugin's file.
pub(crate) fn skill_reference(host: Host, plugin_root: &Path) -> String {
  if host == Host::Opencode {
    "skill({ name: \"jev-jev\" })".to_owned()
  } else {
    format!("{}/skills/jev/SKILL.md", plugin_root.display())
  }
}

/// Hooks and agent commands can receive different environments. Never expose the key.
pub(crate) fn credential_notice(env: &Env) -> &'static str {
  match env.get("TYPESAFE_API_KEY") {
    Some(key) if !key.is_empty() => "",
    Some(_) | None => CREDENTIALS,
  }
}

fn mandate(roots: &Roots, plugin_root: &Path, wrapper: &Path) -> String {
  if !executable(wrapper) {
    return UNAVAILABLE.to_owned();
  }
  format!(
    concat!(
      "{}Jev is mandatory on every task containing semantic decisions. After initial exploration, ",
      "identify useful judgments over supplied evidence; you MUST call {} before the decision it ",
      "informs. The command is toolu jev and does not load a project .env file. Reassess after ",
      "new evidence, failed hypotheses, or changed requirements. Batch independent questions in ",
      "one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a ",
      "task has no semantic decision, say so in one sentence rather than skipping silently. ",
      "Syntax and linked examples: {}. Keep exact rules, tests, and code verification ",
      "deterministic. On service failure, state the limitation and use an explicit evidence ",
      "fallback. Jev never replaces tests or authorization."
    ),
    credential_notice(roots.env()),
    invocation(wrapper),
    skill_reference(roots.host(), plugin_root),
  )
}

/// `source` of the payload names the trigger; only `compact` on `OpenCode` stays quiet.
fn compacting(stdin: Option<&str>) -> bool {
  let input = payload(stdin);
  matches!(
    input.as_ref().and_then(|input| input.get("source")),
    Some(Ordered::String(source)) if source == "compact"
  )
}

fn outcome_of(published: &Published, quiet: bool, roots: &Roots, plugin_root: &Path) -> Outcome {
  if let Some(warning) = &published.warning {
    return Outcome {
      exit: Exit::Success,
      stdout: None,
      stderr: Some(warning.clone()),
    };
  }
  let Some(path) = &published.result.path else {
    return silent();
  };
  match published.result.status {
    HelperStatus::LinkFailed => Outcome {
      exit: Exit::Success,
      stdout: None,
      stderr: Some(format!("jev: cannot publish {}", path.display())),
    },
    HelperStatus::Published | HelperStatus::KeptUserFile if !quiet => {
      context_outcome("SessionStart", &mandate(roots, plugin_root, path))
    }
    HelperStatus::Published
    | HelperStatus::KeptUserFile
    | HelperStatus::Unwritable
    | HelperStatus::SourceMissing => silent(),
  }
}

/// Publish the shim and state the mandate. `stdin` is the hook payload text, `None` when it
/// could not be read; it is a normal start unless it names `source` `compact` on `OpenCode`.
pub fn session_start(env: &Env, plugin_root: &Path, stdin: Option<&str>) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  let quiet = roots.host() == Host::Opencode && compacting(stdin);
  let source = plugin_root.join("scripts/jev.sh");
  let published = publish(&PublishOptions {
    plugin: "jev",
    source: &source,
    dir: "jev",
    name: "jev.sh",
    what: "wrapper",
    roots: &roots,
  });
  let mut outcome = outcome_of(&published, quiet, &roots, plugin_root);
  if let Some(error) = published.report_error {
    outcome.exit = Exit::Failure;
    outcome.stderr = Some(error);
  }
  outcome
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
