//! The semantic branch name: `<type>/[<ref>-]<slug>`, such as
//! `feat/653-pause-and-resume-an-agent` or `fix/ENG-12-retry-timeouts`. The
//! reference is an issue number or a tracker key; the slug is lowercase kebab case.

use crate::format::Violation;
use crate::settings::Settings;

/// The longest branch name.
pub(crate) const MAX_BRANCH: usize = 64;

/// A semantic branch name, split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Branch<'a> {
  /// The commit type before the `/`.
  pub(crate) kind: &'a str,
  /// The issue number or tracker key, when one leads the rest.
  pub(crate) reference: Option<&'a str>,
  /// The lowercase kebab-case description.
  pub(crate) slug: &'a str,
}

fn is_slug(text: &str) -> bool {
  !text.is_empty()
    && text.split('-').all(|word| {
      !word.is_empty()
        && word
          .bytes()
          .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// `653-rest` or `ENG-12-rest`: the reference and the rest after its `-`.
fn reference(rest: &str) -> Option<(&str, &str)> {
  let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
  if digits > 0 {
    let (number, tail) = (rest.get(..digits)?, rest.get(digits..)?);
    return Some((number, tail.strip_prefix('-').unwrap_or(tail)));
  }
  let key = rest
    .bytes()
    .take_while(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    .count();
  let (project, tail) = (rest.get(..key)?, rest.get(key..)?);
  let number = tail.strip_prefix('-')?;
  let digits = number.bytes().take_while(u8::is_ascii_digit).count();
  if key == 0 || !project.starts_with(|c: char| c.is_ascii_uppercase()) || digits == 0 {
    return None;
  }
  let end = key + 1 + digits;
  let (whole, tail) = (rest.get(..end)?, rest.get(end..)?);
  Some((whole, tail.strip_prefix('-').unwrap_or(tail)))
}

/// `name` split into its parts, or `None` when it has no `<type>/` prefix.
pub(crate) fn parse(name: &str) -> Option<Branch<'_>> {
  let (kind, rest) = name.split_once('/')?;
  let (reference, slug) = match reference(rest) {
    Some((found, slug)) => (Some(found), slug),
    None => (None, rest),
  };
  Some(Branch {
    kind,
    reference,
    slug,
  })
}

/// Every way `name` falls short of the semantic branch format.
pub(crate) fn check_branch(name: &str, settings: &Settings) -> Vec<Violation> {
  let mut found = Vec::new();
  let Some(branch) = parse(name) else {
    let why = format!("the branch `{name}` is not `<type>/[<ref>-]<slug>`");
    found.push(Violation::new("branch-format", why));
    return found;
  };
  if !settings.types.iter().any(|kind| kind == branch.kind) {
    let allowed = settings.types.join(", ");
    let why = format!(
      "`{}` is not a commit type; use one of {allowed}",
      branch.kind
    );
    found.push(Violation::new("branch-type", why));
  }
  if branch.slug.contains('/') {
    let why = "use a single `/`, after the type";
    found.push(Violation::new("branch-depth", why));
  } else if !is_slug(branch.slug) {
    let why = format!(
      "`{}` is not a lowercase kebab-case description such as `retry-timeouts`",
      branch.slug
    );
    found.push(Violation::new("branch-slug", why));
  }
  if settings.require_ref && branch.reference.is_none() {
    let why = "start the description with the issue reference: `<type>/<ref>-<slug>`";
    found.push(Violation::new("branch-ref", why));
  }
  let length = name.chars().count();
  if length > MAX_BRANCH {
    let why = format!("the branch is {length} characters; keep it to {MAX_BRANCH}");
    found.push(Violation::new("branch-length", why));
  }
  found
}

/// `text` as a kebab-case slug: lowercase words of letters and digits.
fn slugify(text: &str) -> String {
  let lower = text.to_lowercase();
  let words: Vec<&str> = lower
    .split(|c: char| !c.is_ascii_alphanumeric())
    .filter(|word| !word.is_empty())
    .collect();
  words.join("-")
}

/// A corrected branch name for `name`, keeping its type, reference and words.
pub(crate) fn example_branch(name: &str, settings: &Settings) -> String {
  let parsed = parse(name);
  let kind = parsed
    .map(|branch| branch.kind)
    .filter(|kind| settings.types.iter().any(|known| known == kind))
    .unwrap_or("<type>");
  let reference = parsed
    .and_then(|branch| branch.reference)
    .map_or_else(String::new, |found| format!("{found}-"));
  let words = parsed.map_or(name, |branch| branch.slug);
  let slug = Some(slugify(words))
    .filter(|slug| !slug.is_empty())
    .unwrap_or_else(|| "<short-description>".to_owned());
  format!("{kind}/{reference}{slug}")
}

#[cfg(test)]
#[path = "tests/branch_test.rs"]
mod tests;
