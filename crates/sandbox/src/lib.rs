//! The sandbox plugin's crate: `toolu sandbox`, a Linux place to run builds,
//! gates and agent CLIs. On macOS it is a Lima VM that mounts only the project;
//! on Linux it is this machine. `setup` provisions the toolchain and prints how
//! to install the agent CLIs; `run` and `shell` replace toolu with the command,
//! so its terminal, output and exit status are the caller's own.

use clap::ArgMatches;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::process::exec::exec;

mod cli;
mod clone;
mod context;
mod facts;
mod host;
mod lima;
mod lima_argv;
mod lima_instance;
mod lima_process;
mod lima_setup;
mod lima_vm;
mod mounts;
mod present;
mod project;
mod provision;
mod settings;
mod state;
mod step;
mod vendors;

use context::Context;
use settings::Backend;
use step::{Failure, Launch, Step, describe};

/// The plugin this crate belongs to: `plugins/sandbox`.
pub const PLUGIN: &str = "sandbox";

/// The verbs that hand the terminal to a command, so print no document.
const LAUNCHES: [&str; 2] = ["run", "shell"];

/// The `toolu sandbox` namespace.
pub fn command() -> clap::Command {
  cli::command()
}

/// Run a `toolu sandbox` verb.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let Some((verb, args)) = matches.subcommand() else {
    return Failure::new(Exit::Usage, "a verb is required").outcome();
  };
  if ctx.json && LAUNCHES.contains(&verb) {
    let why =
      format!("--json does not apply to `{verb}`: the command's own output streams through");
    return Failure::new(Exit::Usage, why).outcome();
  }
  let step = Context::resolve(ctx).and_then(|context| match context.settings.backend {
    Backend::Host => host::step(verb, args, &context),
    Backend::Lima => lima::step(verb, args, &context),
  });
  match step {
    Ok(Step::Report(report)) => report.outcome(ctx.json),
    Ok(Step::Launch(launch)) => start(&launch),
    Err(failure) => failure.outcome(),
  }
}

/// Replace toolu with `launch`; returns only when it could not start.
fn start(launch: &Launch) -> Outcome {
  let err = exec(&launch.argv, Some(&launch.cwd), None);
  Failure::new(
    Exit::Unavailable,
    format!("cannot start the command: {}", describe(&err)),
  )
  .outcome()
}

#[cfg(test)]
#[path = "tests/git_support_test.rs"]
mod git_support;

#[cfg(test)]
#[path = "tests/lima_stub_test.rs"]
mod lima_stub;

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
