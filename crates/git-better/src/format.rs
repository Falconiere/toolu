//! The semantic commit message: a Conventional Commit subject, an optional body,
//! and a trailer block that names the change's intent. Pure checks over text;
//! the trailer block is read as `git interpret-trailers --parse` reads it, so a
//! `BREAKING CHANGE:` line (a key with a space) hides every trailer.

use crate::settings::Settings;

/// The longest subject line.
pub(crate) const MAX_SUBJECT: usize = 72;
/// The trailer that says why the change exists.
pub(crate) const INTENT: &str = "Intent";
/// The trailer that rates the change's risk.
pub(crate) const RISK: &str = "Risk";
/// The trailer that describes a breaking change.
pub(crate) const BREAKING: &str = "BREAKING-CHANGE";
/// The risk levels `Risk:` accepts.
pub(crate) const RISKS: [&str; 3] = ["low", "medium", "high"];
/// Subjects git writes for `--fixup`, `--squash` and `--fixup=amend:`.
const AUTOSQUASH: [&str; 3] = ["fixup! ", "squash! ", "amend! "];

/// One failed check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Violation {
  /// A stable name for the check, such as `missing-intent`.
  pub(crate) code: &'static str,
  /// What is wrong and how to fix it.
  pub(crate) message: String,
}

impl Violation {
  /// A violation of check `code`.
  pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
    Violation {
      code,
      message: message.into(),
    }
  }
}

/// A Conventional Commit subject: `type(scope)!: description`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Subject<'a> {
  /// The type, such as `feat`.
  pub(crate) kind: &'a str,
  /// The scope, when one is given.
  pub(crate) scope: Option<&'a str>,
  /// `!` before the colon.
  pub(crate) breaking: bool,
  /// The text after `: `.
  pub(crate) description: &'a str,
}

fn scope_char(c: char) -> bool {
  c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '/' | '.' | ',')
}

/// `line` as a Conventional Commit subject, or `None` when it is not one.
pub(crate) fn subject(line: &str) -> Option<Subject<'_>> {
  let len = line.bytes().take_while(u8::is_ascii_lowercase).count();
  let (kind, mut rest) = (line.get(..len)?, line.get(len..)?);
  if kind.is_empty() {
    return None;
  }
  let mut scope = None;
  if let Some(after) = rest.strip_prefix('(') {
    let (inner, tail) = after.split_once(')')?;
    if inner.is_empty() || !inner.chars().all(scope_char) {
      return None;
    }
    scope = Some(inner);
    rest = tail;
  }
  let breaking = rest.starts_with('!');
  let rest = rest.strip_prefix('!').unwrap_or(rest);
  let description = rest.strip_prefix(": ")?;
  (!description.trim().is_empty()).then_some(Subject {
    kind,
    scope,
    breaking,
    description,
  })
}

fn trailer_line(line: &str) -> Option<(&str, &str)> {
  let (key, value) = line.split_once(':')?;
  let token = !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
  token.then(|| (key, value.trim()))
}

/// The trailers of `message`: its last paragraph after the subject, when every
/// line in it is `Key: value` or a continuation; empty otherwise.
pub(crate) fn trailers(message: &str) -> Vec<(String, String)> {
  let lines: Vec<&str> = message.trim_end().lines().collect();
  let Some(blank) = lines.iter().rposition(|line| line.trim().is_empty()) else {
    return Vec::new();
  };
  let mut found: Vec<(String, String)> = Vec::new();
  for line in lines.get(blank + 1..).unwrap_or_default() {
    if line.starts_with([' ', '\t']) {
      let Some((_, value)) = found.last_mut() else {
        return Vec::new();
      };
      value.push(' ');
      value.push_str(line.trim());
      continue;
    }
    let Some((key, value)) = trailer_line(line) else {
      return Vec::new();
    };
    found.push((key.to_owned(), value.to_owned()));
  }
  found
}

/// The first value of trailer `key`, compared case-insensitively as git does.
pub(crate) fn trailer<'a>(found: &'a [(String, String)], key: &str) -> Option<&'a str> {
  found
    .iter()
    .find(|(name, _)| name.eq_ignore_ascii_case(key))
    .map(|(_, value)| value.as_str())
}

/// `message` with `extra` trailers added as `git commit --trailer` adds them.
pub(crate) fn with_trailers(message: &str, extra: &[String]) -> String {
  let message = message.trim_end();
  if extra.is_empty() {
    return message.to_owned();
  }
  let separator = if trailers(message).is_empty() {
    "\n\n"
  } else {
    "\n"
  };
  format!("{message}{separator}{}", extra.join("\n"))
}

/// A `git commit --trailer` value as git writes it: `key=value` and
/// `key: value` both become `key: value`.
pub(crate) fn trailer_arg(value: &str) -> String {
  match value.find([':', '=']) {
    Some(at) => {
      let (key, rest) = (
        value.get(..at).unwrap_or_default(),
        value.get(at + 1..).unwrap_or_default(),
      );
      format!("{}: {}", key.trim(), rest.trim())
    }
    None => value.trim().to_owned(),
  }
}

/// A word written with a capital first letter and lowercase after it (`Add`),
/// not an acronym (`JWT`) or a single letter.
fn capitalized(word: &str) -> bool {
  let mut letters = word.chars();
  letters.next().is_some_and(char::is_uppercase) && letters.next().is_some_and(char::is_lowercase)
}

/// A `wip` subject, which a branch squashes away before it merges.
pub(crate) fn is_wip(message: &str) -> bool {
  let first = message.lines().next().unwrap_or_default().trim();
  first.eq_ignore_ascii_case("wip")
    || ["wip:", "wip(", "wip!"]
      .iter()
      .any(|prefix| first.to_ascii_lowercase().starts_with(prefix))
}

fn check_subject(first: &str, settings: &Settings, found: &mut Vec<Violation>) -> Option<bool> {
  let Some(parsed) = subject(first) else {
    found.push(Violation::new(
      "subject-format",
      format!("the subject `{first}` is not `type(scope): description`"),
    ));
    return None;
  };
  if !settings.types.iter().any(|kind| kind == parsed.kind) {
    let allowed = settings.types.join(", ");
    let why = format!(
      "`{}` is not a commit type; use one of {allowed}",
      parsed.kind
    );
    found.push(Violation::new("unknown-type", why));
  }
  if settings.format.scope && parsed.scope.is_none() {
    let why = "the subject has no scope; name the area it changes: `type(scope): …`";
    found.push(Violation::new("missing-scope", why));
  }
  let length = first.chars().count();
  if length > MAX_SUBJECT {
    let why = format!("the subject is {length} characters; keep it to {MAX_SUBJECT}");
    found.push(Violation::new("subject-length", why));
  }
  if first.ends_with('.') {
    found.push(Violation::new(
      "subject-period",
      "end the subject without a period",
    ));
  }
  if capitalized(parsed.description) {
    let why = "start the description in lowercase, in the imperative: `add …`, `fix …`";
    found.push(Violation::new("subject-case", why));
  }
  Some(parsed.breaking)
}

fn check_trailers(message: &str, bang: Option<bool>, found: &mut Vec<Violation>) {
  if message
    .lines()
    .any(|line| line.starts_with("BREAKING CHANGE:"))
  {
    let why = "write `BREAKING-CHANGE:`; git reads no trailers at all after `BREAKING CHANGE:`";
    found.push(Violation::new("breaking-spelling", why));
    return;
  }
  let block = trailers(message);
  let written = message.lines().skip(1).any(|line| {
    trailer_line(line)
      .is_some_and(|(key, value)| key.eq_ignore_ascii_case(INTENT) && !value.is_empty())
  });
  if trailer(&block, INTENT).is_none_or(str::is_empty) && written {
    let why = "move `Intent:` into the last paragraph, with every other trailer such as \
               `Co-Authored-By:` and no blank line between them; git reads only that paragraph";
    found.push(Violation::new("intent-placement", why));
  } else if trailer(&block, INTENT).is_none_or(str::is_empty) {
    let why = "add an `Intent:` trailer, one line on why this change is needed, in the last \
               paragraph after a blank line";
    found.push(Violation::new("missing-intent", why));
  }
  if let Some(risk) = trailer(&block, RISK).filter(|risk| !RISKS.contains(risk)) {
    let why = format!("`Risk: {risk}` is not one of low, medium, high");
    found.push(Violation::new("risk-value", why));
  }
  let note = trailer(&block, BREAKING).is_some();
  match bang {
    Some(true) if !note => found.push(Violation::new(
      "breaking-note",
      "a `!` subject needs a `BREAKING-CHANGE:` trailer saying what breaks",
    )),
    Some(false) if note => found.push(Violation::new(
      "breaking-bang",
      "a `BREAKING-CHANGE:` trailer needs `!` before the subject's colon",
    )),
    _ => {}
  }
}

/// Every way `message` falls short of the semantic format. `on_base` says the
/// commit lands on the base branch, where `wip` is never allowed.
pub(crate) fn check_message(message: &str, settings: &Settings, on_base: bool) -> Vec<Violation> {
  let mut found = Vec::new();
  let message = message.trim_end();
  let first = message.lines().next().unwrap_or_default();
  if first.trim().is_empty() {
    found.push(Violation::new("empty", "the message has no subject line"));
    return found;
  }
  if AUTOSQUASH.iter().any(|prefix| first.starts_with(prefix)) {
    return found;
  }
  if is_wip(message) {
    if settings.format.wip && !on_base {
      return found;
    }
    let why = if on_base {
      "`wip` commits are allowed only off the base branch"
    } else {
      "`wip` commits are disabled by gitBetter.allowWip"
    };
    found.push(Violation::new("wip", why));
    return found;
  }
  if message
    .lines()
    .nth(1)
    .is_some_and(|line| !line.trim().is_empty())
  {
    let why = "leave a blank line between the subject and the body";
    found.push(Violation::new("body-spacing", why));
  }
  let bang = check_subject(first, settings, &mut found);
  check_trailers(message, bang, &mut found);
  found
}

/// A corrected skeleton for `message`, keeping what can be kept of its subject.
pub(crate) fn example_message(message: &str, settings: &Settings) -> String {
  let first = message.lines().next().unwrap_or_default().trim();
  let parsed = subject(first);
  let kind = parsed
    .map(|found| found.kind)
    .filter(|kind| settings.types.iter().any(|known| known == kind))
    .unwrap_or("<type>");
  let scope = parsed.and_then(|found| found.scope).unwrap_or("<area>");
  let bang = parsed.is_some_and(|found| found.breaking);
  let description = parsed.map_or("<what changed>", |found| {
    found.description.trim().trim_end_matches('.')
  });
  let description = if capitalized(description) {
    let mut letters = description.chars();
    letters.next().map_or_else(String::new, |c| {
      c.to_lowercase().chain(letters).collect::<String>()
    })
  } else {
    description.to_owned()
  };
  let found = trailers(message);
  let intent = trailer(&found, INTENT)
    .filter(|intent| !intent.is_empty())
    .unwrap_or("<why this change is needed>");
  let mut example = format!(
    "{kind}({scope}){}: {description}\n\nIntent: {intent}",
    if bang { "!" } else { "" }
  );
  if bang {
    example.push_str("\nBREAKING-CHANGE: <what breaks and how to migrate>");
  }
  example
}

#[cfg(test)]
#[path = "tests/format_test.rs"]
mod tests;
