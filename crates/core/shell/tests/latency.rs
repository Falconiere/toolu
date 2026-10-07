//! Latency (#416 AC-6): parse and walk, with the git and write helpers a gate
//! calls, stay under 0.1 ms at p99 over the real commands of
//! `fixtures/shell/{bats-parity,issue-283}.json`, the set `bun run bench:shell`
//! times, in a release build (`cargo test --release -p toolu-shell --test
//! latency`). A debug or instrumented build only checks a 100× smoke ceiling.

#[path = "helpers/cases.rs"]
mod cases;

use std::hint::black_box;
use std::time::{Duration, Instant};

use toolu_shell::analyze;
use toolu_shell::git::{push_targets, runs_git_subcommand};
use toolu_shell::writes::write_targets;

const ROUNDS: usize = 200;

/// One analysis as a gate runs it.
fn analyze_once(source: &str) -> Duration {
  let started = Instant::now();
  let analysis = analyze(source);
  black_box(runs_git_subcommand(&analysis, "push"));
  black_box(push_targets(&analysis).len());
  black_box(write_targets(&analysis).len());
  started.elapsed()
}

/// Every command of the two real-command fixtures, as `fixtureCommands` in
/// `tooling/src/bench-shell.ts` reads them.
fn commands() -> cases::Res<Vec<String>> {
  let mut found = Vec::new();
  for rel in [
    "fixtures/shell/bats-parity.json",
    "fixtures/shell/issue-283.json",
  ] {
    for case in cases::cases(rel)? {
      let command = cases::field(&case, "command")?;
      found.push(
        command
          .replace("$TMP", "/tmp/t")
          .replace("$MKTEMP", "/tmp/m"),
      );
    }
  }
  Ok(found)
}

#[test]
fn p99_over_the_real_commands_is_within_budget() {
  let inputs = commands().unwrap();
  let mut samples: Vec<Duration> = Vec::with_capacity(inputs.len() * ROUNDS);
  for _ in 0..ROUNDS {
    samples.extend(inputs.iter().map(|input| analyze_once(input)));
  }
  samples.sort();
  let at = |percent: usize| samples[samples.len() * percent / 100];
  let (p50, p99) = (at(50), at(99));
  let budget = if cfg!(debug_assertions) {
    Duration::from_millis(10)
  } else {
    Duration::from_micros(100)
  };
  println!(
    "shell analysis over {} samples: p50 {p50:?}, p99 {p99:?}, budget {budget:?}",
    samples.len()
  );
  assert!(p99 <= budget, "p99 {p99:?} exceeds {budget:?}");
}
