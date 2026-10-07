//! Worker notes on the journal and in status files: one line, capped, and
//! without a recognized token prefix.

/// The longest note stored, in Unicode scalar values.
pub(crate) const NOTE_CAP: usize = 1024;

/// `note` as a single journal line. A recognized token prefix becomes
/// `[redacted]`. Longer notes are cut at [`NOTE_CAP`].
pub(crate) fn bounded_note(note: &str) -> String {
  let flat: String = note
    .chars()
    .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
    .collect();
  if flat.contains("ghp_") || flat.contains("github_pat_") || flat.contains("TYPESAFE_API_KEY") {
    return "[redacted]".to_owned();
  }
  flat.chars().take(NOTE_CAP).collect()
}

#[cfg(test)]
#[path = "tests/note_test.rs"]
mod tests;
