//! A remote worker writes a bound report without contacting a remote controller.

use crate::verbs::{failed, json_out, text};
use clap::ArgMatches;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::process::{Spec, run};

/// Handle the paired remote-report flags, before ordinary report spooling.
pub(crate) fn report(matches: &ArgMatches, env: &Env) -> Option<Outcome> {
  let run = matches
    .try_get_one::<String>("run")
    .ok()
    .flatten()?
    .as_str();
  Some(match write(matches, env, run) {
    Ok(()) => json_out(&json!({"ok":true})),
    Err(error) => failed("toolu epic report", &error),
  })
}

fn write(matches: &ArgMatches, env: &Env, identity: &str) -> Result<(), String> {
  let base = text(matches, "expected-base").ok_or("expected-base required")?;
  let cwd = current_dir().map_err(|e| e.to_string())?;
  let mut spec = Spec::new(["git", "rev-parse", "HEAD"]);
  spec.cwd = Some(cwd);
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(10);
  let output = run(&spec).map_err(|e| format!("{e:?}"))?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Err("cannot read committed worker revision".into());
  }
  let commit = output.stdout.trim();
  validate(identity, base, commit)?;
  let doc = json!({"run":identity,"expected_base":base,"commit":commit,
    "phase":text(matches,"phase"),"pr":matches.get_one::<u64>("pr"),"note":text(matches,"note").unwrap_or("")});
  let file = text(matches, "status-file").ok_or("status-file required")?;
  crate::disk::write_value(Path::new(file), &doc)
}

/// A report must bind a safe run and complete Git object identities.
pub(crate) fn validate(run: &str, base: &str, commit: &str) -> Result<(), String> {
  let sha = |s: &str| {
    matches!(s.len(), 40 | 64)
      && s
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
  };
  if run.is_empty()
    || !run
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    || !sha(base)
    || !sha(commit)
  {
    return Err("invalid remote report identity".into());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/hc_report_test.rs"]
mod tests;

/// A unique report spool token.
pub(crate) fn report_token(key: &str) -> String {
  use std::sync::atomic::{AtomicU32, Ordering};
  static NEXT: AtomicU32 = AtomicU32::new(0);
  let n = NEXT.fetch_add(1, Ordering::Relaxed);
  let millis = toolu_state::time::epoch_millis(std::time::SystemTime::now());
  let safe: String = key
    .chars()
    .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    .collect();
  format!("r{millis:013}-{}-{n:04}-{safe}", std::process::id())
}
