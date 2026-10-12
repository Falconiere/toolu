//! Worker notes on the journal and in status files: one line, capped, and
//! without a recognized token prefix.

/// The longest note stored, in Unicode scalar values.
pub(crate) const NOTE_CAP: usize = 1024;

const PREFIXES: &[&str] = &[
  "ghp_",
  "gho_",
  "ghs_",
  "ghu_",
  "ghr_",
  "github_pat_",
  "tsak_",
  "Bearer ",
];

/// `note` as a single journal line. A recognized token prefix becomes
/// `[redacted]`. Longer notes are cut at [`NOTE_CAP`].
pub(crate) fn bounded_note(note: &str) -> String {
  let flat: String = note
    .chars()
    .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
    .collect();
  if PREFIXES.iter().any(|prefix| flat.contains(prefix)) {
    return "[redacted]".to_owned();
  }
  flat.chars().take(NOTE_CAP).collect()
}

/// Screen lines a blocked agent shows.
const SCREEN_LINES: usize = 10;

/// The last [`SCREEN_LINES`] non-empty lines of `tail`, newline-separated and
/// capped at [`NOTE_CAP`] from the end. A recognized token prefix makes the
/// whole text `[redacted]`.
pub(crate) fn screen_lines(tail: &str) -> String {
  let mut lines: Vec<&str> = tail
    .lines()
    .map(str::trim_end)
    .filter(|line| !line.trim().is_empty())
    .rev()
    .take(SCREEN_LINES)
    .collect();
  lines.reverse();
  let text = lines.join("\n");
  if PREFIXES.iter().any(|prefix| text.contains(prefix)) {
    return "[redacted]".to_owned();
  }
  let skip = text.chars().count().saturating_sub(NOTE_CAP);
  text.chars().skip(skip).collect()
}

#[cfg(test)]
#[path = "tests/note_test.rs"]
mod tests;
