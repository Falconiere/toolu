//! The golden cases of the three quality plugins (#265, #266, #267), replayed
//! through the built `toolu`: each case runs in a real sandbox repository with
//! the plugin registered by `toolu <plugin> hook register`, and every tool call
//! is dispatched by `toolu hook post-tools`. The stdout, stderr, exit code and
//! project state (gate file, telemetry) must equal what the bash module gave at
//! the golden's base commit. A named deviation must instead show what the port
//! does right. These tests need `ast-grep` on `PATH`.

use std::collections::BTreeSet;
use std::error::Error;

type Res<T> = Result<T, Box<dyn Error>>;

#[path = "helpers/golden_quality_cases.rs"]
mod cases;
#[path = "helpers/golden_quality_compare.rs"]
mod compare;
#[path = "helpers/golden_quality_gate.rs"]
mod gate;
#[path = "helpers/golden_quality_proc.rs"]
mod proc;
#[path = "helpers/golden_quality_replay.rs"]
mod replay;
#[path = "helpers/golden_quality_sandbox.rs"]
mod sandbox;
#[path = "helpers/golden_quality_setup.rs"]
mod setup;
#[path = "helpers/golden_quality_step.rs"]
mod step;

use cases::{key, load_cases, load_excluded, load_golden};
use replay::{Plugin, replay_all};

const TS: Plugin = Plugin {
  name: "ts-quality",
  installed: &["toolu@toolu", "ts-quality@toolu", "rust-quality@toolu"],
  worktree: None,
};

const PYTHON: Plugin = Plugin {
  name: "python-quality",
  installed: &["toolu@toolu", "python-quality@toolu", "rust-quality@toolu"],
  worktree: Some((
    "flow: a file in a linked worktree is checked",
    "Forbidden suppression in",
  )),
};

const RUST: Plugin = Plugin {
  name: "rust-quality",
  installed: &["toolu@toolu", "rust-quality@toolu", "ts-quality@toolu"],
  worktree: Some((
    "flow: a file in a linked worktree is still checked",
    "Forbidden lint suppression",
  )),
};

/// Golden cases that depend on behaviour that no longer exists, each with its reason.
const NOT_REPLAYED: &[(&str, &str)] = &[];

/// What a plugin's corpus holds, so a dropped case fails the count.
struct Shape {
  /// Fixture file stem: `ts`, `python` or `rust`.
  file: &'static str,
  /// Cases in the corpus.
  cases: usize,
  /// Case and host pairs, one golden entry each.
  pairs: usize,
  /// Pairs that are named deviations.
  deviations: usize,
  /// Pairs that are the linked-worktree case.
  worktrees: usize,
  /// Cases that link a Node tool, replayed alone after the others.
  alone: usize,
}

const TS_SHAPE: Shape = Shape {
  file: "ts",
  cases: 120,
  pairs: 121,
  deviations: 1,
  worktrees: 0,
  alone: 3,
};
const PYTHON_SHAPE: Shape = Shape {
  file: "python",
  cases: 96,
  pairs: 97,
  deviations: 1,
  worktrees: 1,
  alone: 0,
};
const RUST_SHAPE: Shape = Shape {
  file: "rust",
  cases: 119,
  pairs: 120,
  deviations: 1,
  worktrees: 1,
  alone: 0,
};

fn replay_plugin(plugin: &Plugin, shape: &Shape) -> Res<()> {
  let cases = load_cases(shape.file)?;
  let golden = load_golden(shape.file)?;
  ensure(cases.len() == shape.cases, || {
    format!("{} cases, expected {}", cases.len(), shape.cases)
  })?;
  ensure(golden.cases.len() == shape.pairs, || {
    format!(
      "{} golden entries, expected {}",
      golden.cases.len(),
      shape.pairs
    )
  })?;
  let alone = cases.iter().filter(|case| case.links_a_node_tool()).count();
  ensure(alone == shape.alone, || {
    format!("{alone} cases link a Node tool, expected {}", shape.alone)
  })?;
  let report = replay_all(plugin, &cases, &golden)?;
  ensure(report.failures.is_empty(), || {
    format!(
      "{} of {} replays differ:\n{}",
      report.failures.len(),
      report.replayed,
      report.failures.join("\n\n")
    )
  })?;
  ensure(report.replayed + NOT_REPLAYED.len() == shape.pairs, || {
    format!("{} replayed, expected {}", report.replayed, shape.pairs)
  })?;
  ensure(report.deviations == shape.deviations, || {
    format!("{} deviations", report.deviations)
  })?;
  ensure(report.worktrees == shape.worktrees, || {
    format!("{} worktree cases", report.worktrees)
  })
}

fn ensure(ok: bool, why: impl FnOnce() -> String) -> Res<()> {
  if ok { Ok(()) } else { Err(why().into()) }
}

/// The golden holds exactly the corpus's cases, and each capture says what its case says.
fn corpus(shape: &Shape) -> Res<()> {
  let cases = load_cases(shape.file)?;
  let golden = load_golden(shape.file)?;
  ensure(!golden.base.is_empty(), || {
    "the golden names no base commit".to_owned()
  })?;
  let keys: Vec<String> = cases
    .iter()
    .flat_map(|case| case.hosts().into_iter().map(|host| key(case, host)))
    .collect();
  compare::same_keys(&keys, &golden)?;
  for case in &cases {
    for host in case.hosts() {
      let steps = golden
        .cases
        .get(&key(case, host))
        .ok_or("missing capture")?;
      compare::capture_meets(case, steps).map_err(|err| format!("{}: {err}", key(case, host)))?;
    }
  }
  Ok(())
}

/// Every bats test at the base is ported by a case or named as not golden.
fn bats_ported() -> Res<()> {
  let cases = load_cases("rust")?;
  let titles = load_golden("rust")?
    .bats_tests
    .ok_or("the rust golden lists no bats tests")?;
  ensure(titles.len() > 80, || {
    format!("only {} bats titles", titles.len())
  })?;
  let mut ported: BTreeSet<String> = cases.iter().flat_map(|case| case.from.clone()).collect();
  ported.extend(load_excluded()?);
  let listed: BTreeSet<String> = titles.into_iter().collect();
  let unported: Vec<&String> = listed.difference(&ported).collect();
  let unknown: Vec<&String> = ported.difference(&listed).collect();
  ensure(unported.is_empty() && unknown.is_empty(), || {
    format!("bats titles not ported: {unported:?}; ported but not at the base: {unknown:?}")
  })
}

/// A failed test, printed as its message and not as an escaped string.
struct Failure(String);

impl std::fmt::Debug for Failure {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

impl From<Box<dyn Error>> for Failure {
  fn from(err: Box<dyn Error>) -> Self {
    Self(err.to_string())
  }
}

#[test]
fn ts_quality_replays_every_golden_case() -> Result<(), Failure> {
  Ok(replay_plugin(&TS, &TS_SHAPE)?)
}

#[test]
fn python_quality_replays_every_golden_case() -> Result<(), Failure> {
  Ok(replay_plugin(&PYTHON, &PYTHON_SHAPE)?)
}

#[test]
fn rust_quality_replays_every_golden_case() -> Result<(), Failure> {
  Ok(replay_plugin(&RUST, &RUST_SHAPE)?)
}

#[test]
fn the_goldens_hold_exactly_the_corpora_and_meet_their_cases() -> Result<(), Failure> {
  for shape in [&TS_SHAPE, &PYTHON_SHAPE, &RUST_SHAPE] {
    corpus(shape)?;
  }
  Ok(())
}

#[test]
fn every_bats_test_is_ported_by_a_case_or_named_as_not_golden() -> Result<(), Failure> {
  Ok(bats_ported()?)
}

#[test]
fn the_not_replayed_list_is_empty_or_reasoned() {
  assert!(
    NOT_REPLAYED
      .iter()
      .all(|(case, why)| !case.is_empty() && !why.is_empty())
  );
  assert_eq!(NOT_REPLAYED.len(), 0);
}
