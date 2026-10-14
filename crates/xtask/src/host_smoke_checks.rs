//! What `host-smoke` asserts about a finished session: transcripts and the state the
//! hooks left in the project.

use std::path::Path;

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

/// What the allowed control prints before the directory it ran in.
const CWD_MARKER: &str = "toolu-smoke-cwd:";

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

/// The control command ran: it printed the marker and the project, as given or resolved.
/// The directory alone proves nothing, since a session's init event already names it.
pub(crate) fn ran_in(project: &Path, transcript: &str) -> Result<(), String> {
  let mut spellings = vec![project.display().to_string()];
  if let Ok(real) = std::fs::canonicalize(project) {
    let real = real.display().to_string();
    spellings.extend(real.strip_prefix("/private").map(str::to_owned));
    spellings.push(real);
  }
  must(
    spellings
      .iter()
      .any(|path| transcript.contains(&format!("{CWD_MARKER}{path}"))),
    "the clean control command did not run",
  )
}

/// The gate file records the failing edit, and `sample.ts` holds it.
pub(crate) fn quality(project: &Path, host_dir: &str) -> Result<(), String> {
  let gate = project.join(host_dir).join("tmp/quality-gate-status.json");
  let state = std::fs::read_to_string(&gate)
    .map_err(|err| format!("quality gate file missing: {}: {err}", gate.display()))?;
  let source = std::fs::read_to_string(project.join("sample.ts"))
    .map_err(|err| format!("cannot read sample.ts: {err}"))?;
  quality_state(&state, &source)
}

/// The pure half of [`quality`].
pub(crate) fn quality_state(state: &str, source: &str) -> Result<(), String> {
  must(
    state.contains("\"status\": \"failing\"") && state.contains("TS file exceeds 3-line limit"),
    "quality failure not recorded",
  )?;
  must(
    source.lines().filter(|line| !line.is_empty()).count() >= 8,
    "source edit missing",
  )
}

/// The commit was refused by the quality gate.
pub(crate) fn commit_denied(transcript: &str) -> Result<(), String> {
  must(
    transcript.contains("quality gate failing"),
    "the commit was not denied by the quality gate",
  )
}

/// The push was refused by toolu's push review.
pub(crate) fn push_blocked(transcript: &str) -> Result<(), String> {
  let blocked = PUSH_REVIEW_REASONS
    .iter()
    .any(|reason| transcript.contains(reason));
  must(blocked, "the push was not blocked by toolu's push review")
}

#[cfg(test)]
#[path = "tests/host_smoke_checks_test.rs"]
mod tests;
