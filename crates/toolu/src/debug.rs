//! Native summaries for captured logs, traces and failing test transcripts.

mod log;
mod stack;
mod testfail;

use std::fs;
use std::path::PathBuf;

use clap::{Arg, ArgMatches, Command, value_parser};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

fn verb(name: &'static str, about: &'static str) -> Command {
  Command::new(name).about(about).arg(
    Arg::new("file")
      .long("file")
      .value_name("PATH")
      .value_parser(value_parser!(PathBuf))
      .help("Read a file instead of standard input"),
  )
}

/// The `toolu debug` namespace.
pub fn command() -> Command {
  Command::new("debug")
    .about("Count or summarize logs, stacks and failing tests")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(verb("io", "Count input bytes and lines"))
    .subcommand(verb("log", "Summarize error lines and a log tail"))
    .subcommand(verb("stack", "Show app frames and collapse runtime frames"))
    .subcommand(verb(
      "testfail",
      "Summarize failed tests, errors and locations",
    ))
}

fn input(matches: &ArgMatches, ctx: &Ctx, name: &str) -> Result<Vec<u8>, Outcome> {
  if let Some(path) = matches.get_one::<PathBuf>("file") {
    return fs::read(path).map_err(|error| {
      Outcome::failed(
        Exit::Usage,
        format!(
          "toolu debug {name}: cannot read file: {}: {error}",
          path.display()
        ),
      )
    });
  }
  ctx
    .stdin
    .as_ref()
    .map(|text| text.as_bytes().to_vec())
    .ok_or_else(|| {
      Outcome::failed(
        Exit::Usage,
        format!("toolu debug {name}: cannot read standard input"),
      )
    })
}

fn records(text: &str) -> Vec<&str> {
  if text.is_empty() {
    return Vec::new();
  }
  let mut lines: Vec<&str> = text.split('\n').collect();
  if text.ends_with('\n') {
    lines.pop();
  }
  lines
}

fn io(bytes: &[u8], json: bool) -> String {
  let lines = if bytes.is_empty() {
    0
  } else {
    bytes.split(|byte| *byte == b'\n').count() - usize::from(bytes.last() == Some(&b'\n'))
  };
  if json {
    serde_json::json!({"bytes": bytes.len(), "lines": lines}).to_string()
  } else {
    format!("bytes: {}\nlines: {lines}", bytes.len())
  }
}

/// Read one nonnegative output cap, falling back to its documented default.
pub(super) fn cap(env: &Env, name: &str, default: usize) -> usize {
  env
    .get(name)
    .filter(|value| !value.is_empty())
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(default)
}

/// Run a native debug verb.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let Some((name, sub)) = matches.subcommand() else {
    return Outcome::failed(Exit::Usage, "toolu debug: a verb is required".to_owned());
  };
  let bytes = match input(sub, ctx, name) {
    Ok(bytes) => bytes,
    Err(failed) => return failed,
  };
  let env = Env::process();
  let text = String::from_utf8_lossy(&bytes);
  let lines = records(&text);
  let summary = match name {
    "io" => io(&bytes, ctx.json),
    "log" => log::summarize(&lines, ctx.json, &env),
    "stack" => stack::summarize(&lines, ctx.json, &env),
    "testfail" => testfail::summarize(&lines, ctx.json, &env),
    _ => return Outcome::failed(Exit::Usage, format!("toolu debug: unknown verb {name}")),
  };
  Outcome::data(summary)
}

#[cfg(test)]
#[path = "tests/debug_test.rs"]
mod tests;
