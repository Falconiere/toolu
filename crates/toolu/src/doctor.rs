//! `toolu doctor`: prove the native binary is reachable from an agent's
//! non-login command shell. #445 adds the other installation diagnostics.

use clap::{ArgMatches, Command};
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::process::commands::native_toolu_on_path;

/// Check the native binary selected by `sh -c 'command -v toolu'`.
pub fn command() -> Command {
  Command::new("doctor").about("Check that the agent command shell finds native toolu")
}

/// Run the command-shell reachability check.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match native_toolu_on_path() {
    Ok(Some(path)) => {
      if ctx.json {
        Outcome::data(
          json!({ "namespace": "doctor", "reachable": true, "path": path }).to_string(),
        )
      } else {
        Outcome::data(format!(
          "toolu doctor: native toolu reachable from a non-login shell at {}",
          path.display()
        ))
      }
    }
    Ok(None) => Outcome::failed(
      Exit::Failure,
      "toolu doctor: the non-login shell does not resolve a native toolu; check PATH or use the absolute path from SessionStart".to_owned(),
    ),
    Err(error) => Outcome::failed(Exit::Failure, format!("toolu doctor: {error}")),
  }
}

#[cfg(test)]
#[path = "tests/doctor_test.rs"]
mod tests;
