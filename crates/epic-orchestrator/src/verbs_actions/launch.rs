//! `toolu epic launch`.

use std::path::Path;

use clap::{ArgMatches, Command};
use toolu_jev_client::Jev;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

use super::{document, failure, flag, option, text};
use crate::graph::read_graph;
use crate::launch::{Ctx as LaunchCtx, LaunchOpts, Recovery, find_issue, launch_issue, render_dry};
use crate::paths::Paths;
use crate::tracker::Gh;

pub(super) fn launch_command() -> Command {
  Command::new("launch")
    .about("Launch or resume one sub-issue in its herdr worktree")
    .arg(option("graph", "FILE", "The epic's graph.json").required(true))
    .arg(option("issue", "REF", "The sub-issue: reference, key or URL").required(true))
    .arg(option(
      "kind",
      "HOST",
      "The host: claude, codex, cursor or muse; default the route's",
    ))
    .arg(option("model", "MODEL", "The model; default the route's"))
    .arg(option(
      "effort",
      "EFFORT",
      "The effort; default the route's",
    ))
    .arg(option(
      "permission-mode",
      "MODE",
      "Claude's permission mode; implies --safe",
    ))
    .arg(flag("safe", "Keep approval prompts on"))
    .arg(flag(
      "replace",
      "Exit a live agent first, e.g. to move the issue to another host",
    ))
    .arg(flag(
      "force",
      "Launch a blocked issue, or skip the GitHub budget guard",
    ))
    .arg(flag("reprompt", "Prompt again after an uncertain attempt"))
    .arg(flag(
      "dry-run",
      "Print the commands and the brief; write nothing",
    ))
}

/// Jev from the environment, or why it is unavailable.
pub(super) fn jev_client(env: &Env) -> Result<Jev, String> {
  crate::route::jev_from_env(env)
}

fn opts(matches: &ArgMatches) -> LaunchOpts {
  let owned = |id: &str| text(matches, id).map(str::to_owned);
  let recovery = if matches.get_flag("replace") {
    Recovery::Replace
  } else if matches.get_flag("reprompt") {
    Recovery::Reprompt
  } else {
    Recovery::Fresh
  };
  LaunchOpts {
    kind: owned("kind"),
    model: owned("model"),
    effort: owned("effort"),
    permission_mode: owned("permission-mode").unwrap_or_else(|| "auto".to_owned()),
    safe: matches.get_flag("safe")
      || matches.contains_id("permission-mode") && text(matches, "permission-mode").is_some(),
    dry_run: matches.get_flag("dry-run"),
    force: matches.get_flag("force"),
    recovery,
  }
}

/// Launch one issue; a failure prints `{issue, error, native_exit_code}`.
pub(super) fn launch(matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Outcome {
  let wanted = text(matches, "issue").unwrap_or("");
  let graph = match read_graph(Path::new(text(matches, "graph").unwrap_or(""))) {
    Ok(graph) => graph,
    Err(err) => return failure("toolu epic launch", &err),
  };
  let mut flags = opts(matches);
  // A manual launch in an epic started with `--safe` keeps prompts on too.
  flags.safe |= crate::epics::launch_settings(Path::new(&graph.state_dir)).safe;
  let paths = Paths::from_env(env);
  let gh = Gh::new(env);
  let mut run = LaunchCtx::live(env, &paths, gh.as_ref().map_err(Clone::clone));
  let result =
    find_issue(&graph, wanted).and_then(|issue| launch_issue(&graph, issue, &flags, &mut run));
  match result {
    Ok(value) if flags.dry_run && !ctx.json => {
      Outcome::data(render_dry(&value).trim_end().to_owned())
    }
    Ok(value) => document(&value),
    Err(err) => Outcome::failed(Exit::Failure, err.report(wanted).to_string()),
  }
}

#[cfg(test)]
#[path = "tests/launch_test.rs"]
mod tests;
