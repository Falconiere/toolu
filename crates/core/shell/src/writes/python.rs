//! Python writes (`pythonTargets`): a static `open(<literal>, <write mode>)` in
//! the script `python -c` runs, or a static heredoc on its stdin, reports its
//! path; any other `open(`, `Path(…).open`, `write_text`, `shutil.copy*` or
//! `os.rename` is one unknown target. TypeScript's literal regexes use a
//! backreference, so they are matched by hand here in the same order.

use crate::analysis::ShellCommand;
use crate::options::{OptionSpec, Value, parse_args};
use crate::redirect::stdin_script;
use crate::writes::Target;

const PREFIX: &str = "rRbBuUfF";
const QUOTES: [&str; 4] = ["'''", "\"\"\"", "'", "\""];
const WRITE_API: [&str; 11] = [
  "write_text",
  "write_bytes",
  "touch",
  "symlink_to",
  "hardlink_to",
  "shutil.copy",
  "shutil.move",
  "os.rename",
  "os.replace",
  "os.symlink",
  "os.link",
];

/// A string literal at the front of a text: its prefix, body and the rest after it.
struct Literal<'s> {
  prefix: &'s str,
  body: &'s str,
  rest: &'s str,
}

/// What one `open(` call does.
enum Opened {
  /// It only reads.
  Read,
  /// It writes this path, or a path that cannot be read statically.
  Write(Option<String>),
}

/// The script python runs.
enum Code {
  /// Its static text.
  Text(String),
  /// A script that is not static.
  Dynamic,
}

fn is_word(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

/// The first `quote` in `inside` for which `accept` takes what follows: the
/// body before it and the rest after what `accept` took.
fn close<'s>(
  inside: &'s str,
  quote: &str,
  accept: &impl Fn(&str) -> Option<usize>,
) -> Option<(&'s str, &'s str)> {
  let mut from = 0;
  while let Some(found) = inside.get(from..).and_then(|tail| tail.find(quote)) {
    let at = from + found;
    let tail = inside.get(at + quote.len()..)?;
    if let Some(used) = accept(tail) {
      return Some((inside.get(..at)?, tail.get(used..)?));
    }
    from = at + 1;
  }
  None
}

/// A literal with a prefix of up to two of `rRbBuUfF` and any python quote,
/// longest prefix first, closing on the first quote `accept` takes.
fn literal(text: &str, accept: impl Fn(&str) -> Option<usize>) -> Option<Literal<'_>> {
  let letters = text
    .chars()
    .take(2)
    .take_while(|c| PREFIX.contains(*c))
    .count();
  for split in (0..=letters).rev() {
    let (prefix, after) = (text.get(..split)?, text.get(split..)?);
    for quote in QUOTES {
      let found = after
        .strip_prefix(quote)
        .and_then(|inside| close(inside, quote, &accept));
      if let Some((body, rest)) = found {
        return Some(Literal { prefix, body, rest });
      }
    }
  }
  None
}

/// Whitespace then an opening parenthesis.
fn opens_call(text: &str) -> bool {
  text.trim_start().starts_with('(')
}

/// The mode argument after the path: `, 'w'` or `, mode='w'`, then `,` or `)`.
fn mode(rest: &str) -> Option<Literal<'_>> {
  let after = rest.strip_prefix(',')?.trim_start();
  let after = after
    .strip_prefix("mode")
    .and_then(|named| named.trim_start().strip_prefix('='))
    .map_or(after, str::trim_start);
  literal(after, |tail| {
    let trimmed = tail.trim_start();
    trimmed
      .starts_with([',', ')'])
      .then_some(tail.len() - trimmed.len())
  })
}

/// What one `open(` call does, from its arguments.
fn open_call(args: &str, method: bool) -> Opened {
  if method {
    return Opened::Write(None);
  }
  let spaces = |tail: &str| Some(tail.len() - tail.trim_start().len());
  let Some(path) = literal(args.trim_start(), spaces) else {
    return Opened::Write(None);
  };
  if path.rest.starts_with(')') {
    return Opened::Read;
  }
  let Some(mode) = mode(path.rest) else {
    return Opened::Write(None);
  };
  if !mode.body.contains(['w', 'a', 'x', '+']) {
    return Opened::Read;
  }
  let formatted = path.prefix.contains(['f', 'F']) && path.body.contains('{');
  Opened::Write((!formatted).then(|| path.body.to_owned()))
}

/// Every `open(` call, as written after the parenthesis, and whether it is a method.
fn open_calls(script: &str) -> Vec<(&str, bool)> {
  let mut calls = Vec::new();
  let mut from = 0;
  while let Some(found) = script.get(from..).and_then(|tail| tail.find("open")) {
    let at = from + found;
    from = at + 4;
    let before = script.get(..at).and_then(|head| head.chars().next_back());
    if before.is_some_and(is_word) {
      continue;
    }
    let after = script.get(at + 4..).unwrap_or_default().trim_start();
    if let Some(args) = after.strip_prefix('(') {
      calls.push((args, before == Some('.')));
    }
  }
  calls
}

/// A write API call other than `open(` (`PY_WRITE_API`).
fn writes_otherwise(script: &str) -> bool {
  WRITE_API.iter().any(|name| {
    script.match_indices(name).any(|(at, _)| {
      let before = script.get(..at).and_then(|head| head.chars().next_back());
      let after = script.get(at + name.len()..).unwrap_or_default();
      let copies = name.starts_with("shutil.copy");
      let after = if copies {
        after.trim_start_matches(is_word)
      } else {
        after
      };
      !before.is_some_and(is_word) && opens_call(after)
    })
  })
}

/// The script python runs: `-c STRING`, or its stdin; `None` for a script file or module.
fn script(command: &ShellCommand) -> Option<Code> {
  let spec = OptionSpec {
    value_short: "cmWX",
    stop_at_operand: true,
    ..OptionSpec::default()
  };
  let parsed = parse_args(&command.argv, 1, &spec);
  match parsed.values("c").first() {
    Some(Value::Text(inline)) => return Some(Code::Text((*inline).to_owned())),
    Some(Value::Dynamic) => return Some(Code::Dynamic),
    Some(Value::Absent) | None => {}
  }
  if parsed.has("m") {
    return None;
  }
  let reads_stdin = match command.argv.get(parsed.next) {
    None => true,
    Some(word) => word.as_deref() == Some("-"),
  };
  reads_stdin.then(|| stdin_script(&command.redirects).map_or(Code::Dynamic, Code::Text))
}

/// The paths a static script writes.
fn written(code: &str) -> Vec<Option<String>> {
  let calls = open_calls(code)
    .into_iter()
    .map(|(args, method)| open_call(args, method));
  let mut paths: Vec<Option<String>> = calls
    .filter_map(|call| match call {
      Opened::Write(path) => Some(path),
      Opened::Read => None,
    })
    .collect();
  if writes_otherwise(code) {
    paths.push(None);
  }
  paths
}

/// Every file a python script writes; an unreadable script or write is one unknown target.
pub(super) fn targets(command: &ShellCommand) -> Vec<Target> {
  let paths = match script(command) {
    None => Vec::new(),
    Some(Code::Dynamic) => vec![None],
    Some(Code::Text(code)) => written(&code),
  };
  paths
    .into_iter()
    .map(|path| Target {
      text: path.clone().unwrap_or_default(),
      path,
      pattern: None,
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/python_test.rs"]
mod tests;
