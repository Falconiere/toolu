//! Running one plan step's `check` (`packages/toolu-core/src/ledger/ledger-check.ts`).
//! The check runs as `bash -c` in its own process group with stdin at end of
//! file, stdout and stderr interleaved into one file. `PLAN_LEDGER_STEP_TIMEOUT`
//! bounds it: past the deadline the group gets SIGTERM, then SIGKILL after a
//! grace period, and the step exits 124 as GNU `timeout` reports it. In a
//! worktree bound to a resource home the check runs under a machine job lease.

use std::path::PathBuf;
use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::json::jq_text;
use toolu_runtime::process::file::{FileSpec, run_to_file};
use toolu_runtime::process::guard::GroupGuard;
use toolu_runtime::process::{Output, Spec};

use super::jq::string;
use crate::resources::binding::{ResourceBinding, resource_binding};
use crate::resources::jobs::run_managed_job;

/// The exit code of a check stopped at its deadline.
pub const TIMEOUT_EXIT: i32 = 124;
/// The exit code of an invalid bound or a truncated managed output.
const INVALID_TIMEOUT_EXIT: i32 = 125;
const TRUNCATED_OUTPUT_NOTE: &str = "plan-ledger: check output exceeded capture limit\n";
const KILL_GRACE: Duration = Duration::from_secs(2);
const EVIDENCE_LINES: usize = 10;
const EVIDENCE_BYTES: usize = 2000;
/// The longest delay `setTimeout` honours.
const MAX_DELAY: Duration = Duration::from_millis(2_147_483_647);

/// A GNU `timeout` duration in seconds (`1800`, `+1.5`, ` 1e3`, `2m`); 0
/// disables the bound. `None` for text outside that decimal grammar.
pub fn parse_timeout(value: &str) -> Option<f64> {
  let text = value.trim_start_matches([' ', '\t', '\n', '\u{b}', '\u{c}', '\r']);
  let text = text.strip_prefix('+').unwrap_or(text);
  let (number, unit) = match text.char_indices().last() {
    Some((at, unit @ ('s' | 'm' | 'h' | 'd'))) => (text.get(..at)?, unit),
    _ => (text, 's'),
  };
  if !decimal(number) {
    return None;
  }
  let scale = match unit {
    'm' => 60.0,
    'h' => 3600.0,
    'd' => 86_400.0,
    _ => 1.0,
  };
  let seconds = number.parse::<f64>().ok()? * scale;
  seconds.is_finite().then_some(seconds)
}

/// `(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?`.
fn decimal(text: &str) -> bool {
  let (mantissa, exponent) = match text.find(['e', 'E']) {
    Some(at) => (text.get(..at).unwrap_or_default(), text.get(at + 1..)),
    None => (text, None),
  };
  let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
  let mantissa_ok = match mantissa.split_once('.') {
    Some(("", fraction)) => digits(fraction),
    Some((whole, fraction)) => digits(whole) && (fraction.is_empty() || digits(fraction)),
    None => digits(mantissa),
  };
  let exponent_ok = exponent.is_none_or(|exp| digits(exp.strip_prefix(['+', '-']).unwrap_or(exp)));
  mantissa_ok && exponent_ok
}

/// One check to run.
#[derive(Debug, Clone)]
pub struct CheckRun {
  /// The check, run by `bash -c`.
  pub check: String,
  /// The project root.
  pub cwd: PathBuf,
  /// The check's whole environment.
  pub env: Env,
  /// Where the combined output lands; the caller reads and removes it.
  pub out_file: PathBuf,
  /// The raw `PLAN_LEDGER_STEP_TIMEOUT` text (`0` disables the bound).
  pub timeout: String,
}

/// The deadline of a positive bound, capped at [`MAX_DELAY`]; `None` for 0.
fn bound(seconds: f64) -> Option<Duration> {
  (seconds > 0.0)
    .then(|| Duration::try_from_secs_f64(seconds).map_or(MAX_DELAY, |d| d.min(MAX_DELAY)))
}

/// `pl_run_check`: the check's exit code (124 when the bound stopped it).
///
/// # Errors
/// When the output file, the binding, the lease or the spawn fails; the step
/// then records exit 1 with no output, as a failed bash redirect did.
pub fn run_check(run: &CheckRun) -> Result<i32, String> {
  let Some(seconds) = parse_timeout(&run.timeout) else {
    let note = format!(
      "plan-ledger: invalid PLAN_LEDGER_STEP_TIMEOUT '{}'\n",
      run.timeout
    );
    std::fs::write(&run.out_file, note).map_err(|err| err.to_string())?;
    return Ok(INVALID_TIMEOUT_EXIT);
  };
  match resource_binding(&run.cwd)? {
    Some(binding) => managed(run, &binding, seconds),
    None => unmanaged(run, seconds),
  }
}

fn managed(run: &CheckRun, binding: &ResourceBinding, seconds: f64) -> Result<i32, String> {
  std::fs::write(&run.out_file, "").map_err(|err| err.to_string())?;
  let mut spec = Spec::new(Vec::<String>::new());
  spec.cwd = Some(run.cwd.clone());
  spec.env = Some(run.env.clone());
  spec.timeout = bound(seconds).unwrap_or(MAX_DELAY);
  let argv = [
    "bash",
    "-c",
    "exec bash -c \"$1\" 2>&1",
    "toolu-ledger",
    &run.check,
  ]
  .map(str::to_owned);
  let result: Output = run_managed_job(&argv, binding, &spec)?;
  let note = if result.truncated {
    TRUNCATED_OUTPUT_NOTE
  } else {
    ""
  };
  let body = format!("{}{}{note}", result.stdout, result.stderr);
  std::fs::write(&run.out_file, body).map_err(|err| err.to_string())?;
  Ok(if result.timed_out {
    TIMEOUT_EXIT
  } else if result.truncated {
    INVALID_TIMEOUT_EXIT
  } else {
    result.exit_code
  })
}

fn unmanaged(run: &CheckRun, seconds: f64) -> Result<i32, String> {
  let guard = GroupGuard::install()?;
  let spec = FileSpec {
    argv: vec!["bash".to_owned(), "-c".to_owned(), run.check.clone()],
    cwd: Some(run.cwd.clone()),
    env: Some(run.env.clone()),
    out: run.out_file.clone(),
    timeout: bound(seconds),
    grace: KILL_GRACE,
  };
  let output = run_to_file(&spec, &mut |pid| guard.arm(pid)).map_err(|err| format!("{err:?}"))?;
  Ok(if output.timed_out {
    TIMEOUT_EXIT
  } else {
    output.exit_code
  })
}

/// `$(...)` capture: NUL bytes dropped, trailing newlines stripped.
pub fn command_substitution(bytes: &[u8]) -> Vec<u8> {
  let mut kept: Vec<u8> = bytes.iter().copied().filter(|b| *b != 0).collect();
  while kept.last() == Some(&b'\n') {
    kept.pop();
  }
  kept
}

/// `tail -n 10` of text with no trailing newline.
fn last_lines(bytes: &[u8]) -> &[u8] {
  let mut seen = 0;
  for (at, byte) in bytes.iter().enumerate().rev() {
    if *byte == b'\n' {
      seen += 1;
      if seen == EVIDENCE_LINES {
        return bytes.get(at + 1..).unwrap_or_default();
      }
    }
  }
  bytes
}

/// `pl_evidence`: tail, cap and decode the output as `tail | head -c | jq -Rs` does.
pub fn evidence_of(bytes: &[u8]) -> String {
  let kept = command_substitution(bytes);
  let tail = last_lines(&kept);
  let capped = tail.get(..EVIDENCE_BYTES.min(tail.len())).unwrap_or(tail);
  String::from_utf8_lossy(capped).into_owned()
}

/// The evidence recorded for a finished check. On exit 124 bash prepended the
/// timeout reason to the already JSON-encoded tail and encoded it again.
pub fn step_evidence(exit_code: i32, output: &[u8], timeout: &str) -> String {
  let evidence = evidence_of(output);
  if exit_code != TIMEOUT_EXIT {
    return evidence;
  }
  let reason = format!(
    "timed out after {timeout}s (PLAN_LEDGER_STEP_TIMEOUT)\n{}",
    jq_text(&string(&evidence), false)
  );
  evidence_of(reason.as_bytes())
}

#[cfg(test)]
#[path = "tests/check_test.rs"]
mod tests;
