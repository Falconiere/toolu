//! Where tree-sitter-bash 0.23 reads a script differently from bash, the
//! script is parsed again with the difference removed. Offsets never change,
//! so word values are still read from the script as written.
//!
//! - **`time`:** bash reads one `time [-p]` before a pipeline as a keyword
//!   (`time { git push; }` runs `git push`); tree-sitter reads a command named
//!   `time` with `{` as its argument. The keyword is blanked to spaces.
//! - **Glued openers:** bash reads `[`, `[[` and `{` as a test or a group only
//!   when whitespace follows; `[g]it push` and `{node,} -e x` are a pathname
//!   pattern and a brace expansion in the command's name. tree-sitter opens a
//!   test or a group there, so the opener is masked as a plain word character
//!   for parsing only.

use std::ops::Range;

use tree_sitter::{Node, Tree};

use crate::words::text_of;

/// What to change before parsing again.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Fixups {
  /// `time` keywords to blank.
  pub(crate) blank: Vec<Range<usize>>,
  /// Offsets of glued openers to mask for parsing.
  pub(crate) glued: Vec<usize>,
}

/// Whether `command` starts a pipeline: `time` is a keyword only there.
fn starts_pipeline(command: Node<'_>) -> bool {
  match command.parent() {
    Some(parent) if parent.kind() == "pipeline" => parent
      .named_child(0)
      .is_some_and(|first| first.id() == command.id()),
    _ => true,
  }
}

/// The bytes of `time` and a following `-p` in `command`, if it starts with the keyword.
fn time_of(command: Node<'_>, source: &str) -> Option<Range<usize>> {
  let name = command.child_by_field_name("name")?;
  let word = name
    .named_child(0)
    .filter(|_| name.named_child_count() == 1)?;
  if word.kind() != "word" || text_of(word, source) != "time" || !starts_pipeline(command) {
    return None;
  }
  let mut cursor = command.walk();
  let first = command
    .children_by_field_name("argument", &mut cursor)
    .next();
  let end = first
    .filter(|argument| argument.kind() == "word" && text_of(*argument, source) == "-p")
    .map_or(name.end_byte(), |argument| argument.end_byte());
  Some(name.start_byte()..end)
}

/// The offset of `token` when it is a `[`, `[[` or `{` that tree-sitter took
/// for an opener (of a test, a group, or inside an error) with no whitespace after it.
fn glued_of(token: Node<'_>, source: &str) -> Option<usize> {
  if token.is_named() || !matches!(token.kind(), "[" | "[[" | "{") {
    return None;
  }
  let parent = token.parent()?;
  if !matches!(
    parent.kind(),
    "test_command" | "compound_statement" | "ERROR"
  ) {
    return None;
  }
  let after = source
    .get(token.end_byte()..)
    .and_then(|rest| rest.chars().next());
  after
    .is_some_and(|c| !c.is_whitespace())
    .then_some(token.start_byte())
}

/// Every fixup in `tree`; `time` keywords only when `time` is set.
pub(crate) fn find(tree: &Tree, source: &str, time: bool) -> Fixups {
  let mut found = Fixups::default();
  let time = time && source.contains("time");
  let mut stack = vec![tree.root_node()];
  while let Some(node) = stack.pop() {
    if time && node.kind() == "command" {
      found.blank.extend(time_of(node, source));
    }
    found.glued.extend(glued_of(node, source));
    let mut cursor = node.walk();
    stack.extend(node.children(&mut cursor));
  }
  found
}

/// `source` with every byte in `ranges` replaced by `byte`.
fn fill(source: &str, ranges: impl Iterator<Item = Range<usize>>, byte: u8) -> String {
  let mut bytes = source.as_bytes().to_vec();
  for range in ranges {
    if let Some(slice) = bytes.get_mut(range) {
      slice.fill(byte);
    }
  }
  String::from_utf8(bytes).unwrap_or_else(|_| source.to_owned())
}

/// `source` with the `time` keywords blanked.
pub(crate) fn blank(source: &str, ranges: &[Range<usize>]) -> String {
  fill(source, ranges.iter().cloned(), b' ')
}

/// `source` with each glued opener masked as `_` (one ASCII byte each).
pub(crate) fn mask(source: &str, glued: &[usize]) -> String {
  fill(source, glued.iter().map(|at| *at..at + 1), b'_')
}

#[cfg(test)]
#[path = "tests/fixup_test.rs"]
mod tests;
