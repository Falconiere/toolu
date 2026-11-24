//! What `host-smoke` asserts about a finished session: transcripts and the state the
//! hooks left in the project.

use std::path::Path;

use serde_json::Value;

const GATE_FILE: &str = "quality-gate-status.json";
const STATUS: &str = "failing";
const SOURCE: &str = "gate-status-hook";
const REASON: &str = "Quality command failed: sh tools/smoke/test.sh (exit 1)";

/// `Ok` when `condition` holds, else the `message`.
pub(crate) fn must(condition: bool, message: &str) -> Result<(), String> {
  if condition {
    Ok(())
  } else {
    Err(message.to_owned())
  }
}

/// The deny reasons of the push-review gate: block mode, then ask mode.
const PUSH_REVIEW_REASONS: [&str; 2] = [
  "Code review required before push",
  "No clean review is recorded for this diff",
];

/// The protected-files gate's block-mode reason, which `host-smoke` configures.
const PROTECTED_REASON: &str = "Blocked by gates.protectedFiles.mode='block'";

/// The protected `.env` is untouched and the transcript carries the protected-files
/// gate's own deny reason.
pub(crate) fn deny(project: &Path, transcript: &str) -> Result<(), String> {
  let env_file = project.join(".env");
  let text = std::fs::read_to_string(&env_file)
    .map_err(|err| format!("cannot read {}: {err}", env_file.display()))?;
  must(text == "KEEP=1\n", "protected .env changed")?;
  must(
    transcript.contains(PROTECTED_REASON),
    "no live deny evidence",
  )
}

/// The control command ran: the transcript holds the project's `nonce`, which only
/// `cat .toolu-smoke-nonce` in the project directory prints. A session's init event
/// names the directory and the model can type a marker itself, but neither can know the
/// nonce without reading the file by its relative path.
pub(crate) fn ran_in(nonce: &str, transcript: &str) -> Result<(), String> {
  must(
    !nonce.is_empty() && transcript.contains(nonce),
    "the clean control command did not run",
  )
}

/// The gate file records the failing quality command.
pub(crate) fn quality(project: &Path, state_dir: &str) -> Result<(), String> {
  let gate = gate(project, state_dir);
  let state = std::fs::read_to_string(&gate)
    .map_err(|err| format!("quality gate file missing: {}: {err}", gate.display()))?;
  quality_state(&state)
}

/// The pure half of [`quality`]: the gate file records the failing wrapper command.
pub(crate) fn quality_state(state: &str) -> Result<(), String> {
  let state: Value =
    serde_json::from_str(state).map_err(|err| format!("the quality gate is not JSON: {err}"))?;
  must(
    state["status"] == STATUS && state["source"] == SOURCE && state["reason"] == REASON,
    "the failing quality command was not recorded",
  )
}

/// The gate status file that the failing probe must create.
pub(crate) fn gate(project: &Path, state_dir: &str) -> std::path::PathBuf {
  project.join(state_dir).join(GATE_FILE)
}

/// The push was refused by a gate: toolu's push review or the red quality gate.
pub(crate) fn push_blocked(transcript: &str) -> Result<(), String> {
  let blocked = PUSH_REVIEW_REASONS
    .iter()
    .any(|reason| transcript.contains(reason))
    || transcript.contains("quality gate failing");
  must(
    blocked,
    "the push was not blocked by toolu's push review or quality gate",
  )
}

#[cfg(test)]
#[path = "tests/host_smoke_checks_test.rs"]
mod tests;
