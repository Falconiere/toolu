//! The edit session of a host whose post-edit block replaces the tool result
//! (Codex, Muse): the model may repair its edit in the same turn, so the first
//! failing gate file and source seen while it ran are written back afterwards.

use std::path::Path;

use crate::host_smoke_exec::{TIMEOUT, exec, tail};
use crate::host_smoke_project::config_dir;
use crate::host_smoke_steps::Run;

/// The gate file and source as they are now, when the gate file records a failure.
pub(crate) fn observe(gate: &Path, source: &Path) -> Option<(String, String)> {
  let state = std::fs::read_to_string(gate).ok()?;
  if !state.contains("\"status\": \"failing\"") {
    return None;
  }
  Some((state, std::fs::read_to_string(source).ok()?))
}

/// Run `argv`, the `host`'s edit session in `project`, keeping its transcript as
/// `<host>-edit.jsonl`, and leave the first failing state it recorded in place.
pub(crate) fn observed_edit(
  run: &Run<'_>,
  project: &Path,
  argv: &[String],
  host: &str,
) -> Result<(), String> {
  let gate = project
    .join(config_dir(host))
    .join("tmp/quality-gate-status.json");
  let source = project.join("sample.ts");
  let mut seen: Option<(String, String)> = None;
  let mut tick = || {
    if seen.is_none() {
      seen = observe(&gate, &source);
    }
  };
  let done = exec(argv, project, run.env, TIMEOUT, &mut tick)?;
  std::fs::write(run.root.join(format!("{host}-edit.jsonl")), &done.output)
    .map_err(|err| format!("cannot write the edit transcript: {err}"))?;
  let Some((state, text)) = seen.filter(|_| done.success) else {
    return Err(format!(
      "{host} did not record a live failing edit (exit {:?}): {}",
      done.code,
      tail(&done.output)
    ));
  };
  std::fs::write(&source, text).map_err(|err| format!("cannot restore sample.ts: {err}"))?;
  std::fs::write(&gate, state).map_err(|err| format!("cannot restore the gate file: {err}"))
}

#[cfg(test)]
#[path = "tests/host_smoke_observe_test.rs"]
mod tests;
