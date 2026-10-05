//! Comparing the binary's version with a plugin's: `MAJOR.MINOR.PATCH` decimal
//! triples, with any `-prerelease` or `+build` suffix ignored.

use std::cmp::Ordering;

/// How the binary's version relates to the plugin's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
  /// The same version string.
  Same,
  /// The binary is older than the plugin.
  Older,
  /// The binary is newer than the plugin.
  Newer,
  /// Equal triples, different strings (a prerelease or build suffix).
  Differs,
  /// One side is not a version.
  Incomparable,
}

/// Compare `binary` with `plugin`.
pub fn compare(binary: &str, plugin: &str) -> Comparison {
  if binary == plugin {
    return Comparison::Same;
  }
  match (triple(binary), triple(plugin)) {
    (Some(ours), Some(theirs)) => match ours.cmp(&theirs) {
      Ordering::Less => Comparison::Older,
      Ordering::Greater => Comparison::Newer,
      Ordering::Equal => Comparison::Differs,
    },
    _ => Comparison::Incomparable,
  }
}

/// `MAJOR.MINOR.PATCH` of `text`, ignoring a `-…` or `+…` suffix.
fn triple(text: &str) -> Option<(u64, u64, u64)> {
  let core = text.split(['-', '+']).next()?;
  let mut parts = core.split('.').map(number);
  let triple = (parts.next()??, parts.next()??, parts.next()??);
  parts.next().is_none().then_some(triple)
}

fn number(part: &str) -> Option<u64> {
  if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  part.parse().ok()
}

#[cfg(test)]
#[path = "tests/version_test.rs"]
mod tests;
