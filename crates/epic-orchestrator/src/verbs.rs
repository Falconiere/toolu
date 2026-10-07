//! The `toolu epic` command tree. Later verbs stay on `planned` (#435, #448).

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::namespace::Planned;

const LATER: Planned = Planned {
  name: "epic",
  about: "Drive an epic to merged PRs: the resident engine, merge queue and trackers",
  verbs: &[
    "graph", "route", "launch", "finish", "close", "release", "jira", "probe", "gate", "queue",
  ],
  issues: &[435, 448],
};

pub(crate) fn command() -> Command {
  Command::new("epic")
    .about("Drive an epic to merged PRs: the resident engine, merge queue and trackers")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(engine_command())
    .subcommand(start_command())
    .subcommand(status_command())
    .subcommand(pause_command(
      "pause",
      "Pause effects for every epic, or one epic",
    ))
    .subcommand(pause_command(
      "resume",
      "Resume effects for every epic, or one epic",
    ))
    .subcommand(key_command("ack", "Clear a stall attention item"))
    .subcommand(answer_command())
    .subcommand(wait_command())
    .subcommand(report_command())
    .subcommand(job_command())
    .subcommand(
      Command::new("service")
        .about("Install the user service unit")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
          Command::new("install").about("Write the systemd user unit without starting it"),
        ),
    )
    .subcommand(
      Command::new("planned").about("Not ported yet (#435, #448): show the planned verbs"),
    )
    .subcommand(token_command())
}

pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let env = env_of(ctx);
  match matches.subcommand() {
    Some(("engine", args)) => crate::control::engine(args, &env),
    Some(("start", args)) => crate::control::start(args, &env),
    Some(("status", args)) => crate::query::status(args, &env),
    Some(("pause", args)) => crate::query::pause(args, &env, true),
    Some(("resume", args)) => crate::query::pause(args, &env, false),
    Some(("ack", args)) => crate::query::ack(args, &env),
    Some(("answer", args)) => crate::query::answer(args, &env),
    Some(("wait", args)) => crate::query::wait(ctx, args, &env),
    Some(("report", args)) => crate::query::report(args, &env),
    Some(("job", args)) => crate::query::job(args),
    Some(("service", args)) if args.subcommand_name() == Some("install") => {
      crate::query::service(&env)
    }
    Some(("token", args)) if args.subcommand_name() == Some("new") => token_new(ctx),
    _ => LATER.run(ctx),
  }
}

fn engine_command() -> Command {
  Command::new("engine")
    .about("Run the resident engine in the foreground")
    .arg(flag(
      "ensure",
      "Start the engine when the registry is non-empty and the lock is free",
    ))
    .arg(flag(
      "replace",
      "Ask a running engine to drain, then take its place",
    ))
}

fn start_command() -> Command {
  Command::new("start")
    .about("Register an epic state directory and start the engine")
    .arg(
      Arg::new("state-dir")
        .value_name("STATE_DIR")
        .required(true)
        .help("The epic state directory"),
    )
}

fn status_command() -> Command {
  Command::new("status")
    .about("Print the engine, issues and attention")
    .arg(
      Arg::new("epic")
        .value_name("EPIC")
        .help("Epic key, or a trailing issue number"),
    )
}

fn pause_command(name: &'static str, about: &'static str) -> Command {
  Command::new(name).about(about).arg(
    Arg::new("epic")
      .value_name("EPIC")
      .help("One epic key; every epic when omitted"),
  )
}

fn key_command(name: &'static str, about: &'static str) -> Command {
  Command::new(name).about(about).arg(
    Arg::new("key")
      .value_name("KEY")
      .required(true)
      .help("Issue key"),
  )
}

fn answer_command() -> Command {
  Command::new("answer")
    .about("Record an answer on the journal for one issue")
    .arg(
      Arg::new("key")
        .value_name("KEY")
        .required(true)
        .help("Issue key"),
    )
    .arg(
      Arg::new("text")
        .value_name("TEXT")
        .required(true)
        .help("The answer"),
    )
}

fn wait_command() -> Command {
  Command::new("wait")
    .about("Wait for a judgment, or return when the limit passes")
    .arg(
      Arg::new("max-seconds")
        .long("max-seconds")
        .value_name("N")
        .value_parser(clap::value_parser!(u64))
        .help("Stop after this many seconds; 0 returns immediately"),
    )
}

fn report_command() -> Command {
  Command::new("report")
    .about("Record a worker phase")
    .arg(
      Arg::new("phase")
        .value_name("PHASE")
        .required(true)
        .help("The phase to record"),
    )
    .arg(
      Arg::new("status-file")
        .long("status-file")
        .value_name("FILE")
        .required(true)
        .help("The issue status snapshot"),
    )
    .arg(
      Arg::new("pr")
        .long("pr")
        .value_name("N")
        .value_parser(clap::value_parser!(u64))
        .help("Pull request number"),
    )
    .arg(
      Arg::new("note")
        .long("note")
        .value_name("TEXT")
        .help("Note stored with the phase"),
    )
}

fn job_command() -> Command {
  Command::new("job")
    .about("Run a command under the worktree's resource lease")
    .arg(
      Arg::new("argv")
        .value_name("ARGV")
        .required(true)
        .num_args(1..)
        .trailing_var_arg(true)
        .allow_hyphen_values(true)
        .help("The command and its arguments"),
    )
}

fn token_command() -> Command {
  Command::new("token")
    .about("Manage the status bearer token")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(Command::new("new").about("Rotate the status bearer token in secrets.json"))
}

fn flag(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name)
    .long(name)
    .action(ArgAction::SetTrue)
    .help(help)
}

pub(crate) fn token_new(ctx: &Ctx) -> Outcome {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy());
  }
  match secrets::rotate_status_token(&Roots::new(env, ctx.host)) {
    Ok(path) if ctx.json => Outcome::data(
      json!({ "status_token": "<redacted>", "path": path, "rotated": true }).to_string(),
    ),
    Ok(path) => Outcome::data(format!(
      "toolu epic: status token rotated at {}",
      path.display()
    )),
    Err(error) => Outcome::failed(Exit::Failure, format!("toolu epic token new: {error}")),
  }
}

pub(crate) fn env_of(ctx: &Ctx) -> Env {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy());
  }
  env
}

pub(crate) fn text<'a>(matches: &'a ArgMatches, id: &str) -> Option<&'a str> {
  matches.get_one::<String>(id).map(String::as_str)
}

pub(crate) fn json_out(value: &Value) -> Outcome {
  match serde_json::to_string(value) {
    Ok(text) => Outcome::data(text),
    Err(err) => {
      let err = err.to_string();
      failed("toolu epic", &err)
    }
  }
}

pub(crate) fn failed(prefix: &str, err: &str) -> Outcome {
  Outcome::failed(Exit::Failure, format!("{prefix}: {err}"))
}

#[cfg(test)]
#[path = "tests/verbs_test.rs"]
mod tests;
