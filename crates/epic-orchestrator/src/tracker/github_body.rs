//! Tick one child's line in a GitHub epic's checklist (`tickBody`).

use regex::Regex;

use crate::refs::{parse_ref, strip_prefix};

/// `body` with the unchecked task line for `issue` ticked. A full URL or
/// `owner/repo#N` line is ticked; a bare `#N` line is ticked only in the epic's
/// own repository and only when nothing or the title's first three words follow.
///
/// # Errors
/// `issue` is not a reference, or a pattern cannot compile.
pub(crate) fn tick_body(
  body: &str,
  epic_repo: (&str, &str),
  issue: &str,
  title: &str,
) -> Result<String, String> {
  let target = parse_ref(issue, None)?;
  let full = [
    format!(
      "https://github.com/{}/{}/issues/{}",
      target.owner, target.repo, target.number
    ),
    format!("{}/{}#{}", target.owner, target.repo, target.number),
  ];
  let alternatives: Vec<String> = full.iter().map(|text| regex::escape(text)).collect();
  let task = Regex::new(&format!(
    r"(?i)^(\s*[-*]\s*)\[ \](\s*)({})",
    alternatives.join("|")
  ))
  .map_err(|err| err.to_string())?;
  let bare = Regex::new(&format!(
    r"^(\s*[-*]\s*)\[ \](\s*)(#{})(.*)$",
    target.number
  ))
  .map_err(|err| err.to_string())?;
  let same_repo =
    target.owner.eq_ignore_ascii_case(epic_repo.0) && target.repo.eq_ignore_ascii_case(epic_repo.1);
  let head: Vec<String> = words(title).into_iter().take(3).collect();
  let lines: Vec<String> = body
    .split('\n')
    .map(|line| {
      if let Some(ticked) = tick_full(&task, line) {
        return ticked;
      }
      if same_repo && let Some(ticked) = tick_bare(&bare, line, &head) {
        return ticked;
      }
      line.to_owned()
    })
    .collect();
  Ok(lines.join("\n"))
}

/// The full-reference form, when the match is not followed by a digit.
fn tick_full(task: &Regex, line: &str) -> Option<String> {
  let caps = task.captures(line)?;
  let whole = caps.get(0)?;
  let after = line.get(whole.end()..)?;
  if after.starts_with(|ch: char| ch.is_ascii_digit()) {
    return None;
  }
  Some(format!(
    "{}[x]{}{}{after}",
    caps.get(1)?.as_str(),
    caps.get(2)?.as_str(),
    caps.get(3)?.as_str()
  ))
}

fn tick_bare(bare: &Regex, line: &str, head: &[String]) -> Option<String> {
  let caps = bare.captures(line)?;
  let rest = caps.get(4)?.as_str();
  if rest.starts_with(|ch: char| ch.is_ascii_digit()) {
    return None;
  }
  let following = words(rest);
  let named = !head.is_empty() && following.iter().take(head.len()).eq(head.iter());
  if !(following.is_empty() || named) {
    return None;
  }
  Some(format!(
    "{}[x]{}{}{rest}",
    caps.get(1)?.as_str(),
    caps.get(2)?.as_str(),
    caps.get(3)?.as_str()
  ))
}

/// Lowercase `[a-z0-9]+` words of a title without its `[Prefix]`.
fn words(text: &str) -> Vec<String> {
  strip_prefix(text)
    .to_lowercase()
    .split(|ch: char| !(ch.is_ascii_lowercase() || ch.is_ascii_digit()))
    .filter(|word| !word.is_empty())
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
#[path = "tests/github_body_test.rs"]
mod tests;
