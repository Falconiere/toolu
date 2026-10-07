//! The temporary Bun bridge for `.js` registry modules (#418; #440 removes it).
//! A run of consecutive `.js` modules shares one `bun` process, which imports
//! each in walk order and prints one result line per module, stopping after a
//! deny (or a block after the tool). A module that hangs, exits the process or
//! garbles its line fails alone; the modules after it run in a fresh process.

pub(crate) mod bun;
pub(crate) mod runner;

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use toolu_protocol::decision::Decision;
use toolu_runtime::process::{RunError, Spec, Wait, run};

use crate::dispatch::MAX_OUTPUT_BYTES;
use runner::{MARK, RUNNER, Request, request_text};

use super::Entry;

/// What the runner said about one module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
  /// It decided.
  Decision(Decision),
  /// It failed: the message for `toolu-registry: module <file> failed: …`.
  Error(String),
}

/// What a batch did.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Batch {
  /// One line per module reached, in order: `(file, line)`.
  pub(crate) lines: Vec<(String, Line)>,
  /// Bun's stderr, from every process the batch used.
  pub(crate) stderr: String,
  /// Bun could not be started for the modules after `lines`.
  pub(crate) no_bun: bool,
}

/// Where and how long a batch runs.
pub(crate) struct Launch<'l> {
  pub(crate) bun: &'l Path,
  pub(crate) env: &'l toolu_runtime::env::Env,
  pub(crate) cwd: &'l Path,
  /// The deadline of one module; a batch gets one per module.
  pub(crate) module_timeout: Duration,
}

/// The lines `stdout` holds for `modules`, in order; it stops at the first line
/// that is unreadable or names another module.
fn parse_lines(stdout: &str, modules: &[&Entry]) -> Vec<(String, Line)> {
  let mut lines = Vec::new();
  let marked = stdout
    .split('\n')
    .filter_map(|line| line.strip_prefix(MARK));
  for (text, entry) in marked.zip(modules) {
    let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(text) else {
      break;
    };
    if fields.get("file").and_then(Value::as_str) != Some(entry.file.as_str()) {
      break;
    }
    let line = match (
      fields.get("decision"),
      fields.get("error").and_then(Value::as_str),
    ) {
      (Some(decision), _) => match serde_json::from_value::<Decision>(decision.clone()) {
        Ok(decision) => Line::Decision(decision),
        Err(_) => break,
      },
      (None, Some(error)) => Line::Error(error.to_owned()),
      (None, None) => break,
    };
    lines.push((entry.file.clone(), line));
  }
  lines
}

/// Whether `line` ends the walk: a decision of the request's stop kind.
fn stops(line: &Line, stop: &str) -> bool {
  match line {
    Line::Decision(Decision::Deny { .. }) => stop == "deny",
    Line::Decision(Decision::Block { .. }) => stop == "post_block",
    Line::Decision(_) | Line::Error(_) => false,
  }
}

/// The `bun` process for `modules`: one module deadline per module.
fn spec_for(launch: &Launch<'_>, request: &Request<'_>, modules: &[&Entry]) -> Spec {
  let count = u32::try_from(modules.len()).unwrap_or(u32::MAX);
  let mut spec = Spec::new([
    launch.bun.to_string_lossy().into_owned(),
    "--no-install".to_owned(),
    "-e".to_owned(),
    RUNNER.to_owned(),
  ]);
  spec.env = Some(launch.env.clone());
  spec.cwd = Some(launch.cwd.to_path_buf());
  spec.stdin = request_text(request, modules).into_bytes();
  spec.timeout = launch.module_timeout.saturating_mul(count);
  spec.max_output_bytes = MAX_OUTPUT_BYTES;
  spec.wait = Wait::Streams;
  spec
}

/// Why the module in flight got no line.
fn failure(timed_out: bool, exit_code: i32, deadline: Duration) -> String {
  if timed_out {
    format!("timed out after {} ms", deadline.as_millis())
  } else if exit_code != 0 {
    format!("bridge exited {exit_code}")
  } else {
    "bridge output unreadable".to_owned()
  }
}

/// Run `modules` through Bun, re-running the rest after a module that failed the process.
pub(crate) fn run_batch(launch: &Launch<'_>, request: &Request<'_>, modules: &[&Entry]) -> Batch {
  let mut batch = Batch::default();
  let mut rest = modules;
  while !rest.is_empty() {
    let spec = spec_for(launch, request, rest);
    let deadline = spec.timeout;
    let output = match run(&spec) {
      Ok(output) => output,
      Err(RunError::Spawn(_)) => {
        batch.no_bun = true;
        return batch;
      }
      Err(err) => {
        let failure = format!("bridge failed: {err:?}");
        batch.lines.extend(
          rest
            .iter()
            .map(|entry| (entry.file.clone(), Line::Error(failure.clone()))),
        );
        return batch;
      }
    };
    batch.stderr.push_str(&output.stderr);
    let lines = parse_lines(&output.stdout, rest);
    let done = lines.len();
    for (file, line) in lines {
      let stop = stops(&line, request.stop);
      batch.lines.push((file, line));
      if stop {
        return batch;
      }
    }
    let Some((failed, after)) = rest.get(done..).and_then(<[&Entry]>::split_first) else {
      return batch;
    };
    let reason = failure(output.timed_out, output.exit_code, deadline);
    batch.lines.push((failed.file.clone(), Line::Error(reason)));
    rest = after;
  }
  batch
}

#[cfg(test)]
#[path = "tests/bridge_test.rs"]
mod tests;
