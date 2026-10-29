//! The verbs that read or write GitHub: `tick`, `wait`, `collect`, `reply`
//! and `resolve`.

use std::path::Path;
use std::time::Duration;

use clap::ArgMatches;
use serde_json::Value;
use toolu_github::{Client, Config};
use toolu_runtime::cli::Outcome;

use super::{World, now, number, path, required, text};
use crate::collect::{self, failure};
use crate::error::{Failure, Result};
use crate::pattern::{is, pattern};
use crate::schedule::iso;
use crate::state::{sibling, slot, valid_repo, write_json};
use crate::tick::{self, Source};
use crate::wait::{self, Plan, SystemClock};
use crate::writes::{self, Slot, Target};

pattern!(THREAD_ID, r"^[A-Za-z0-9_=-]+$");

/// GitHub's largest comment body.
const MAX_BODY: usize = 65_536;

/// `--repo` and `--pr`.
fn pull_request(matches: &ArgMatches) -> Result<(&str, u64)> {
  let repo = text(matches, "repo").filter(|repo| valid_repo(repo));
  let repo = repo.ok_or_else(|| Failure::usage("--repo <owner/repo> required"))?;
  let number = number(matches, "pr").ok_or_else(|| Failure::usage("--pr <n> required"))?;
  Ok((repo, number))
}

/// The client settings, with `--timeout` as the per-call deadline.
fn config(matches: &ArgMatches, world: &World<'_>) -> Result<Config> {
  let mut config = world
    .github
    .as_ref()
    .map_err(|err| failure("config", err))?
    .clone();
  if let Some(seconds) = number(matches, "timeout") {
    config.http.timeout = Duration::from_secs(seconds);
  }
  Ok(config)
}

fn client(matches: &ArgMatches, world: &World<'_>, source: &str) -> Result<Client> {
  Client::new(config(matches, world)?, world.env).map_err(|err| failure(source, &err))
}

fn page_size(matches: &ArgMatches) -> u64 {
  number(matches, "page-size").unwrap_or(100)
}

/// One check: GitHub, or the recorded `--snapshot-in`.
pub(super) fn tick(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let (repo, number) = pull_request(matches)?;
  let now = text(matches, "now").map_or_else(now, str::to_owned);
  let request = tick::Request {
    repo,
    number,
    state_file: required(matches, "state-file")?,
    snapshot_out: path(matches, "snapshot-out"),
    now: &now,
    scheduled: false,
  };
  let source = match path(matches, "snapshot-in") {
    Some(file) => Source::File(file),
    None => Source::GitHub {
      config: config(matches, world)?,
      page_size: page_size(matches),
    },
  };
  tick::tick(&request, source, world.env).map(|ticked| ticked.result)
}

/// The state's next scheduled check, if it has one.
fn next_check(state_file: &Path) -> Option<u64> {
  let text = std::fs::read_to_string(state_file).ok()?;
  let state = serde_json::from_str::<Value>(&text).ok()?;
  state.pointer("/schedule/nextCheckAt")?.as_u64()
}

/// Scheduled checks until one needs the agent, or the waiting document.
pub(super) fn wait(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let (repo, pr) = pull_request(matches)?;
  let state_file = required(matches, "state-file")?;
  let config = config(matches, world)?;
  let page_size = page_size(matches);
  let max_seconds =
    number(matches, "max-seconds").unwrap_or_else(|| wait::default_seconds(world.host.name()));
  let slot = slot(repo, pr);
  let (state_path, result_path) = (
    state_file.display().to_string(),
    sibling(state_file, ".result.json").display().to_string(),
  );
  let plan = Plan {
    max_seconds,
    next_check: next_check(state_file),
    slot: &slot,
    state_path: &state_path,
    result_path: &result_path,
  };
  let mut check = |at: u64| {
    let now = iso(at);
    let request = tick::Request {
      repo,
      number: pr,
      state_file,
      snapshot_out: None,
      now: &now,
      scheduled: true,
    };
    let source = Source::GitHub {
      config: config.clone(),
      page_size,
    };
    tick::tick(&request, source, world.env).map(|ticked| ticked.result)
  };
  wait::wait(&SystemClock, &plan, &mut check)
}

/// One snapshot: on stdout, or written to `--out` with its path printed.
pub(super) fn collect(matches: &ArgMatches, world: &World<'_>) -> Outcome {
  let collected = pull_request(matches).and_then(|(repo, number)| {
    let now = now();
    let request = collect::Request {
      repo,
      number,
      page_size: page_size(matches),
      now: &now,
    };
    collect::collect(&client(matches, world, "collect")?, &request)
  });
  let snapshot = match collected {
    Ok(collected) => collected.snapshot,
    Err(failure) => return failure.outcome(),
  };
  let Some(out) = path(matches, "out") else {
    return Outcome::data(snapshot.to_string());
  };
  match write_json(out, &snapshot, "snapshot_out") {
    Ok(()) => Outcome::data(out.display().to_string()),
    Err(failure) => failure.outcome(),
  }
}

fn reply_body(matches: &ArgMatches) -> Result<String> {
  let file = required(matches, "body-file")?;
  let body = std::fs::read_to_string(file)
    .map_err(|_error| Failure::usage("--body-file <existing file> required"))?;
  if body.is_empty() {
    return Err(Failure::usage("reply body is empty"));
  }
  if body.len() > MAX_BODY {
    return Err(Failure::usage("reply body exceeds the 65536-byte limit"));
  }
  Ok(body)
}

fn target(matches: &ArgMatches) -> Result<Target<'_>> {
  let id = |name: &str, kind: &str| {
    let message = format!("--{name} <id> required for --kind {kind}");
    number(matches, name).ok_or_else(|| Failure::usage(message))
  };
  match text(matches, "kind").unwrap_or_default() {
    "thread" => {
      let thread = text(matches, "thread").filter(|thread| is(&THREAD_ID, thread));
      let message = "--thread <graphqlId> required for --kind thread";
      Ok(Target::Thread {
        thread: thread.ok_or_else(|| Failure::usage(message))?,
        root: id("root-comment", "thread")?,
        in_reply_to: id("in-reply-to", "thread")?,
      })
    }
    "conversation" => Ok(Target::Conversation(id("comment-id", "conversation")?)),
    "review" => Ok(Target::Review(id("review-id", "review")?)),
    _ => Err(Failure::usage(
      "--kind must be thread, conversation or review",
    )),
  }
}

/// The one reply to a thread, conversation comment or review.
pub(super) fn reply(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let state_file = required(matches, "state-file")?;
  let body = reply_body(matches)?;
  let to = target(matches)?;
  let now = now();
  let slot = Slot {
    path: state_file,
    env: world.env,
    now: &now,
  };
  writes::reply(&client(matches, world, "reply")?, &slot, &to, &body)
}

/// Resolve a review thread and confirm it.
pub(super) fn resolve(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let state_file = required(matches, "state-file")?;
  let thread = text(matches, "thread").filter(|thread| is(&THREAD_ID, thread));
  let thread = thread.ok_or_else(|| Failure::usage("--thread <graphqlId> required"))?;
  let now = now();
  let slot = Slot {
    path: state_file,
    env: world.env,
    now: &now,
  };
  writes::resolve(
    &client(matches, world, "resolve")?,
    &slot,
    thread,
    world.pause,
  )
}

#[cfg(test)]
#[path = "tests/github_test.rs"]
mod tests;
