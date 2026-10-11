//! OpenCode fixers (#357): a detached `opencode run` in the slot worktree, not
//! a herdr pane. Its agent comes in through `OPENCODE_CONFIG_CONTENT` with
//! `task` and the plain push and `gh` command forms denied, and its environment
//! has no GitHub login and lets git reach only local (`file`) remotes.

use std::io::Write as _;
use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::process::RunError;
use toolu_runtime::process::file::spawn_logged;

use super::procs::process_start;
use crate::error::{Failure, Result};
use crate::fixer::host::Fixer;
use crate::route::on_path;

/// The fixer agent's name in OpenCode's config.
pub(crate) const FIXER_AGENT: &str = "pr-babysit-fixer";

const GITHUB_TOKENS: [&str; 4] = [
  "GH_TOKEN",
  "GITHUB_TOKEN",
  "GH_ENTERPRISE_TOKEN",
  "GITHUB_ENTERPRISE_TOKEN",
];

/// One OpenCode fixer launch.
pub(crate) struct Run<'a> {
  /// The model id, or the user's default.
  pub(crate) model: Option<&'a str>,
  /// The variant (effort), or the model's default.
  pub(crate) effort: Option<&'a str>,
  /// `--auto`: approve asks, keep explicit denies.
  pub(crate) unattended: bool,
  /// The slot worktree.
  pub(crate) worktree: &'a Path,
  /// The one-line prompt naming the brief.
  pub(crate) prompt: &'a str,
}

fn agent() -> Value {
  let deny = "deny";
  json!({
    "mode": "primary",
    "description": "pr-babysit fixer: edits, tests and commits review fixes in its worktree",
    "permission": {
      "task": deny,
      "bash": {
        "gh": deny, "gh *": deny,
        "git push": deny, "git push *": deny, "git * push": deny, "git * push *": deny,
      },
    },
  })
}

fn invalid(message: &str) -> Failure {
  Failure::new("config_invalid", message)
}

/// The caller's `OPENCODE_CONFIG_CONTENT` with the fixer agent added; every
/// other key is kept.
///
/// # Errors
/// `config_invalid` when the caller's value is not a JSON object, or its `agent` is not one.
pub(crate) fn config_content(existing: Option<&str>) -> Result<String> {
  let mut base = Map::new();
  if let Some(text) = existing.filter(|text| !text.trim().is_empty()) {
    let Ok(Value::Object(value)) = serde_json::from_str::<Value>(text) else {
      return Err(invalid(
        "OPENCODE_CONFIG_CONTENT is not a JSON object; fix or unset it",
      ));
    };
    if value.get("agent").is_some_and(|agent| !agent.is_object()) {
      return Err(invalid("OPENCODE_CONFIG_CONTENT agent is not an object"));
    }
    base = value;
  }
  let agents = base.entry("agent").or_insert_with(|| json!({}));
  if let Value::Object(agents) = agents {
    agents.insert(FIXER_AGENT.into(), agent());
  }
  Ok(Value::Object(base).to_string())
}

/// The caller's environment, plus the fixer agent, minus every way to write to
/// GitHub: no token, a `gh` config dir that does not exist, and git limited to
/// the `file` transport.
///
/// # Errors
/// `config_invalid` for a bad `OPENCODE_CONFIG_CONTENT`.
pub(crate) fn fixer_env(base: &Env, worktree: &Path, gh_config: &Path) -> Result<Env> {
  let content = config_content(base.get("OPENCODE_CONFIG_CONTENT"))?;
  let kept = base.vars().filter(|(key, _)| !GITHUB_TOKENS.contains(key));
  let env = Env::from_pairs(kept)
    .with("PWD", &worktree.display().to_string())
    .with("OPENCODE_CONFIG_CONTENT", &content)
    .with("GH_CONFIG_DIR", &gh_config.display().to_string())
    .with("GIT_ALLOW_PROTOCOL", "file")
    .with("GIT_TERMINAL_PROMPT", "0");
  Ok(env)
}

/// `opencode run` arguments for one fixer group.
///
/// # Errors
/// `config_invalid` for a shell-unsafe model or effort.
pub(crate) fn args(run: &Run<'_>) -> Result<Vec<String>> {
  let mut args: Vec<String> = ["run", "--format", "json", "--dir"]
    .map(str::to_owned)
    .to_vec();
  args.push(run.worktree.display().to_string());
  args.extend(["--agent".to_owned(), FIXER_AGENT.to_owned()]);
  args.extend(Fixer::Opencode.args(FIXER_AGENT, run.model, run.effort, run.unattended)?);
  args.push(run.prompt.to_owned());
  Ok(args)
}

/// Starts `opencode run` detached in the worktree with output to `log`, and
/// returns its pid and start time; never waits for it.
///
/// # Errors
/// Why it did not start; a start failure's message also goes to the log.
pub(crate) fn spawn(
  run: &Run<'_>,
  log: &Path,
  env: &Env,
  gh_config: &Path,
) -> std::result::Result<(u32, String), String> {
  if !on_path(env, "opencode") {
    return Err("opencode is not on PATH".into());
  }
  let mut argv = vec!["opencode".to_owned()];
  argv.extend(args(run).map_err(|failure| failure.message)?);
  let child_env = fixer_env(env, run.worktree, gh_config).map_err(|failure| failure.message)?;
  let error = match spawn_logged(&argv, run.worktree, &child_env, log) {
    Ok(pid) => return Ok((pid, process_start(pid))),
    Err(RunError::Spawn(error)) => error,
    Err(other) => format!("{other:?}"),
  };
  let line = format!("opencode did not start: {error}\n");
  let logged = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log)
    .and_then(|mut file| file.write_all(line.as_bytes()));
  match logged {
    Ok(()) => Err(format!(
      "opencode did not start; its error goes to {}",
      log.display()
    )),
    Err(_) => Err(line.trim_end().to_owned()),
  }
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
