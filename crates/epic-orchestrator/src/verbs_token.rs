//! `toolu epic token`: rotate the status bearer token, or print the
//! dashboard URL that carries it in the fragment.

use std::net::SocketAddr;

use clap::{ArgMatches, Command};
use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::disk::read_value;
use crate::lock::live;
use crate::paths::Paths;
use crate::verbs::{env_of, failed};

/// The `token` subcommand tree.
pub(crate) fn command() -> Command {
  Command::new("token")
    .about("Manage the status bearer token")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(Command::new("new").about("Rotate the status bearer token in secrets.json"))
    .subcommand(
      Command::new("url")
        .about("Print the running engine's dashboard URL with the token in its fragment"),
    )
}

/// Dispatch `token new` or `token url`.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand_name() {
    Some("url") => url(ctx),
    _ => new(ctx),
  }
}

/// Rotate the status token without exposing its value.
fn new(ctx: &Ctx) -> Outcome {
  match secrets::rotate_status_token(&Roots::new(env_of(ctx), ctx.host)) {
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

/// Print `http://<address>/#token=<token>` for the running engine.
fn url(ctx: &Ctx) -> Outcome {
  match dashboard_url(&env_of(ctx)) {
    Ok(url) if ctx.json => Outcome::data(json!({ "url": url }).to_string()),
    Ok(url) => Outcome::data(url),
    Err(err) => failed("toolu epic token url", &err),
  }
}

/// The dashboard URL of the engine `env` selects, or why there is none.
fn dashboard_url(env: &Env) -> Result<String, String> {
  let paths = Paths::from_env(env);
  if !live(&paths.lock()) {
    return Err("no engine is running".to_owned());
  }
  let addr = read_value(&paths.http())?
    .get("addr")
    .and_then(Value::as_str)
    .and_then(|addr| addr.parse::<SocketAddr>().ok())
    .ok_or_else(|| "the engine has no status listener".to_owned())?;
  // The engine loads its secrets with no host, so read them the same way.
  let secrets = secrets::load(&Roots::new(env.clone(), None)).map_err(|err| err.to_string())?;
  let token = secrets
    .status_token()
    .ok_or_else(|| "no status token; run toolu epic token new".to_owned())?;
  Ok(format!(
    "{}#token={token}",
    crate::notify_notice::dashboard(addr)
  ))
}

#[cfg(test)]
#[path = "tests/verbs_token_test.rs"]
mod tests;
