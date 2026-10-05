//! `cargo xtask <task>`: workspace tasks for the toolu Rust rebuild (epic #402).
//!
//! Exit codes: 0 clean, 1 findings, 2 usage, setup or missing-tool error.

mod coverage;
mod data;
mod gate;
mod gate_change;
mod guardrails;
mod layers;
mod layers_check;
mod lexer;
mod measure;
mod metadata;
mod options;
mod output;
mod reach;
mod source;
mod unused_pub;
mod workspace;

use std::process::ExitCode;

use options::Options;

/// What a task concluded when it ran to the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
  /// Nothing to report.
  Clean,
  /// Findings were printed.
  Findings,
}

/// A task: parsed options in, a verdict or a setup error out.
type Task = fn(&Options) -> Result<Verdict, String>;

/// Every `cargo xtask` task. The behaviour inventory discovers its entries here.
const TASKS: &[(&str, Task)] = &[
  ("gate", gate::run),
  ("guardrails", guardrails::run),
  ("check-layers", layers_check::run),
  ("check-reach", reach::run),
  ("check-unused-pub", unused_pub::run),
  ("check-gate-change", gate_change::run),
  ("check-coverage", coverage::run),
  ("measure", measure::run),
];

// `\x20` keeps the second line's indent: a `\` continuation strips leading spaces.
const USAGE: &str = "usage: cargo xtask <task> [--root DIR] [--base REF] [--title TEXT] \
  [--only STEP]... [FILE]\n\
  \x20      cargo xtask measure --out FILE -- COMMAND [ARG]...\n\
  tasks: gate, guardrails, check-layers, check-reach, check-unused-pub, check-gate-change, \
  check-coverage, measure";

/// Run the task named by `args[0]` and map its outcome to an exit code.
fn run(args: &[String]) -> ExitCode {
  let result = match args.split_first() {
    Some((name, rest)) => match TASKS.iter().find(|(task, _)| task == name) {
      Some((_, task)) => Options::parse(rest)
        .and_then(|options| options.for_task(name))
        .map_err(|err| format!("{err}\n{USAGE}"))
        .and_then(|options| task(&options)),
      None => Err(format!("unknown task {name}\n{USAGE}")),
    },
    None => Err(USAGE.to_owned()),
  };
  match result {
    Ok(Verdict::Clean) => ExitCode::SUCCESS,
    Ok(Verdict::Findings) => ExitCode::from(1),
    Err(message) => {
      output::error(&format!("xtask: {message}"));
      ExitCode::from(2)
    }
  }
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  run(&args)
}

#[cfg(test)]
#[path = "tests/main_test.rs"]
mod tests;
