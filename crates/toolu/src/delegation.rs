//! Global machine setup and operational delegation commands.

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{Value, json};
use toolu_mux::relay::{
  client, config,
  store::{Store, Submission, occupied},
};
use toolu_protocol::exit::Exit;
use toolu_runtime::{
  cli::{Ctx, Outcome},
  env::Env,
};

mod machines;
mod selection;
mod service;
mod task;
mod trust;
use machines::{machine_operation, setup_machines};

/// Registration commands shared by setup and tests.
pub fn machines_command() -> Command {
  Command::new("machines")
    .about("Register existing Herdr workers")
    .subcommand(Command::new("list").about("List saved workers"))
    .subcommand(
      Command::new("register")
        .about("Register or update a worker")
        .arg(Arg::new("name").required(true))
        .arg(
          Arg::new("root")
            .long("root")
            .required(true)
            .help("Absolute worker workspace root"),
        )
        .arg(
          Arg::new("agent")
            .long("agent")
            .required(true)
            .action(ArgAction::Append)
            .value_parser(["claude", "codex", "muse", "cursor", "opencode"]),
        )
        .arg(
          Arg::new("capacity")
            .long("capacity")
            .default_value("1")
            .value_parser(clap::value_parser!(u32).range(1..)),
        ),
    )
    .subcommand(
      Command::new("remove")
        .about("Remove an idle worker")
        .arg(Arg::new("name").required(true)),
    )
    .subcommand(
      Command::new("recipe")
        .about("Save explicit repository setup argv recipes")
        .arg(Arg::new("origin").long("origin").required(true))
        .arg(
          Arg::new("file")
            .long("file")
            .required(true)
            .help("JSON array of argv arrays; [] removes the recipe"),
        ),
    )
}

/// Worker readiness and drain operations.
pub fn operations_command() -> Command {
  Command::new("machines")
    .about("Inspect readiness and drain workers")
    .subcommand(Command::new("status").about("Show configuration and authoritative assignments"))
    .subcommands(["drain", "resume"].map(|name| {
      Command::new(name)
        .about("Change worker admission")
        .arg(Arg::new("name").required(true))
    }))
}

/// Persistent service controls.
pub fn relay_command() -> Command {
  Command::new("relay")
    .about("Inspect and recover the delegation relay")
    .subcommand(Command::new("status").about("Show the relay and durable assignments"))
    .subcommand(Command::new("recover").about("Reconcile the relay and restart its service"))
    .subcommand(Command::new("serve").about("Run the durable scheduler on its designated relay"))
    .subcommand(
      Command::new("receive")
        .hide(true)
        .about("Authenticated Herdr RPC receiver"),
    )
}

/// Standalone task submission, results and cancellation.
pub fn delegate_command() -> Command {
  Command::new("delegate")
    .about("Submit committed work to registered machines")
    .subcommand(delegate_submit())
    .subcommands(["status", "cancel", "retry"].map(|name| {
      Command::new(name)
        .about("Inspect, retry delivery or cancel a submitted task")
        .arg(Arg::new("key").required(true))
    }))
    .subcommand(
      Command::new("results")
        .about("Import a verified result under a private Git ref")
        .arg(Arg::new("key").required(true))
        .arg(Arg::new("repo").long("repo").required(true)),
    )
    .subcommand(delegate_report())
    .subcommand(delegate_trust())
}

fn delegate_trust() -> Command {
  Command::new("trust")
    .about("Trust an attempt directory in this machine's host trust store")
    .arg(
      Arg::new("agent")
        .long("agent")
        .required(true)
        .value_parser(["claude", "codex"]),
    )
    .arg(
      Arg::new("path")
        .long("path")
        .required(true)
        .help("Absolute attempt worktree"),
    )
    .arg(
      Arg::new("repo")
        .long("repo")
        .help("Absolute clone root the worktree belongs to"),
    )
}

/// Run worker operations.
pub fn machines_run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  outcome(machine_operation(matches, &env(ctx)))
}

/// Run relay operations.
pub fn relay_run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let env = env(ctx);
  let result = match matches.subcommand_name() {
    Some("receive") => toolu_mux::relay::rpc::receive(
      &config::root(&env),
      &env,
      ctx.stdin.as_deref().unwrap_or(""),
    ),
    Some("serve") => service::serve(&env),
    Some("recover") => service::start(&env),
    Some("status") => service::status(&env),
    _ => Err("a relay operation is required".into()),
  };
  outcome(result)
}

/// Run standalone task operations.
pub fn delegate_run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  outcome(task::run(matches, &env(ctx)))
}

/// Configure global worker registrations or relay selection.
pub(super) fn setup(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let env = env(ctx);
  let result = match matches.subcommand() {
    Some(("machines", sub)) => setup_machines(sub, &env),
    Some(("relay", sub)) => service::select(&env, value(sub, "machine")),
    _ => Err("expected machines or relay setup".into()),
  };
  outcome(result)
}

fn env(ctx: &Ctx) -> Env {
  let env = Env::process();
  ctx.config_dir.as_ref().map_or(env.clone(), |dir| {
    env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy())
  })
}

fn value<'a>(matches: &'a ArgMatches, name: &str) -> &'a str {
  matches.get_one::<String>(name).map_or("", String::as_str)
}

fn outcome(result: Result<Value, String>) -> Outcome {
  match result {
    Ok(value) => Outcome::data(value.to_string()),
    Err(error) => Outcome::failed(Exit::Failure, format!("delegation: {error}")),
  }
}

#[cfg(test)]
#[path = "tests/delegation_test.rs"]
mod tests;

fn delegate_submit() -> Command {
  Command::new("submit")
    .about("Persist and submit a task")
    .arg(
      Arg::new("key")
        .long("key")
        .required(true)
        .help("Caller-generated durable submission key"),
    )
    .arg(Arg::new("repo").long("repo").required(true))
    .arg(
      Arg::new("brief")
        .long("brief")
        .required(true)
        .help("Path to the task brief"),
    )
    .arg(
      Arg::new("agent")
        .long("agent")
        .required(true)
        .value_parser(["claude", "codex", "muse", "cursor", "opencode"]),
    )
    .arg(Arg::new("machine").long("machine"))
    .arg(Arg::new("group").long("group"))
    .arg(
      Arg::new("args")
        .num_args(0..)
        .trailing_var_arg(true)
        .allow_hyphen_values(true)
        .help("Agent flags after --"),
    )
}

fn delegate_report() -> Command {
  Command::new("report")
    .about("Write an attempt-bound worker outcome")
    .args(
      ["file", "attempt", "base", "commit"].map(|name| Arg::new(name).long(name).required(true)),
    )
    .arg(
      Arg::new("repo")
        .long("repo")
        .help("Worker repository; defaults to current directory"),
    )
    .arg(
      Arg::new("status")
        .long("status")
        .required(true)
        .value_parser(["done", "failed"]),
    )
    .arg(Arg::new("note").long("note").default_value(""))
}
