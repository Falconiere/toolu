//! Rule 19, the part clippy does not own: a to-do or fix-me marker in a
//! comment names its issue number (`#<n>`) on the same line.

use regex::Regex;

use super::{Context, Finding};

/// Marker comments without an issue number.
pub(super) fn check(ctx: &Context<'_>) -> Result<Vec<Finding>, String> {
  let marker = Regex::new(r"\b(TODO|FIXME)\b").map_err(|err| err.to_string())?;
  let issue = Regex::new(r"#\d+").map_err(|err| err.to_string())?;
  let mut found = Vec::new();
  for source in &ctx.sources {
    let lines = source.lines.comments.iter().flat_map(|(first, text)| {
      text
        .lines()
        .enumerate()
        .map(move |(offset, line)| (first + offset, line))
    });
    for (at, line) in lines {
      let Some(hit) = marker.find(line).filter(|_| !issue.is_match(line)) else {
        continue;
      };
      found.push(Finding::new(
        "leftovers",
        &source.display(),
        at,
        format!(
          "{0} without an issue number — write {0}(#<issue>) or do it now",
          hit.as_str()
        ),
      ));
    }
  }
  Ok(found)
}

#[cfg(test)]
#[path = "tests/leftovers_test.rs"]
mod tests;
