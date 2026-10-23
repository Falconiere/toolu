//! `toolu epic graph` and `toolu epic route`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use clap::{ArgMatches, Command, value_parser};
use serde_json::json;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

use super::{document, failure, flag, option, text, usage};
use crate::graph::{
  BuildInput, Graph, build, herdr_roots, read_graph, render_table, repo_roots, write_graph,
};
use crate::paths::Paths;
use crate::reach::Reach;
use crate::refs::{current_repo, epics_home};
use crate::route::{
  HostPool, RouteOptions, RoutingTable, load_table, parse_hosts, render_routes, route_issues,
};
use crate::tracker::{TrackerKind, detect_tracker};

pub(super) fn graph_command() -> Command {
  Command::new("graph")
    .about("Build an epic's dependency graph and the next launch batch")
    .arg(
      clap::Arg::new("epic")
        .value_name("EPIC")
        .required(true)
        .help("The epic: a GitHub issue, Jira key or Linear issue"),
    )
    .arg(
      option("max", "N", "Live issue agents at once")
        .value_parser(value_parser!(u64))
        .default_value("3"),
    )
    .arg(
      option("tracker", "TRACKER", "The epic's tracker").value_parser(["github", "jira", "linear"]),
    )
    .arg(option(
      "repo",
      "O/R",
      "The code repository of Jira and Linear children",
    ))
    .arg(option("out", "FILE", "Also write graph.json to FILE"))
    .arg(flag(
      "save",
      "Write graph.json into the epic's state directory",
    ))
    .arg(flag("dry-run", "Read the tracker and write nothing"))
}

pub(super) fn route_command() -> Command {
  Command::new("route")
    .about("Route sub-issues to worker hosts, models and efforts")
    .arg(option("graph", "FILE", "The epic's graph.json").required(true))
    .arg(option("hosts", "SPEC", "The host pool, e.g. claude:2,codex:1").default_value("claude"))
    .arg(
      option(
        "issue",
        "REF",
        "Route this issue (repeatable); default the launch batch",
      )
      .action(clap::ArgAction::Append),
    )
    .arg(flag("no-jev", "Use the heuristic tiers only"))
    .arg(flag(
      "reroute",
      "Choose a host again even when the saved one is valid",
    ))
    .arg(flag("dry-run", "Print the routes and write nothing"))
}

/// Build the graph, save it where asked, and print the table or the document.
pub(super) fn graph(matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Outcome {
  graph_with(matches, ctx, env, &Reach::live())
}

/// The children's default code repository: `--repo`, else the `current`
/// repository for Jira and Linear; GitHub children name their own.
fn default_repo(
  matches: &ArgMatches,
  kind: TrackerKind,
  current: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
  match text(matches, "repo") {
    Some(repo) => Ok(repo.to_owned()),
    None if kind == TrackerKind::Github => Ok(String::new()),
    None => current(),
  }
}

/// [`graph`] with the clients `reach` builds.
fn graph_with(matches: &ArgMatches, ctx: &Ctx, env: &Env, reach: &Reach) -> Outcome {
  let epic = text(matches, "epic").unwrap_or("");
  let kind = match detect_tracker(epic, text(matches, "tracker"), env) {
    Ok(kind) => kind,
    Err(err) => return usage("toolu epic graph", &err),
  };
  let tracker = default_repo(matches, kind, current_repo)
    .and_then(|repo| Ok(((reach.tracker)(kind, epic, &repo, env)?, repo)));
  let (tracker, repo) = match tracker {
    Ok(made) => made,
    Err(err) => return failure("toolu epic graph", &err),
  };
  // A Jira or Linear child's branch PRs are read from GitHub.
  let gh = match kind {
    TrackerKind::Github => None,
    TrackerKind::Jira | TrackerKind::Linear => match (reach.gh)(env) {
      Ok(gh) => Some(gh),
      Err(err) => return failure("toolu epic graph", &err),
    },
  };
  let input = BuildInput {
    tracker: tracker.as_ref(),
    default_repo: repo,
    state_dir: epics_home(env).join(tracker.state_slug()),
    max_parallel: matches.get_one::<u64>("max").copied().unwrap_or(3),
    gh: gh.as_ref(),
    herdr_roots: herdr_roots(&Paths::from_env(env), env),
    repo_roots: repo_roots(env),
  };
  match build(&input).and_then(|built| save(matches, &built).map(|()| built)) {
    Ok(built) if ctx.json => document(&json!(built)),
    Ok(built) => Outcome::data(render_table(&built)),
    Err(err) => failure("toolu epic graph", &err),
  }
}

/// Write `built` to `--out` and, with `--save`, the state directory; a dry
/// run writes nothing.
fn save(matches: &ArgMatches, built: &Graph) -> Result<(), String> {
  if matches.get_flag("dry-run") {
    return Ok(());
  }
  let mut targets: Vec<PathBuf> = text(matches, "out")
    .map(PathBuf::from)
    .into_iter()
    .collect();
  if matches.get_flag("save") {
    targets.push(Path::new(&built.state_dir).join("graph.json"));
  }
  targets
    .iter()
    .try_for_each(|target| write_graph(target, built))
}

/// Route the wanted issues (or the launch batch) and print the routes.
pub(super) fn route(matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Outcome {
  let file = text(matches, "graph").unwrap_or("");
  let graph = match read_graph(Path::new(file)) {
    Ok(graph) => graph,
    Err(err) => return failure("toolu epic route", &err),
  };
  let pool: HostPool = match parse_hosts(
    text(matches, "hosts").unwrap_or("claude"),
    graph.max_parallel,
  ) {
    Ok(pool) => pool,
    Err(err) => return usage("toolu epic route", &err),
  };
  let table: RoutingTable = load_table(env);
  let wanted: Vec<String> = matches.get_many::<String>("issue").map_or_else(
    || graph.launch_batch.clone(),
    |issues| issues.cloned().collect(),
  );
  let jev = crate::verbs_actions::launch::jev_client(env);
  let no_jev = crate::epics::launch_settings(Path::new(&graph.state_dir)).no_jev;
  let options = RouteOptions {
    jev: if matches.get_flag("no-jev") || no_jev {
      None
    } else {
      Some(jev.as_ref().map_err(Clone::clone))
    },
    reroute: matches.get_flag("reroute"),
    table: &table,
    resource_root: Some(toolu_engine::resources::store::resource_home(env)),
    now_ms: toolu_state::time::epoch_millis(SystemTime::now()),
    persist: !matches.get_flag("dry-run"),
  };
  match route_issues(&graph, &wanted, &pool, &options) {
    Ok((routes, note)) if ctx.json => document(&json!({"routes": routes, "note": note})),
    Ok((routes, note)) => Outcome::data(render_routes(&routes, note.as_deref())),
    Err(err) => failure("toolu epic route", &err),
  }
}

#[cfg(test)]
#[path = "tests/graph_test.rs"]
mod tests;
