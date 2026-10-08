//! A verb's matches as the state and the questions Jev is asked.

use clap::ArgMatches;
use toolu_jev_client::{Error, Question, Questions, State};

/// What `execute` asks Jev.
pub(crate) struct Call {
  pub(crate) state: State,
  pub(crate) model: String,
  pub(crate) questions: Questions,
  pub(crate) raw: bool,
}

/// Why no call could be built.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CallError {
  /// The client refused a question.
  Client(Error),
  /// An input could not be read or combined.
  Input(String),
}

impl From<Error> for CallError {
  fn from(err: Error) -> CallError {
    CallError::Client(err)
  }
}

/// Whether `--state` or the questions source of the verb is `-`.
pub(crate) fn needs_stdin(matches: &ArgMatches) -> bool {
  let Some((name, sub)) = matches.subcommand() else {
    return false;
  };
  text(sub, "state") == Some("-") || (name == "ask" && text(sub, "questions") == Some("-"))
}

/// The call `matches` describes. `stdin` is what `-` reads.
pub(crate) fn build(matches: &ArgMatches, stdin: Option<&str>) -> Result<Call, CallError> {
  let Some((name, sub)) = matches.subcommand() else {
    return Err(CallError::Input("a verb is required".to_owned()));
  };
  if name == "ask" && text(sub, "state") == Some("-") && text(sub, "questions") == Some("-") {
    return Err(CallError::Input(
      "only one option can read stdin".to_owned(),
    ));
  }
  Ok(Call {
    state: state(sub, stdin)?,
    model: text(sub, "model")
      .unwrap_or(toolu_jev_client::DEFAULT_MODEL)
      .to_owned(),
    questions: questions(name, sub, stdin)?,
    raw: sub.try_get_one::<bool>("raw").ok().flatten() == Some(&true),
  })
}

fn text<'a>(sub: &'a ArgMatches, id: &str) -> Option<&'a str> {
  sub
    .try_get_one::<String>(id)
    .ok()
    .flatten()
    .map(String::as_str)
}

fn many<'a>(sub: &'a ArgMatches, id: &str) -> Vec<&'a str> {
  sub
    .try_get_many::<String>(id)
    .ok()
    .flatten()
    .map(|values| values.map(String::as_str).collect())
    .unwrap_or_default()
}

fn state(sub: &ArgMatches, stdin: Option<&str>) -> Result<State, CallError> {
  let state = text(sub, "state").unwrap_or_default();
  if state == "-" {
    return Ok(State::structured(&read_stdin(stdin)?));
  }
  match state.strip_prefix('@') {
    Some(path) => Ok(State::structured(&read_file(path)?)),
    None => Ok(State::text(state)),
  }
}

fn questions(name: &str, sub: &ArgMatches, stdin: Option<&str>) -> Result<Questions, CallError> {
  if name == "ask" {
    let source = text(sub, "questions").unwrap_or_default();
    let body = if source == "-" {
      read_stdin(stdin)?
    } else {
      read_file(source)?
    };
    return Ok(Questions::parse(body.trim_end_matches('\n'))?);
  }
  let instructions = text(sub, "instructions").unwrap_or_default();
  let question = match name {
    "noul" => Question::noul(instructions, text(sub, "true"), text(sub, "false")),
    "choice" => choice(sub, instructions),
    "score" => Question::score(instructions, &many(sub, "level")),
    other => return Err(CallError::Input(format!("unknown verb {other}"))),
  }?;
  Ok(Questions::single(text(sub, "id").unwrap_or("q"), question))
}

/// `KEY` or `KEY=DESC`, split at the first `=`.
fn choice(sub: &ArgMatches, instructions: &str) -> Result<Question, Error> {
  let options: Vec<(&str, Option<&str>)> = many(sub, "option")
    .into_iter()
    .map(|option| match option.split_once('=') {
      Some((key, description)) => (key, Some(description)),
      None => (option, None),
    })
    .collect();
  Question::choice(instructions, &options)
}

fn read_stdin(stdin: Option<&str>) -> Result<String, CallError> {
  stdin
    .map(str::to_owned)
    .ok_or_else(|| CallError::Input("cannot read stdin".to_owned()))
}

fn read_file(path: &str) -> Result<String, CallError> {
  std::fs::read_to_string(path).map_err(|_err| CallError::Input(format!("cannot read {path}")))
}

#[cfg(test)]
#[path = "tests/call_test.rs"]
mod tests;
