//! Replaying every case of a quality plugin on every host it names, in parallel,
//! against the golden capture.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

use crate::Res;
use crate::cases::{Case, Golden, Host, Recorded, key};
use crate::compare::{deviates, matches};
use crate::sandbox::Sandbox;
use crate::setup::{case_env, prepare, register};
use crate::step::dispatch;

/// A quality plugin and what its golden asks beyond a plain match.
pub(crate) struct Plugin {
  /// The plugin and `toolu` namespace, e.g. `ts-quality`.
  pub(crate) name: &'static str,
  /// The plugin specs installed in each sandbox.
  pub(crate) installed: &'static [&'static str],
  /// The case that must keep its gate in a linked worktree, with the text each of its steps prints.
  pub(crate) worktree: Option<(&'static str, &'static str)>,
}

/// How the replays went.
#[derive(Debug, Default)]
pub(crate) struct Report {
  /// Case and host pairs replayed.
  pub(crate) replayed: usize,
  /// Of those, the pairs checked as named deviations.
  pub(crate) deviations: usize,
  /// Of those, the pairs checked as the worktree case.
  pub(crate) worktrees: usize,
  /// One description per mismatch, each naming its case.
  pub(crate) failures: Vec<String>,
}

enum Checked {
  Matched,
  Deviated,
  Worktree,
}

/// Run every step of `case` on `host` in a fresh sandbox; the sandbox outlives the steps.
fn run_case(plugin: &Plugin, case: &Case, host: Host) -> Res<(Sandbox, Vec<Recorded>)> {
  let sb = Sandbox::new()?;
  prepare(&sb, host, case, plugin.installed)?;
  register(&sb, host, plugin.name)?;
  for other in &case.register {
    register(&sb, host, other)?;
  }
  let env = case_env(&sb, case.env.as_ref())?;
  let steps = case
    .steps
    .iter()
    .map(|step| dispatch(&sb, host, &env, step))
    .collect::<Res<_>>()?;
  Ok((sb, steps))
}

/// The main project keeps no gate, and the linked worktree's own gate holds the failure.
fn worktree_gate(
  plugin: &Plugin,
  sb: &Sandbox,
  text: &str,
  steps: &[Recorded],
) -> Result<(), String> {
  if let Some(step) = steps.iter().find(|step| !step.stdout.contains(text)) {
    return Err(format!("a step does not print {text:?}: {:?}", step.stdout));
  }
  if sb
    .path(".claude/tmp/quality-gate-status.json")
    .is_ok_and(|gate| gate.exists())
  {
    return Err("the main project holds a gate file".to_owned());
  }
  let gate = sb.project.join("wt/.claude/tmp/quality-gate-status.json");
  let doc: Value = std::fs::read_to_string(&gate)
    .map_err(|err| format!("{}: {err}", gate.display()))
    .and_then(|body| serde_json::from_str(&body).map_err(|err| err.to_string()))?;
  let source = format!("{}-hook", plugin.name);
  let entries = doc.get("entries").and_then(Value::as_object);
  let sourced = entries.is_some_and(|entries| {
    !entries.is_empty()
      && entries.iter().all(|(file, entry)| {
        file.starts_with(&sb.project.join("wt").display().to_string())
          && entry.get("source").and_then(Value::as_str) == Some(source.as_str())
      })
  });
  if doc.get("status").and_then(Value::as_str) != Some("failing") || !sourced {
    return Err(format!(
      "the worktree gate is not failing from {source}: {doc}"
    ));
  }
  Ok(())
}

/// The outputs match and the main project keeps no state.
fn worktree_matches(expected: &[Recorded], actual: &[Recorded]) -> Result<(), String> {
  let bare = |steps: &[Recorded]| -> Vec<(String, String, i32)> {
    steps
      .iter()
      .map(|s| (s.stdout.clone(), s.stderr.clone(), s.exit_code))
      .collect()
  };
  if bare(expected) != bare(actual) {
    return Err("the worktree case printed something else than the golden".to_owned());
  }
  match actual.iter().find(|step| !step.state.is_empty()) {
    Some(step) => Err(format!(
      "the main project holds state: {:?}",
      step.state.keys()
    )),
    None => Ok(()),
  }
}

fn replay_one(
  plugin: &Plugin,
  case: &Case,
  host: Host,
  golden: &Golden,
) -> Result<Checked, String> {
  let name = key(case, host);
  let expected = golden
    .cases
    .get(&name)
    .ok_or_else(|| format!("no golden capture for {name}"))?;
  let (sb, actual) = run_case(plugin, case, host).map_err(|err| format!("replay failed: {err}"))?;
  if let Some((worktree, text)) = plugin.worktree
    && case.name == worktree
  {
    worktree_matches(expected, &actual)?;
    worktree_gate(plugin, &sb, text, &actual)?;
    return Ok(Checked::Worktree);
  }
  if let Some(deviation) = &case.deviation {
    deviates(deviation, expected, &actual)?;
    return Ok(Checked::Deviated);
  }
  matches(case, expected, &actual)?;
  Ok(Checked::Matched)
}

type Done = Vec<(usize, Result<Checked, String>)>;

/// The pairs to replay, the indexes of this pass, and the results so far, shared by the workers.
struct Work<'a> {
  plugin: &'a Plugin,
  pairs: &'a [(&'a Case, Host)],
  golden: &'a Golden,
  order: &'a [usize],
  next: AtomicUsize,
  done: Mutex<Done>,
}

impl Work<'_> {
  /// Replay the next unclaimed pair of the pass until none is left.
  fn run(&self) {
    loop {
      let at = self.next.fetch_add(1, Ordering::SeqCst);
      let Some(&index) = self.order.get(at) else {
        break;
      };
      let Some(&(case, host)) = self.pairs.get(index) else {
        break;
      };
      let result = replay_one(self.plugin, case, host, self.golden);
      if let Ok(mut done) = self.done.lock() {
        done.push((index, result));
      }
    }
  }
}

/// Replay the pairs at `order` with `workers` threads, each pulling the next pair.
fn replay_pass(
  plugin: &Plugin,
  pairs: &[(&Case, Host)],
  golden: &Golden,
  order: &[usize],
  workers: usize,
) -> Res<Done> {
  let work = Work {
    plugin,
    pairs,
    golden,
    order,
    next: AtomicUsize::new(0),
    done: Mutex::new(Vec::new()),
  };
  std::thread::scope(|scope| {
    for _ in 0..workers {
      scope.spawn(|| work.run());
    }
  });
  work.done.into_inner().map_err(|err| err.to_string().into())
}

/// Replay `cases` on each of their hosts, in parallel and then the Node-tool cases alone.
pub(crate) fn replay_all(plugin: &Plugin, cases: &[Case], golden: &Golden) -> Res<Report> {
  let pairs: Vec<(&Case, Host)> = cases
    .iter()
    .flat_map(|case| case.hosts().into_iter().map(move |host| (case, host)))
    .collect();
  // The hook gives jscpd 10 s. Idle it takes 0.15 s, but beside the pool's instrumented
  // toolu runs it took 6.5 s and timed out in the container gate, so those cases wait.
  let (alone, pooled): (Vec<usize>, Vec<usize>) = (0..pairs.len()).partition(|&index| {
    pairs
      .get(index)
      .is_some_and(|(case, _)| case.links_a_node_tool())
  });
  let workers = std::thread::available_parallelism()
    .map_or(4, usize::from)
    .min(8);
  let mut results = replay_pass(plugin, &pairs, golden, &pooled, workers)?;
  results.extend(replay_pass(plugin, &pairs, golden, &alone, 1)?);
  results.sort_by_key(|(index, _)| *index);
  Ok(report(&pairs, results))
}

fn report(pairs: &[(&Case, Host)], results: Done) -> Report {
  let mut report = Report::default();
  for (index, result) in results {
    report.replayed += 1;
    match result {
      Ok(Checked::Matched) => {}
      Ok(Checked::Deviated) => report.deviations += 1,
      Ok(Checked::Worktree) => report.worktrees += 1,
      Err(why) => {
        let name = pairs
          .get(index)
          .map_or_else(String::new, |(case, host)| key(case, *host));
        report.failures.push(format!("{name}: {why}"));
      }
    }
  }
  report
}
