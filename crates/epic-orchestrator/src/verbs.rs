//! The `toolu epic` command tree.

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{Value, json};
use toolu_engine::babysit::BabysitTick;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

/// The phases a worker reports, in pipeline order.
const PHASES: [&str; 12] = [
  "brainstorm",
  "spec",
  "spec-review",
  "plan",
  "plan-review",
  "execution",
  "pr-open",
  "babysit",
  "rebasing",
  "ready",
  "needs-human",
  "failed",
];

/// The complete `toolu epic` command tree.
pub(crate) fn command() -> Command {
  Command::new("epic")
    .about("Drive an epic to merged PRs: the resident engine, merge queue and trackers")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(engine_command())
    .subcommand(serve_command())
    .subcommand(start_command())
    .subcommand(crate::query::status_command())
    .subcommand(queue_command())
    .subcommand(pause_command(
      "pause",
      "Pause effects for every epic, or one epic",
    ))
    .subcommand(pause_command(
      "resume",
      "Resume effects for every epic, or one epic",
    ))
    .subcommand(key_command(
      "ack",
      "Acknowledge an issue's attention items, or a keyless one by kind",
    ))
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
    .subcommand(token_command())
    .subcommands(crate::verbs_actions::commands())
    .subcommands(crate::verbs_metrics::commands())
}

/// Dispatch one epic subcommand.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx, tick: &dyn BabysitTick) -> Outcome {
  let env = env_of(ctx);
  match matches.subcommand() {
    Some(("engine", args)) => crate::control::engine(args, &env, tick),
    Some(("serve", args)) => crate::control::serve(args, &env, tick),
    Some(("start", args)) => crate::control::start(args, &env),
    Some(("status", args)) => crate::query::status(args, &env),
    Some(("queue", args)) => crate::query::queue(args, &env),
    Some(("pause", args)) => crate::query::pause(args, &env, true),
    Some(("resume", args)) => crate::query::pause(args, &env, false),
    Some(("ack", args)) => crate::query::ack(ctx, args, &env),
    Some(("answer", args)) => crate::query::answer(ctx, args, &env),
    Some(("wait", args)) => crate::query::wait(ctx, args, &env),
    Some(("report", args)) => crate::query::report(args, &env),
    Some(("job", args)) => crate::query::job(args),
    Some(("service", args)) if args.subcommand_name() == Some("install") => {
      crate::service::service(&env)
    }
    Some(("token", args)) if args.subcommand_name() == Some("new") => token_new(ctx),
    Some((name, args)) => crate::verbs_actions::run(name, args, ctx, &env)
      .or_else(|| crate::verbs_metrics::run(name, args, ctx, &env))
      .unwrap_or_else(|| failed("toolu epic", &format!("unknown verb {name}"))),
    None => failed("toolu epic", "a verb is required"),
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
    .arg(http_arg())
    .arg(flag("no-http", "Disable the status listener").conflicts_with("http"))
}

fn serve_command() -> Command {
  Command::new("serve")
    .about("Run the resident engine with its HTTP status listener")
    .arg(http_arg())
    .arg(flag("no-http", "Disable the status listener").conflicts_with("http"))
}

fn http_arg() -> Arg {
  Arg::new("http")
    .long("http")
    .value_name("ADDR")
    .help("Bind status HTTP to this IP address and port")
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
    .arg(
      Arg::new("safe")
        .long("safe")
        .action(ArgAction::SetTrue)
        .help("Keep approval prompts on in this epic's workers, until a start without it"),
    )
    .arg(
      Arg::new("no-jev")
        .long("no-jev")
        .action(ArgAction::SetTrue)
        .help("Route and check coupling by heuristics only, until a start without it"),
    )
}

fn queue_command() -> Command {
  Command::new("queue")
    .about("Print the merge queue of every repository, in merge order")
    .arg(
      Arg::new("epic")
        .value_name("EPIC")
        .help("Only this epic's entries"),
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
    .about("Send a message to a live worker; marks its open attention answered")
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
    .arg(
      Arg::new("by")
        .long("by")
        .value_name("NAME")
        .help("Who answered; defaults to the --host name, else user"),
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
    .arg(
      Arg::new("epic")
        .long("epic")
        .value_name("EPIC")
        .help("Only this epic's items and engine-wide ones; key or trailing issue number"),
    )
}

fn report_command() -> Command {
  Command::new("report")
    .about("Record a worker phase")
    .arg(
      Arg::new("phase")
        .value_name("PHASE")
        .required(true)
        .value_parser(PHASES)
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

/// Rotate the status token without exposing its value.
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

/// The command environment with an explicit config override.
pub(crate) fn env_of(ctx: &Ctx) -> Env {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy());
  }
  env
}

/// Read one string argument.
pub(crate) fn text<'a>(matches: &'a ArgMatches, id: &str) -> Option<&'a str> {
  matches.get_one::<String>(id).map(String::as_str)
}

/// Encode one JSON document as CLI output.
pub(crate) fn json_out(value: &Value) -> Outcome {
  match serde_json::to_string(value) {
    Ok(text) => Outcome::data(text),
    Err(err) => {
      let err = err.to_string();
      failed("toolu epic", &err)
    }
  }
}

/// Return a prefixed CLI failure without source bytes.
pub(crate) fn failed(prefix: &str, err: &str) -> Outcome {
  Outcome::failed(Exit::Failure, format!("{prefix}: {err}"))
}

#[cfg(test)]
#[path = "tests/verbs_test.rs"]
mod tests;
