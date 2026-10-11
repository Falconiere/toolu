//! An OpenCode fixer's process group: its leader's start time, which tells a
//! reused pid from the fixer, whether it still runs, and how it is stopped
//! with every descendant, including those that moved to a group of their own.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use toolu_runtime::process::group::{self, Signal};
use toolu_runtime::process::{Spec, run};

const POLL: Duration = Duration::from_millis(100);

struct Row {
  pid: u32,
  ppid: u32,
  pgid: u32,
}

fn ps(args: &[&str]) -> Option<String> {
  let mut argv = vec!["ps"];
  argv.extend_from_slice(args);
  let output = run(&Spec::new(argv)).ok()?;
  (output.exit_code == 0).then_some(output.stdout)
}

/// The leader's start time as `ps` prints it, or `""` when there is no such process.
pub(crate) fn process_start(pid: u32) -> String {
  ps(&["-o", "lstart=", "-p", &pid.to_string()])
    .map(|out| out.trim().to_owned())
    .unwrap_or_default()
}

/// Every live (non-zombie) process; empty when `ps` fails.
fn table() -> Vec<Row> {
  let listed = ps(&["-A", "-o", "pid=,ppid=,pgid=,stat="]).unwrap_or_default();
  let rows = listed.lines().filter_map(|line| {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let [pid, ppid, pgid, stat] = fields.as_slice() else {
      return None;
    };
    let number = |text: &str| text.parse::<u32>().ok();
    (!stat.starts_with('Z')).then_some(Row {
      pid: number(pid)?,
      ppid: number(ppid)?,
      pgid: number(pgid)?,
    })
  });
  rows.collect()
}

/// The group's members and every descendant, whatever its group: OpenCode
/// starts each bash tool call in a process group of its own.
fn family(pgid: u32) -> BTreeSet<u32> {
  let rows = table();
  let mut found: BTreeSet<u32> = rows
    .iter()
    .filter(|row| row.pgid == pgid)
    .map(|row| row.pid)
    .collect();
  loop {
    let more: Vec<u32> = rows
      .iter()
      .filter(|row| !found.contains(&row.pid) && found.contains(&row.ppid))
      .map(|row| row.pid)
      .collect();
    if more.is_empty() {
      return found;
    }
    found.extend(more);
  }
}

/// Whether the fixer group started as `pid` at `start` still runs. A reused
/// pid (a different recorded start) is not the fixer.
pub(crate) fn group_alive(pid: u32, start: &str) -> bool {
  if pid <= 1 {
    return false;
  }
  let leader = process_start(pid);
  if !leader.is_empty() && !start.is_empty() && leader != start {
    return false;
  }
  table().iter().any(|row| row.pgid == pid)
}

fn send(pid: u32, pids: &BTreeSet<u32>, signal: Signal) {
  let _group = group::signal(pid, signal);
  for member in pids {
    let _member = group::signal_pid(*member, signal);
  }
}

fn gone_by(pids: &BTreeSet<u32>, deadline: Instant) -> bool {
  loop {
    let live: BTreeSet<u32> = table().iter().map(|row| row.pid).collect();
    if pids.is_disjoint(&live) {
      return true;
    }
    if Instant::now() >= deadline {
      return false;
    }
    std::thread::sleep(POLL);
  }
}

/// Ends the fixer: its group and every descendant (taken before any signal,
/// so none is lost to reparenting), `SIGTERM`, up to 10 s, then `SIGKILL`.
/// True when nothing of it is left.
pub(crate) fn stop_group(pid: u32, start: &str) -> bool {
  if !group_alive(pid, start) {
    return true;
  }
  let pids = family(pid);
  send(pid, &pids, Signal::SIGTERM);
  if gone_by(&pids, Instant::now() + Duration::from_secs(10)) {
    return true;
  }
  send(pid, &pids, Signal::SIGKILL);
  gone_by(&pids, Instant::now() + Duration::from_secs(5))
}

#[cfg(test)]
#[path = "tests/procs_test.rs"]
mod tests;
