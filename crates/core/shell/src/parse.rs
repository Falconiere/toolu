//! tree-sitter-bash parsing (#416). One `Syntax` serves a whole analysis: the
//! line and every `bash -c`/`eval` string are parsed by the same parser under
//! one shared deadline, so pathological input cannot stall a hook. A parse that
//! runs past the deadline is cancelled and the analysis becomes unknown.
//!
//! Syntax errors are `ERROR` and `MISSING` nodes. tree-sitter keeps the nodes it
//! could read around them, which is how a `git push` before an unterminated
//! quote is still reported.

use std::time::{Duration, Instant};

use tree_sitter::{Language, Node, Parser, Tree};

use crate::analysis::{CommandOrigin, ShellError};
use crate::fixup::{self, Fixups};

/// How many times a script is parsed again for `fixup`'s changes.
const FIXUP_PASSES: usize = 3;

/// How long one analysis may spend parsing, across every nested script.
pub const PARSE_BUDGET: Duration = Duration::from_secs(1);

/// Longest command analyzed, in UTF-16 code units as TypeScript counts `length` (1 MiB).
pub const MAX_SHELL_INPUT: usize = 1024 * 1024;

/// Why a script could not be parsed at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParseFailure {
  /// The grammar could not be loaded into the parser.
  Language(String),
  /// The shared deadline passed.
  Cancelled,
}

impl ParseFailure {
  /// The message an analysis reports.
  pub(crate) fn message(&self) -> String {
    match self {
      ParseFailure::Language(reason) => format!("parser: {reason}"),
      ParseFailure::Cancelled => format!(
        "parser: cancelled after the {} ms parse budget",
        PARSE_BUDGET.as_millis()
      ),
    }
  }
}

/// The bash parser and the deadline it shares across one analysis.
pub(crate) struct Syntax {
  parser: Parser,
  deadline: Instant,
}

impl Syntax {
  /// A parser for one analysis, due `budget` from now.
  pub(crate) fn new(budget: Duration) -> Result<Syntax, ParseFailure> {
    let mut parser = Parser::new();
    let language = Language::new(tree_sitter_bash::LANGUAGE);
    parser
      .set_language(&language)
      .map_err(|error| ParseFailure::Language(error.to_string()))?;
    let deadline = Instant::now() + budget;
    Ok(Syntax { parser, deadline })
  }

  /// Parse a script, then again with `fixup`'s changes until none is left.
  /// The tree indexes the returned text, which has the same length as `source`.
  pub(crate) fn script(&mut self, source: &str) -> Result<(Tree, String), ParseFailure> {
    let mut text = source.to_owned();
    let mut glued = Vec::new();
    let mut tree = self.parse(&text)?;
    for pass in 0..FIXUP_PASSES {
      let found = fixup::find(&tree, &text, pass == 0);
      if found == Fixups::default() {
        break;
      }
      text = fixup::swap(&fixup::blank(&text, &found.blank), &found.swaps);
      if found.swaps.is_empty() {
        glued.extend(found.glued);
      }
      tree = self.parse(&fixup::mask(&text, &glued))?;
    }
    Ok((tree, text))
  }

  /// Parse `source` within what remains of the deadline.
  fn parse(&mut self, source: &str) -> Result<Tree, ParseFailure> {
    let left = self.deadline.saturating_duration_since(Instant::now());
    let micros = u64::try_from(left.as_micros()).unwrap_or(u64::MAX);
    if micros == 0 {
      return Err(ParseFailure::Cancelled);
    }
    self.parser.set_timeout_micros(micros);
    self
      .parser
      .parse(source, None)
      .ok_or(ParseFailure::Cancelled)
  }
}

/// Visit `node` and its descendants in source order with one cursor; `visit`
/// returns whether to go into the node's children.
pub(crate) fn preorder<'t>(node: Node<'t>, mut visit: impl FnMut(Node<'t>) -> bool) {
  let mut cursor = node.walk();
  loop {
    if visit(cursor.node()) && cursor.goto_first_child() {
      continue;
    }
    while !cursor.goto_next_sibling() {
      if !cursor.goto_parent() {
        return;
      }
    }
  }
}

/// The length TypeScript reports for `source`: UTF-16 code units.
pub(crate) fn utf16_len(source: &str) -> usize {
  source.chars().map(char::len_utf16).sum()
}

/// Whether `node` starts a nested script whose errors belong to it.
fn is_substitution(node: Node<'_>) -> bool {
  matches!(node.kind(), "command_substitution" | "process_substitution")
}

/// The message for one `ERROR` or `MISSING` node.
fn error_message(node: Node<'_>, source: &str) -> String {
  if node.is_missing() {
    return match node.kind() {
      "\"" => "unterminated double quote".to_owned(),
      "'" => "unterminated single quote".to_owned(),
      "`" => "unterminated backquote".to_owned(),
      kind => format!("expected '{kind}'"),
    };
  }
  let text = source
    .get(node.byte_range())
    .unwrap_or_default()
    .trim_start();
  match text.chars().next() {
    None => "unexpected end of input".to_owned(),
    Some('\'') => "unterminated single quote".to_owned(),
    Some('"') => "unterminated double quote".to_owned(),
    Some(_) => {
      let head: String = text.chars().take_while(|c| *c != '\n').take(24).collect();
      format!("syntax error near '{head}'")
    }
  }
}

/// Every syntax error in `tree`, in source order. An error inside a
/// substitution belongs to that nested script, as TypeScript reports it.
pub(crate) fn syntax_errors(tree: &Tree, source: &str, origin: CommandOrigin) -> Vec<ShellError> {
  let mut errors = Vec::new();
  let mut stack = vec![(tree.root_node(), origin)];
  while let Some((node, at)) = stack.pop() {
    if crate::redirect::is_read_write(node, source) {
      continue;
    }
    if node.is_error() || node.is_missing() {
      let message = error_message(node, source);
      errors.push(ShellError {
        message,
        pos: node.start_byte(),
        origin: at,
      });
      continue;
    }
    if !node.has_error() {
      continue;
    }
    let inner = if is_substitution(node) {
      CommandOrigin::Substitution
    } else {
      at
    };
    let mut cursor = node.walk();
    let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
    stack.extend(children.into_iter().rev().map(|child| (child, inner)));
  }
  errors
}

#[cfg(test)]
#[path = "tests/parse_test.rs"]
mod tests;
