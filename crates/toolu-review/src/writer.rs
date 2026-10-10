//! CLI input and result for the v2 writer. The file recipe lives in `state`.

use clap::ArgMatches;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;

use crate::state::{Input, write};

fn value<'a>(matches: &'a ArgMatches, name: &str) -> Option<&'a str> {
  matches.get_one::<String>(name).map(String::as_str)
}

fn input(matches: &ArgMatches) -> Result<Input<'_>, Outcome> {
  let count_text = value(matches, "findings-count").unwrap_or_default();
  let count = count_text
    .bytes()
    .all(|byte| byte.is_ascii_digit())
    .then(|| count_text.parse::<u64>().ok())
    .flatten()
    .ok_or_else(|| {
      Outcome::failed(
        Exit::Usage,
        "review: --findings-count must be an integer".to_owned(),
      )
    })?;
  let json = |name: &str, default: &str| -> Result<Ordered, Outcome> {
    Ordered::parse(value(matches, name).unwrap_or(default))
      .map_err(|_error| Outcome::failed(Exit::Failure, format!("review: bad --{name} JSON")))
  };
  Ok(Input {
    findings_count: count,
    reviewers: json("reviewers", "[\"toolu-review:review\"]")?,
    findings: json("findings", "[]")?,
    repo: value(matches, "repo").unwrap_or("."),
    branch: value(matches, "branch"),
    reviewed_files: value(matches, "reviewed-files"),
  })
}

/// Run the writer with a snapshot of the host environment.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Outcome {
  let Some(("write-state", verb)) = matches.subcommand() else {
    return Outcome::failed(Exit::Usage, "review: a verb is required".to_owned());
  };
  let input = match input(verb) {
    Ok(input) => input,
    Err(outcome) => return outcome,
  };
  match write(&input, ctx, env) {
    Ok(path) if ctx.json => {
      Outcome::data(serde_json::json!({"path":path.display().to_string()}).to_string())
    }
    Ok(path) => Outcome::data(path.display().to_string()),
    Err(message) => Outcome::failed(Exit::Failure, format!("review: {message}")),
  }
}

#[cfg(test)]
#[path = "tests/writer_test.rs"]
mod tests;
