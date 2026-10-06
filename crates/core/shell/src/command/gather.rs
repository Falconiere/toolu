//! The words, assignments and redirects of one simple command, as bash reads
//! them from tree-sitter-bash's nodes: words split at an escaped separator are
//! joined, what tree-sitter nested after a heredoc delimiter is folded back in,
//! and a lone `-` that its heredoc scanner drops (`python3 - <<EOF`) is restored.

use tree_sitter::Node;

use crate::heredoc::Heredoc;
use crate::redirect;
use crate::words::{self, Resolved, text_of};

/// Node kinds that are one word of a `[ … ]` test.
const TEST_WORDS: &[&str] = &[
  "word",
  "string",
  "raw_string",
  "ansi_c_string",
  "translated_string",
  "number",
  "simple_expansion",
  "expansion",
  "command_substitution",
  "process_substitution",
  "arithmetic_expansion",
  "concatenation",
  "test_operator",
  "regex",
  "extglob_pattern",
  "variable_name",
  "brace_expression",
];

/// One word of the command: its nodes, or text tree-sitter dropped.
pub(super) struct WordNodes<'t> {
  /// The nodes, more than one when a backslash-newline split the word.
  pub(super) nodes: Vec<Node<'t>>,
  /// Text with no node, restored from the source.
  pub(super) lost: Option<String>,
  /// Where the word starts.
  pub(super) start: usize,
}

impl WordNodes<'_> {
  /// Resolve the word statically.
  pub(super) fn resolve(&self, source: &str) -> Resolved {
    match &self.lost {
      Some(text) => words::literal(text),
      None => words::resolve(&self.nodes, source),
    }
  }

  /// The word when it is one plain unquoted token (`time`, `coproc`, `fi`).
  pub(super) fn keyword<'s>(&self, source: &'s str) -> Option<&'s str> {
    match self.nodes.as_slice() {
      [node] if node.kind() == "word" && self.lost.is_none() => Some(text_of(*node, source)),
      _ => None,
    }
  }
}

/// Everything one simple command is made of.
pub(super) struct Gathered<'t> {
  pub(super) assignments: Vec<Node<'t>>,
  pub(super) words: Vec<WordNodes<'t>>,
  pub(super) redirects: Vec<Node<'t>>,
  /// Other statements tree-sitter hung under the command (`time (…)`); they run too.
  pub(super) others: Vec<Node<'t>>,
  /// Descriptors written as a word glued to their redirect (`0<file`), by redirect start.
  pub(super) fds: Vec<(usize, u32)>,
  /// Where the command's text ends: its last word or redirect, never a heredoc body.
  pub(super) end: usize,
}

/// The words of a `[ … ]` test, in source order, brackets included.
pub(super) fn test_words(node: Node<'_>) -> Vec<Node<'_>> {
  let mut found = Vec::new();
  let mut stack = vec![node];
  while let Some(next) = stack.pop() {
    let leaf = next.child_count() == 0 && next.id() != node.id();
    if (next.is_named() && TEST_WORDS.contains(&next.kind())) || leaf {
      found.push(next);
      continue;
    }
    let mut cursor = next.walk();
    let children: Vec<Node<'_>> = next.children(&mut cursor).collect();
    stack.extend(children.into_iter().rev());
  }
  found
}

/// A node that stands for nothing: tree-sitter inserted it to recover.
fn is_hollow(node: Node<'_>) -> bool {
  node.is_missing() || node.byte_range().is_empty()
}

/// Sort the children of a command node into `gathered`.
fn sort_children<'t>(node: Node<'t>, gathered: &mut Gathered<'t>, nodes: &mut Vec<Node<'t>>) {
  let command = node.kind() == "command";
  let mut cursor = node.walk();
  let mut more = cursor.goto_first_child();
  while more {
    let (child, field) = (cursor.node(), cursor.field_name());
    match (child.kind(), field) {
      ("variable_assignment" | "variable_assignments", _) if command => {
        gathered.assignments.push(child);
      }
      (kind, _) if kind.ends_with("_redirect") => gathered.redirects.push(child),
      ("command_name", _) => {
        let mut inner = child.walk();
        nodes.extend(
          child
            .named_children(&mut inner)
            .filter(|word| !is_hollow(*word)),
        );
      }
      ("comment", _) => {}
      (_, Some("argument")) => nodes.push(child),
      _ if !command && !is_hollow(child) => nodes.push(child),
      _ if child.is_named() && !is_hollow(child) => gathered.others.push(child),
      _ => {}
    }
    more = cursor.goto_next_sibling();
  }
}

/// Text between two nodes of one bash word: nothing, or escaped separators.
fn joins(gap: &str) -> bool {
  gap.is_empty() || words::is_escaped_gap(gap)
}

/// Group word nodes that tree-sitter split at an escaped separator, which bash
/// does not split at: `node -\<newline>e`, `'node'\ '-e x'`.
fn join_escaped<'t>(nodes: Vec<Node<'t>>, source: &str) -> Vec<WordNodes<'t>> {
  let mut words: Vec<WordNodes<'t>> = Vec::new();
  for node in nodes {
    let joined = words
      .last()
      .and_then(|word| word.nodes.last())
      .is_some_and(|last| {
        joins(
          source
            .get(last.end_byte()..node.start_byte())
            .unwrap_or_default(),
        )
      });
    match words.last_mut() {
      Some(word) if joined => word.nodes.push(node),
      _ => words.push(WordNodes {
        nodes: vec![node],
        lost: None,
        start: node.start_byte(),
      }),
    }
  }
  words
}

/// Words tree-sitter dropped just before a heredoc redirect: the source between
/// the previous node and the redirect, and a `-` its operator token swallowed.
fn lost_words(redirect: Node<'_>, previous_end: usize, source: &str) -> Vec<WordNodes<'static>> {
  let mut lost = Vec::new();
  let gap = source
    .get(previous_end..redirect.start_byte())
    .unwrap_or_default();
  for (offset, text) in gap
    .split_whitespace()
    .map(|word| (gap.find(word).unwrap_or(0), word))
  {
    lost.push(WordNodes {
      nodes: Vec::new(),
      lost: Some(text.to_owned()),
      start: previous_end + offset,
    });
  }
  let swallowed = redirect
    .child(0)
    .filter(|token| text_of(*token, source).starts_with("-<<"));
  if let Some(token) = swallowed {
    lost.push(WordNodes {
      nodes: Vec::new(),
      lost: Some("-".to_owned()),
      start: token.start_byte(),
    });
  }
  lost
}

/// Fold what a heredoc nested after its delimiter back into the command.
fn fold_heredocs<'t>(
  gathered: &mut Gathered<'t>,
  nodes: &mut Vec<Node<'t>>,
  source: &str,
) -> Vec<WordNodes<'t>> {
  let mut lost = Vec::new();
  let mut extra = Vec::new();
  for redirect in &gathered.redirects {
    if redirect.kind() != "heredoc_redirect" {
      gathered.end = gathered.end.max(redirect.end_byte());
      if redirect.kind() == "file_redirect" {
        nodes.extend(redirect::split_targets(*redirect, source).1);
      }
      continue;
    }
    let before = nodes.iter().chain(&gathered.redirects).map(Node::end_byte);
    let previous_end = before.filter(|end| *end <= redirect.start_byte()).max();
    lost.extend(lost_words(
      *redirect,
      previous_end.unwrap_or(redirect.start_byte()),
      source,
    ));
    let heredoc = Heredoc::read(*redirect, source);
    gathered.end = gathered.end.max(heredoc.line_end);
    nodes.extend(heredoc.arguments);
    for inner in &heredoc.redirects {
      if inner.kind() == "file_redirect" {
        nodes.extend(redirect::split_targets(*inner, source).1);
      }
    }
    extra.extend(heredoc.redirects);
  }
  gathered.redirects.extend(extra);
  lost
}

/// A word glued to the redirect after it that bash reads as its descriptor:
/// `{name}` (a variable, no number) or digits.
fn glued_fd(node: Node<'_>, redirects: &[Node<'_>], source: &str) -> Option<(usize, Option<u32>)> {
  let at = redirects
    .binary_search_by_key(&node.end_byte(), Node::start_byte)
    .ok()?;
  let redirect = redirects.get(at)?;
  if redirect.child_by_field_name("descriptor").is_some() {
    return None;
  }
  let text = text_of(node, source);
  if let Some(name) = text
    .strip_prefix('{')
    .and_then(|rest| rest.strip_suffix('}'))
  {
    let ident = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    return ident.then_some((redirect.start_byte(), None));
  }
  let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
  digits.then(|| (redirect.start_byte(), text.parse().ok()))
}

/// Drop descriptor words from `nodes`, recording digit descriptors.
fn take_fds(nodes: &mut Vec<Node<'_>>, gathered: &mut Gathered<'_>, source: &str) {
  nodes.retain(|node| match glued_fd(*node, &gathered.redirects, source) {
    Some((at, fd)) => {
      gathered.fds.extend(fd.map(|fd| (at, fd)));
      false
    }
    None => true,
  });
}

/// Gather the simple command `node` with the `statement` redirects written after it.
pub(super) fn gather<'t>(node: Node<'t>, statement: &[Node<'t>], source: &str) -> Gathered<'t> {
  let simple = node.kind() != "redirected_statement";
  let end = if simple {
    node.end_byte()
  } else {
    node.start_byte()
  };
  let mut gathered = Gathered {
    assignments: Vec::new(),
    words: Vec::new(),
    redirects: Vec::new(),
    others: Vec::new(),
    fds: Vec::new(),
    end,
  };
  let mut nodes = Vec::new();
  match node.kind() {
    "test_command" => nodes = test_words(node),
    "variable_assignment" | "variable_assignments" => gathered.assignments.push(node),
    kind if kind == "redirected_statement" || kind.ends_with("_redirect") => {}
    _ => sort_children(node, &mut gathered, &mut nodes),
  }
  gathered.redirects.extend_from_slice(statement);
  let lost = fold_heredocs(&mut gathered, &mut nodes, source);
  gathered.redirects.sort_by_key(Node::start_byte);
  nodes.sort_by_key(Node::start_byte);
  take_fds(&mut nodes, &mut gathered, source);
  gathered.words = join_escaped(nodes, source);
  gathered.words.extend(lost);
  gathered.words.sort_by_key(|word| word.start);
  gathered
}

#[cfg(test)]
#[path = "tests/gather_test.rs"]
mod tests;
