//! `toolu code-review eval`: the explicit paired-comparison entry. Without `--live`
//! every call is answered from the replay store and nothing is sent, so no key is
//! needed; `--live` requires the repository's opt-in and `OPENROUTER_API_KEY`, and
//! without them exits 69 unverified before any request.

use std::path::{Path, PathBuf};

use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::arms::{ArmChoice, Mode, Plan};
use super::fixtures::Fixtures;
use super::replay::Store;
use super::runner::run_all;
use super::setting::Setting;
use super::suite::{SuiteReport, Verdict};
use crate::settings::{self, ENABLE_HINT};

/// The `eval` verb.
pub(crate) fn command() -> Command {
  Command::new("eval")
    .about("Compare a baseline and a Jev-enhanced review against committed expectations; replays recorded responses unless --live")
    .arg(
      Arg::new("cases")
        .long("cases")
        .value_name("DIR")
        .required(true)
        .help("The case set: expectations.json and cases/<id>/files/{base,tip}"),
    )
    .arg(
      Arg::new("arm")
        .long("arm")
        .value_name("ARM")
        .value_parser(PossibleValuesParser::new(ArmChoice::NAMES))
        .default_value("both")
        .help("Which arms to run: the Jev judge off, on, or both"),
    )
    .arg(
      Arg::new("effort")
        .long("effort")
        .value_name("LEVEL")
        .action(ArgAction::Append)
        .value_parser(PossibleValuesParser::new(Setting::ALL.map(Setting::name)))
        .help(
          "Effort setting to run; repeatable for calibration. none is the known-weak baseline (lowest level, never judged); default: auto",
        ),
    )
    .arg(
      Arg::new("live")
        .long("live")
        .action(ArgAction::SetTrue)
        .help(
          "Call OpenRouter and Jev for real (needs the repository opt-in and OPENROUTER_API_KEY) and record every exchange; without it nothing is sent",
        ),
    )
    .arg(
      Arg::new("replay-dir")
        .long("replay-dir")
        .value_name("DIR")
        .help("Where recorded responses are read (replay) or written (--live); default: <cases>/replay"),
    )
    .arg(
      Arg::new("report")
        .long("report")
        .value_name("PATH")
        .help("Also write the JSON report to this file"),
    )
}

fn usage(message: &str) -> Outcome {
  Outcome::failed(Exit::Usage, format!("code-review eval: {message}"))
}

/// Why a live run may not send anything, when it may not.
fn live_blocked(env: &Env, ctx: &Ctx) -> Option<String> {
  if env
    .get("OPENROUTER_API_KEY")
    .is_none_or(|key| key.trim().is_empty())
  {
    return Some("OPENROUTER_API_KEY is missing".to_owned());
  }
  let roots = Roots::new(env.clone(), ctx.host);
  let project = roots.project_root(None);
  (!settings::load(&roots, project.as_deref()).enabled).then(|| ENABLE_HINT.to_owned())
}

fn selected<'a>(verb: &'a ArgMatches, name: &str) -> Option<&'a str> {
  verb.get_one::<String>(name).map(String::as_str)
}

/// The `--effort` settings in first-seen order; a repeat would bill and count twice.
fn settings_of(verb: &ArgMatches) -> Vec<Setting> {
  let mut chosen: Vec<Setting> = Vec::new();
  for setting in verb
    .get_many::<String>("effort")
    .into_iter()
    .flatten()
    .filter_map(|name| Setting::parse(name))
  {
    if !chosen.contains(&setting) {
      chosen.push(setting);
    }
  }
  if chosen.is_empty() {
    vec![Setting::Auto]
  } else {
    chosen
  }
}

fn present(suite: &SuiteReport, verb: &ArgMatches, ctx: &Ctx) -> Outcome {
  let document = match serde_json::to_string(suite) {
    Ok(document) => document,
    Err(error) => return Outcome::failed(Exit::Failure, format!("code-review eval: {error}")),
  };
  let written = selected(verb, "report").and_then(|path| {
    std::fs::write(Path::new(path), &document)
      .err()
      .map(|error| format!("code-review eval: {path}: {error}"))
  });
  let stdout = if ctx.json { document } else { suite.summary() };
  // A paid live run's results still reach stdout when the report file cannot be written.
  if let Some(error) = written {
    return Outcome {
      exit: Exit::Failure,
      stdout: Some(stdout),
      stderr: Some(error),
    };
  }
  match suite.verdict() {
    Verdict::Satisfied => Outcome::data(stdout),
    Verdict::Missed => Outcome {
      exit: Exit::Blocked,
      stdout: Some(stdout),
      stderr: Some("code-review eval: expectations missed".to_owned()),
    },
    Verdict::Unverified => Outcome {
      exit: Exit::Unavailable,
      stdout: Some(stdout),
      stderr: Some("code-review eval: unverified; not a pass".to_owned()),
    },
  }
}

/// Run `eval`.
pub(crate) fn run(verb: &ArgMatches, ctx: &Ctx, env: &Env) -> Outcome {
  let Some(cases) = selected(verb, "cases").map(PathBuf::from) else {
    return usage("--cases is required");
  };
  let fixtures = match Fixtures::load(&cases) {
    Ok(fixtures) => fixtures,
    Err(message) => return usage(&message),
  };
  let live = verb.get_flag("live");
  if live && let Some(why) = live_blocked(env, ctx) {
    return Outcome::failed(
      Exit::Unavailable,
      format!("code-review eval: {why}; nothing was sent, unverified"),
    );
  }
  let dir = selected(verb, "replay-dir").map_or_else(|| cases.join("replay"), PathBuf::from);
  let store = Store::new(&dir);
  let mode = if live {
    Mode::Live(Some(store))
  } else {
    Mode::Replay(store)
  };
  let settings = settings_of(verb);
  let plan = Plan {
    env,
    arms: selected(verb, "arm")
      .and_then(ArmChoice::parse)
      .unwrap_or(ArmChoice::Both),
    settings: &settings,
    mode: &mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  };
  match run_all(&fixtures, &plan) {
    Ok(suite) => present(&suite, verb, ctx),
    Err(message) => Outcome::failed(Exit::Failure, format!("code-review eval: {message}")),
  }
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
