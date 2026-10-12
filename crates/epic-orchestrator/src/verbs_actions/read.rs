//! `toolu epic jira issue get` and `toolu epic probe --check-evidence`.

use clap::{Arg, ArgMatches, Command};
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::{failure, flag, text, usage};
use crate::jira_cli::{issue_get, parse_key};
use crate::probe_evidence::check_evidence;
use crate::tracker::jira_client::JiraClient;

pub(super) fn jira_command() -> Command {
  let get = Command::new("get")
    .about("Print one Jira issue as JSON")
    .arg(
      Arg::new("key")
        .value_name("KEY")
        .required(true)
        .help("The issue key, e.g. PAY-12"),
    );
  Command::new("jira")
    .about("Read the epic's Jira issues")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      Command::new("issue")
        .about("Jira issues")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(get),
    )
}

pub(super) fn probe_command() -> Command {
  Command::new("probe")
    .about("Validate the committed host-lifecycle evidence")
    .arg(
      flag(
        "check-evidence",
        "Validate the evidence and print every gap",
      )
      .required(true),
    )
    .arg(
      Arg::new("file")
        .value_name("FILE")
        .help("The evidence file; default the committed record"),
    )
}

/// Print one Jira issue.
pub(super) fn jira(matches: &ArgMatches, env: &Env) -> Outcome {
  let Some(get) = matches
    .subcommand_matches("issue")
    .and_then(|issue| issue.subcommand_matches("get"))
  else {
    return usage("jira issue", crate::jira_cli::USAGE);
  };
  let key = match parse_key(text(get, "key").unwrap_or("")) {
    Ok(key) => key,
    Err(err) => return usage("jira issue", &err),
  };
  match JiraClient::new(env).and_then(|client| issue_get(&client, &key)) {
    Ok(text) => Outcome::data(text.trim_end().to_owned()),
    Err(err) => failure("jira issue", &err),
  }
}

/// Validate the evidence and print the summary.
pub(super) fn probe(matches: &ArgMatches) -> Outcome {
  let file = text(matches, "file");
  let body = match file.map(std::fs::read_to_string).transpose() {
    Ok(body) => body,
    Err(err) => return failure("probe-evidence: invalid", &err.to_string()),
  };
  match check_evidence(body.as_deref()) {
    Ok(summary) => Outcome::data(summary),
    Err(err) => Outcome::failed(toolu_protocol::exit::Exit::Failure, err),
  }
}

#[cfg(test)]
#[path = "tests/read_test.rs"]
mod tests;
