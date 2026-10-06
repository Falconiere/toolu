//! Words (`shell-words.ts` `scanWord`). A word is static only when every part
//! is literal, so `"$(cat <<'EOF' … EOF)"`, the form agents write commit
//! messages in, resolves to its heredoc body while anything that expands at
//! run time is `None`. A pathname pattern (`.en[v]`, `*.log`) is not a value:
//! its `pattern` carries it. Nested scripts are visited by the walk, not here.
//!
//! tree-sitter-bash splits one word into several nodes in two cases that bash
//! does not: a `concatenation` of pieces, and a word broken by a backslash-
//! newline. A logical word is therefore a list of nodes.

pub(crate) mod quote;

use tree_sitter::Node;

use crate::analysis::Word;
use crate::heredoc;

/// A word resolved statically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
  /// The value when every part is literal and nothing globs, else `None`.
  pub(crate) value: Word,
  /// For a word whose only expansion is pathname globbing, its unexpanded pattern.
  pub(crate) pattern: Option<String>,
  /// Quotes removed, expansions left as written (`"$HOME/.env"` is `$HOME/.env`).
  pub(crate) text: String,
}

/// One part of a word, before the parts are joined.
enum Piece {
  /// Unquoted literal source text, escapes not yet removed.
  Literal(String),
  /// A quoted static value and its text.
  Quoted(String),
  /// An expansion: no static value; its text as written.
  Dynamic(String),
  /// An extended glob: its text is both value and pattern.
  Glob(String),
  /// `"$(cat <<'EOF' … EOF)"`: the heredoc body as value, the substitution as written.
  Cat(String, String),
}

/// The source text of `node`.
pub(crate) fn text_of<'s>(node: Node<'_>, source: &'s str) -> &'s str {
  source.get(node.byte_range()).unwrap_or_default()
}

/// The pieces between `from` and `to` inside a double-quoted string.
fn string_pieces(node: Node<'_>, source: &str, open: usize, pieces: &mut Vec<Piece>) {
  let text = text_of(node, source);
  let closed = text.len() > open && text.ends_with('"');
  let (from, to) = (
    node.start_byte() + open,
    node.end_byte() - usize::from(closed),
  );
  let mut at = from;
  let mut cursor = node.walk();
  for child in node.named_children(&mut cursor) {
    if child.start_byte() < at || child.end_byte() > to || child.kind() == "string_content" {
      continue;
    }
    let gap = source.get(at..child.start_byte()).unwrap_or_default();
    pieces.push(Piece::Quoted(quote::double_quoted(gap)));
    pieces.push(match heredoc::cat_body(child, source) {
      Some(body) => Piece::Cat(body, text_of(child, source).to_owned()),
      None => Piece::Dynamic(text_of(child, source).to_owned()),
    });
    at = child.end_byte();
  }
  let tail = source.get(at..to).unwrap_or_default();
  pieces.push(Piece::Quoted(quote::double_quoted(tail)));
}

/// The body of a quoted node whose delimiters are `open` and `close` bytes long.
fn between(node: Node<'_>, source: &str, open: usize, close: &str) -> String {
  let text = text_of(node, source);
  let inner = text.get(open..).unwrap_or_default();
  inner.strip_suffix(close).unwrap_or(inner).to_owned()
}

/// Split one node of a word into pieces.
fn pieces_of(node: Node<'_>, source: &str, pieces: &mut Vec<Piece>) {
  match node.kind() {
    "concatenation" | "variable_assignment" => {
      let mut cursor = node.walk();
      for child in node.children(&mut cursor) {
        pieces_of(child, source, pieces);
      }
    }
    "string" => string_pieces(node, source, 1, pieces),
    "translated_string" => string_pieces(node, source, 2, pieces),
    "raw_string" => pieces.push(Piece::Quoted(between(node, source, 1, "'"))),
    "ansi_c_string" => pieces.push(Piece::Quoted(quote::ansi_c(&between(node, source, 2, "'")))),
    "extglob_pattern" => pieces.push(Piece::Glob(text_of(node, source).to_owned())),
    "simple_expansion"
    | "expansion"
    | "command_substitution"
    | "process_substitution"
    | "arithmetic_expansion"
    | "brace_expression" => {
      pieces.push(Piece::Dynamic(text_of(node, source).to_owned()));
    }
    _ => pieces.push(Piece::Literal(text_of(node, source).to_owned())),
  }
}

/// Join pieces: merged unquoted literal runs decide whether the word globs.
fn join(pieces: Vec<Piece>) -> Resolved {
  let (mut value, mut text) = (Some(String::new()), String::new());
  let (mut glob, mut braces, mut run) = (false, false, String::new());
  for piece in pieces {
    if !matches!(piece, Piece::Literal(_)) {
      let flushed = std::mem::take(&mut run);
      glob |= quote::has_glob(&flushed);
      braces |= quote::has_brace(&flushed);
    }
    let (part, shown) = match piece {
      Piece::Literal(raw) => {
        run.push_str(&raw);
        let plain = quote::unquoted(&raw);
        (Some(plain.clone()), plain)
      }
      Piece::Quoted(plain) => (Some(plain.clone()), plain),
      Piece::Cat(body, written) => (Some(body), written),
      Piece::Dynamic(written) => (None, written),
      Piece::Glob(written) => {
        glob = true;
        (Some(written.clone()), written)
      }
    };
    text.push_str(&shown);
    value = value.zip(part).map(|(mut joined, part)| {
      joined.push_str(&part);
      joined
    });
  }
  glob |= quote::has_glob(&run);
  braces |= quote::has_brace(&run);
  if braces {
    value = None;
  }
  match value {
    Some(found) if glob => Resolved {
      value: None,
      pattern: Some(found),
      text,
    },
    value => Resolved {
      value,
      pattern: None,
      text,
    },
  }
}

/// Text made only of backslash-escaped characters (`\<newline>`, `\ `): bash
/// keeps it inside the word, but tree-sitter-bash reads it as a separator.
pub(crate) fn is_escaped_gap(gap: &str) -> bool {
  let mut chars = gap.chars();
  let mut any = false;
  while let Some(c) = chars.next() {
    if c != '\\' || chars.next().is_none() {
      return false;
    }
    any = true;
  }
  any
}

/// Resolve a word tree-sitter left no node for, from its unquoted source text.
pub(crate) fn literal(raw: &str) -> Resolved {
  join(vec![Piece::Literal(raw.to_owned())])
}

/// Resolve the logical word made of `nodes`.
/// The text between two nodes of one word (an escaped separator) is literal.
pub(crate) fn resolve(nodes: &[Node<'_>], source: &str) -> Resolved {
  let mut pieces = Vec::new();
  let mut previous: Option<Node<'_>> = None;
  for node in nodes {
    if let Some(before) = previous {
      let gap = source
        .get(before.end_byte()..node.start_byte())
        .unwrap_or_default();
      pieces.push(Piece::Literal(gap.to_owned()));
    }
    pieces_of(*node, source, &mut pieces);
    previous = Some(*node);
  }
  join(pieces)
}

#[cfg(test)]
#[path = "tests/words_test.rs"]
mod tests;
