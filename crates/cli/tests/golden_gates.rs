//! The pre-tool gate goldens against the native binary. The deleted TypeScript suites
//! (`pre-tool-modules-{a,b,c}`) replayed `fixtures/gates/pre-tool-modules-*-golden.json`,
//! the captures bash made at the base commits, through the `TOOLU_IMPL` seam, which
//! selected `toolu hook <entry> --event PreToolUse --plugin-root plugins/toolu`. This does
//! the same from Rust: every case in a fresh sandbox, the decision class and text, then the
//! `comparable` projection against the capture (or, for a case with a `deviation`, against
//! the known-wrong capture, which it must differ from). The B and A goldens also hold the
//! rows of `pretool-corpus.json` that these modules decide.

#[path = "helpers/golden_gates_check.rs"]
mod golden_gates_check;
#[path = "helpers/golden_gates_corpus.rs"]
mod golden_gates_corpus;
#[path = "helpers/golden_gates_files.rs"]
mod golden_gates_files;
#[path = "helpers/golden_gates_model.rs"]
mod golden_gates_model;
#[path = "helpers/golden_gates_prepare.rs"]
mod golden_gates_prepare;
#[path = "helpers/golden_gates_run.rs"]
mod golden_gates_run;
#[path = "helpers/golden_gates_sandbox.rs"]
mod golden_gates_sandbox;
#[path = "helpers/golden_gates_setup.rs"]
mod golden_gates_setup;
#[path = "helpers/golden_gates_stdin.rs"]
mod golden_gates_stdin;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

use golden_gates_check::{check_case, check_corpus};
use golden_gates_model::{Capture, CorpusCase, Host, ModuleCase, Res, read_cases, read_golden};
use golden_gates_prepare::Suite;

/// The outcome of one replay: `None` when it matched.
type Verdict = Option<String>;

/// What a replay of one golden found.
struct Replay {
  /// The golden and the commit it was captured at.
  golden: String,
  /// Captures the golden holds.
  expected: usize,
  /// Cases replayed.
  replayed: usize,
  /// Named mismatches.
  failures: Vec<String>,
}

impl Replay {
  /// What went wrong, if anything.
  fn problems(&self, count: usize) -> Option<String> {
    if self.expected != count {
      return Some(format!(
        "{}: the golden holds {} captures, not {count}",
        self.golden, self.expected
      ));
    }
    if self.replayed != self.expected {
      return Some(format!(
        "{}: replayed {} of {} captures",
        self.golden, self.replayed, self.expected
      ));
    }
    if self.failures.is_empty() {
      return None;
    }
    let list = self.failures.join("\n");
    Some(format!(
      "{}: {} of {} differ\n{list}",
      self.golden,
      self.failures.len(),
      self.replayed
    ))
  }
}

/// What `work` found for `item`, named by `label`.
fn problem<T>(
  item: &T,
  label: &impl Fn(&T) -> String,
  work: &impl Fn(&T) -> Res<Verdict>,
) -> Option<String> {
  let found = match work(item) {
    Ok(None) => return None,
    Ok(Some(found)) => found,
    Err(error) => format!("harness error: {error}"),
  };
  Some(format!("{}: {found}", label(item)))
}

/// Take items off `next` until none are left, sending each problem.
fn drain<T>(
  items: &[T],
  next: &AtomicUsize,
  label: &impl Fn(&T) -> String,
  work: &impl Fn(&T) -> Res<Verdict>,
  sender: &mpsc::Sender<String>,
) {
  while let Some(item) = items.get(next.fetch_add(1, Ordering::SeqCst)) {
    if let Some(found) = problem(item, label, work)
      && sender.send(found).is_err()
    {
      break;
    }
  }
}

/// `work` over `items` on a few threads; the problems found, each named by `label`.
fn par_verdicts<T: Sync>(
  items: &[T],
  label: impl Fn(&T) -> String + Sync,
  work: impl Fn(&T) -> Res<Verdict> + Sync,
) -> Vec<String> {
  let workers = thread::available_parallelism()
    .map_or(4, std::num::NonZero::get)
    .min(6);
  let next = AtomicUsize::new(0);
  let (sender, receiver) = mpsc::channel::<String>();
  thread::scope(|scope| {
    for _ in 0..workers {
      let sender = sender.clone();
      let (next, label, work) = (&next, &label, &work);
      scope.spawn(move || drain(items, next, label, work, &sender));
    }
  });
  drop(sender);
  let mut failures: Vec<String> = receiver.iter().collect();
  failures.sort();
  failures
}

/// Replay every case of `fixtures/gates/<file>.json` against `<file>-golden.json`.
fn replay_cases(suite: Suite, file: &str) -> Res<Replay> {
  let cases: Vec<ModuleCase> = read_cases(file)?;
  let golden = read_golden(&format!("{file}-golden"))?;
  let mut names: Vec<&str> = cases.iter().map(|c| c.name.as_str()).collect();
  let mut captured: Vec<&str> = golden.cases.keys().map(String::as_str).collect();
  names.sort_unstable();
  captured.sort_unstable();
  if names != captured {
    return Err(format!("{file}: capture names differ from case names").into());
  }
  let failures = par_verdicts(
    &cases,
    |c| c.name.clone(),
    |c| match golden.cases.get(&c.name) {
      Some(want) => check_case(suite, c, want),
      None => Err(format!("no golden capture for {}", c.name).into()),
    },
  );
  Ok(Replay {
    golden: format!("{file} (captured at {})", golden.base),
    expected: golden.cases.len(),
    replayed: cases.len(),
    failures,
  })
}

/// The corpus rows `<file>-golden.json` holds, as `<row> [<host>]`, whose names start with
/// one of `prefixes` (all of them when empty).
fn corpus_jobs<'a>(
  corpus: &'a [CorpusCase],
  keys: &'a std::collections::BTreeMap<String, Capture>,
  prefixes: &[&str],
) -> Vec<(&'a CorpusCase, Host, &'a Capture)> {
  let mut jobs = Vec::new();
  for row in corpus {
    if !prefixes.is_empty() && !prefixes.iter().any(|prefix| row.name.starts_with(prefix)) {
      continue;
    }
    for host in [Host::Claude, Host::Codex] {
      if let Some(want) = keys.get(&format!("{} [{}]", row.name, host.name())) {
        jobs.push((row, host, want));
      }
    }
  }
  jobs
}

/// Replay the corpus rows of `<file>-golden.json`.
fn replay_corpus(suite: Suite, file: &str, prefixes: &[&str]) -> Res<Replay> {
  let corpus: Vec<CorpusCase> = read_cases("pretool-corpus")?;
  let golden = read_golden(&format!("{file}-golden"))?;
  let jobs = corpus_jobs(&corpus, &golden.corpus, prefixes);
  let failures = par_verdicts(
    &jobs,
    |(row, host, _)| format!("{} [{}]", row.name, host.name()),
    |(row, host, want)| check_corpus(suite, row, *host, want),
  );
  Ok(Replay {
    golden: format!("{file} corpus (captured at {})", golden.base),
    expected: golden.corpus.len(),
    replayed: jobs.len(),
    failures,
  })
}

#[test]
fn modules_a_replay_all_109_golden_cases() {
  let replay = replay_cases(Suite::A, "pre-tool-modules-a").unwrap();
  if let Some(problems) = replay.problems(109) {
    panic!("{problems}");
  }
}

#[test]
fn modules_a_replay_the_22_corpus_rows_they_decide() {
  let replay = replay_corpus(Suite::A, "pre-tool-modules-a", &[]).unwrap();
  if let Some(problems) = replay.problems(22) {
    panic!("{problems}");
  }
}

#[test]
fn modules_b_replay_all_110_golden_cases() {
  let replay = replay_cases(Suite::B, "pre-tool-modules-b").unwrap();
  if let Some(problems) = replay.problems(110) {
    panic!("{problems}");
  }
}

#[test]
fn modules_b_replay_the_14_corpus_rows_they_decide() {
  let prefixes = ["bash-commands:", "commit-gate:", "quality-gate:"];
  let replay = replay_corpus(Suite::B, "pre-tool-modules-b", &prefixes).unwrap();
  if let Some(problems) = replay.problems(14) {
    panic!("{problems}");
  }
}

#[test]
fn modules_c_replay_all_180_golden_cases() {
  let replay = replay_cases(Suite::C, "pre-tool-modules-c").unwrap();
  if let Some(problems) = replay.problems(180) {
    panic!("{problems}");
  }
}
