//! Explicit desktop focus for a herdr-core terminal, without launching an app.

use clap::{Arg, ArgMatches, Command};
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run as process};

/// The hub owns app guidance and the explicit `show` action.
pub const PLUGIN: &str = "toolu";

/// Desktop actions always require an explicit target.
pub fn command() -> Command {
  Command::new("herdr-app")
    .about("Show a worker in an already running Herdr App")
    .subcommand_required(true)
    .subcommand(
      Command::new("show")
        .about("Focus an existing local or remote terminal")
        .arg(Arg::new("terminal-address").required(true)),
    )
}

fn show(env: &Env, target: &str) -> Result<String, String> {
  if target.is_empty() || target.starts_with('-') {
    return Err("an explicit terminal address is required".into());
  }
  let terminal = toolu_mux::hc::Client::new(env)
    .get(target)
    .map_err(|error| format!("hc {}: {}", error.code, error.message))?;
  let address = toolu_mux::hc::address(&terminal).map_err(|error| error.message)?;
  let mut spec = Spec::new(["herdr-app", "do", "agent.focus"]);
  spec.argv.push(json!({"terminal": address}).to_string());
  spec.env = Some(env.clone());
  let output = process(&spec).map_err(|error| format!("herdr-app: {error:?}"))?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Err(format!("herdr-app: {}", output.stderr.trim()));
  }
  Ok(address.to_owned())
}

/// Resolve through hc, then focus through the app's existing control socket.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let Some(("show", sub)) = matches.subcommand() else {
    return Outcome::failed(
      Exit::Usage,
      "herdr-app show requires a terminal address".into(),
    );
  };
  let Some(target) = sub.get_one::<String>("terminal-address") else {
    return Outcome::failed(Exit::Usage, "terminal address is required".into());
  };
  match show(&Env::process(), target) {
    Ok(address) if ctx.json => Outcome::data(json!({"terminal":address}).to_string()),
    Ok(address) => Outcome::data(format!("Showing {address}")),
    Err(error) => Outcome::failed(Exit::Usage, error),
  }
}

#[cfg(test)]
#[path = "tests/herdr_app_test.rs"]
mod tests;
