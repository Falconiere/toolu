//! The macOS inventory: identities from `ps`, cwds from `lsof`.

use std::collections::BTreeMap;
use std::time::Duration;

use regex::Regex;
use toolu_runtime::process::{Spec, run};

use super::ProcessRecord;

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BYTES: usize = 8 * 1024 * 1024;

fn command(argv: &[&str]) -> Result<String, String> {
  let mut spec = Spec::new(argv.iter().copied());
  spec.timeout = TIMEOUT;
  spec.max_output_bytes = MAX_BYTES;
  let name = argv.first().copied().unwrap_or("");
  let out =
    run(&spec).map_err(|err| format!("process inventory command unavailable: {name}: {err:?}"))?;
  if out.exit_code != 0 || out.timed_out || out.truncated {
    return Err(format!(
      "process inventory command failed ({}): {}",
      out.exit_code,
      out.stderr.trim()
    ));
  }
  Ok(out.stdout)
}

/// `ps -axo pid=,ppid=,pgid=,stat=,lstart=` rows, zombies dropped.
pub(super) fn parse_ps(output: &str) -> Vec<ProcessRecord> {
  let Ok(row) = Regex::new(r"^\s*(\d+)\s+(\d+)\s+(\d+)\s+(\S+)\s+(.+?)\s*$") else {
    return Vec::new();
  };
  output
    .lines()
    .filter_map(|line| {
      let caps = row.captures(line)?;
      let number = |index: usize| caps.get(index)?.as_str().parse::<u32>().ok();
      let stat = caps.get(4)?.as_str();
      if stat.starts_with('Z') {
        return None;
      }
      Some(ProcessRecord {
        pid: number(1)?,
        parent: number(2)?,
        group: number(3)?,
        started: format!("mac:{}", caps.get(5)?.as_str()),
        cwd: None,
        cwd_unavailable: false,
      })
    })
    .collect()
}

/// `lsof -F pn` output as PID to cwd.
pub(super) fn parse_lsof(output: &str) -> BTreeMap<u32, String> {
  let mut cwds = BTreeMap::new();
  let mut pid = None;
  for line in output.lines() {
    if let Some(rest) = line.strip_prefix('p') {
      pid = rest.parse::<u32>().ok();
    } else if let (Some(rest), Some(current)) = (line.strip_prefix('n'), pid) {
      cwds.insert(current, rest.to_owned());
    }
  }
  cwds
}

/// Every live process's identity.
///
/// # Errors
/// `ps` fails.
pub(super) fn identities() -> Result<Vec<ProcessRecord>, String> {
  Ok(parse_ps(&command(&[
    "ps",
    "-axo",
    "pid=,ppid=,pgid=,stat=,lstart=",
  ])?))
}

/// Every live process with the cwd `lsof` reports.
///
/// # Errors
/// `ps` or `lsof` fails.
pub(super) fn processes() -> Result<Vec<ProcessRecord>, String> {
  let cwds = parse_lsof(&command(&["lsof", "-n", "-P", "-d", "cwd", "-F", "pn"])?);
  Ok(
    identities()?
      .into_iter()
      .map(|record| ProcessRecord {
        cwd: cwds.get(&record.pid).cloned(),
        ..record
      })
      .collect(),
  )
}

#[cfg(test)]
#[path = "tests/mac_test.rs"]
mod tests;
