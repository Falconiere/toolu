//! Native commands for documentation review routing and state.

use std::path::{Path, PathBuf};

use clap::{Arg, ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::{
  cli::{Ctx, Outcome},
  env::Env,
  host::roots::Roots,
};

mod route;

/// The plugin providing this namespace.
pub const PLUGIN: &str = "documentation-gate";

/// The documentation review namespace.
pub fn command() -> Command {
  Command::new("docs")
    .about("Inspect and record documentation reviews")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(repo_args(
      Command::new("inspect").about("Inspect the committed diff"),
    ))
    .subcommand(
      Command::new("route")
        .about("Choose a ready documentation worker")
        .arg(file_arg("candidates"))
        .arg(file_arg("evidence")),
    )
    .subcommand(repo_args(
      Command::new("write-state")
        .about("Record a completed review")
        .arg(file_arg("report")),
    ))
}

fn file_arg(name: &'static str) -> Arg {
  Arg::new(name).long(name).value_name("FILE").required(true)
}

fn repo_args(command: Command) -> Command {
  command
    .arg(Arg::new("repo").long("repo").value_name("PATH"))
    .arg(Arg::new("branch").long("branch").value_name("NAME"))
}

/// Run a documentation review command.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let mut env = Env::process();
  if let Some(path) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &path.to_string_lossy());
  }
  let roots = Roots::new(env.clone(), ctx.host);
  let result = match matches.subcommand() {
    Some(("route", args)) => route::run(args, &env),
    Some(("inspect", args)) => with_repo(args, |repo, branch| {
      toolu_engine::docs_review::inspect(&roots, repo, branch).map(|value| value.to_string())
    }),
    Some(("write-state", args)) => with_repo(args, |repo, branch| {
      let report = read_json(required(args, "report")?)?;
      toolu_engine::docs_review::write(&roots, repo, branch, &report)
        .map(|path| serde_json::json!({"path": path.to_string_lossy()}).to_string())
    }),
    _ => Err("unknown docs command".to_owned()),
  };
  match result {
    Ok(output) => Outcome::data(output),
    Err(error) => Outcome::failed(Exit::Failure, format!("docs: {error}")),
  }
}

fn required<'a>(args: &'a ArgMatches, name: &str) -> Result<&'a str, String> {
  args
    .get_one::<String>(name)
    .map(String::as_str)
    .ok_or_else(|| format!("missing --{name}"))
}

fn read_json(path: &str) -> Result<serde_json::Value, String> {
  let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
  serde_json::from_str(&text).map_err(|e| format!("invalid JSON in {path}: {e}"))
}

/// Run `f` on `--repo`, else the current directory, as `toolu review write-state` does.
fn with_repo(
  args: &ArgMatches,
  f: impl FnOnce(&Path, Option<&str>) -> Result<String, String>,
) -> Result<String, String> {
  let repo = args
    .get_one::<String>("repo")
    .map_or_else(|| PathBuf::from("."), PathBuf::from);
  let branch = args.get_one::<String>("branch").map(String::as_str);
  f(&repo, branch)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
