//! Redirections (`shell-words.ts` `toShellRedirect`). A target can run a
//! substitution, and an unquoted heredoc body expands its own; the walk visits
//! the nodes listed in `nested`. `2>&-` is `>&` with target `-`, as unbash reads it.

use tree_sitter::Node;

use crate::analysis::{Heredoc as Body, RedirectOperator, ShellRedirect};
use crate::heredoc::Heredoc;
use crate::words::{self, text_of};

/// One redirection read from the tree.
pub(crate) struct Redirect<'t> {
  /// The record.
  pub(crate) record: ShellRedirect,
  /// Nodes whose nested scripts run: the target, or an unquoted heredoc body.
  pub(crate) nested: Vec<Node<'t>>,
}

/// The operator a token spells, and whether it closes a descriptor (`>&-`).
fn operator(token: &str) -> Option<(RedirectOperator, bool)> {
  match token {
    ">&-" => Some((RedirectOperator::DupOut, true)),
    "<&-" => Some((RedirectOperator::DupIn, true)),
    _ => RedirectOperator::parse(token).map(|op| (op, false)),
  }
}

/// The destination nodes of a file redirect, split into its target and the
/// words after it. tree-sitter-bash reads every word after `>` as the
/// destination; bash reads only the first (`echo x > .env y` writes `.env` and
/// echoes `x y`). Pieces joined by an escaped separator are one word.
pub(crate) fn split_targets<'t>(node: Node<'t>, source: &str) -> (Vec<Node<'t>>, Vec<Node<'t>>) {
  let mut cursor = node.walk();
  let mut nodes = node.children(&mut cursor).filter(|child| {
    child.is_named() && child.kind() != "file_descriptor" && !is_read_write(*child, source)
  });
  let Some(first) = nodes.next() else {
    return (Vec::new(), Vec::new());
  };
  let mut target = vec![first];
  let mut rest = Vec::new();
  for next in nodes {
    let joined = rest.is_empty()
      && target.last().is_some_and(|last: &Node<'t>| {
        words::is_escaped_gap(
          source
            .get(last.end_byte()..next.start_byte())
            .unwrap_or_default(),
        )
      });
    if joined {
      target.push(next);
    } else {
      rest.push(next);
    }
  }
  (target, rest)
}

/// A `file_redirect` or `herestring_redirect`.
fn file<'t>(node: Node<'t>, source: &str) -> Option<Redirect<'t>> {
  let (mut op, mut fd, mut read_write) = (None, None, false);
  let mut cursor = node.walk();
  for child in node.children(&mut cursor) {
    if child.kind() == "file_descriptor" {
      fd = text_of(child, source).parse().ok();
    } else if let (false, Some(found)) = (child.is_named(), operator(child.kind())) {
      op = Some(found);
    }
    read_write |= is_read_write(child, source);
  }
  let (mut operator, closes) = op?;
  if operator == RedirectOperator::In && read_write {
    operator = RedirectOperator::ReadWrite;
  }
  let (targets, _) = split_targets(node, source);
  let target = words::resolve(&targets, source);
  let (value, text) = if closes {
    (Some("-".to_owned()), "-".to_owned())
  } else {
    (target.value, target.text)
  };
  let record = ShellRedirect {
    operator,
    fd,
    target: value,
    pattern: target.pattern,
    text,
    heredoc: None,
  };
  Some(Redirect {
    record,
    nested: targets,
  })
}

/// The `>` of `<>`, which tree-sitter-bash 0.23 does not know: an `ERROR` holding
/// only `>`, right after a `<` token of a file redirect.
pub(crate) fn is_read_write(node: Node<'_>, source: &str) -> bool {
  node.kind() == "ERROR"
    && text_of(node, source) == ">"
    && node
      .parent()
      .is_some_and(|parent| parent.kind() == "file_redirect")
    && node
      .prev_sibling()
      .is_some_and(|before| before.kind() == "<" && before.end_byte() == node.start_byte())
}

/// A `heredoc_redirect`.
fn heredoc<'t>(node: Node<'t>, source: &str) -> Redirect<'t> {
  let parts = Heredoc::read(node, source);
  let body = Body {
    content: parts.content(source),
    quoted: parts.quoted,
  };
  let nested = match (parts.quoted, parts.body) {
    (false, Some(body)) => vec![body],
    _ => Vec::new(),
  };
  let record = ShellRedirect {
    operator: parts.operator,
    fd: parts.fd,
    target: None,
    pattern: None,
    text: String::new(),
    heredoc: Some(body),
  };
  Redirect { record, nested }
}

/// Read a redirection node; `None` when `node` is not one.
pub(crate) fn read<'t>(node: Node<'t>, source: &str) -> Option<Redirect<'t>> {
  match node.kind() {
    "heredoc_redirect" => Some(heredoc(node, source)),
    "file_redirect" | "herestring_redirect" => file(node, source),
    _ => None,
  }
}

/// The script a command reads on stdin: a static heredoc or herestring, else `None` (`stdinScript`).
pub(crate) fn stdin_script(redirects: &[ShellRedirect]) -> Option<String> {
  for redirect in redirects {
    if redirect.fd.is_some_and(|fd| fd != 0) {
      continue;
    }
    if let Some(body) = &redirect.heredoc {
      return body.content.clone();
    }
    if redirect.operator == RedirectOperator::HereString {
      return redirect.target.clone();
    }
  }
  None
}

#[cfg(test)]
#[path = "tests/redirect_test.rs"]
mod tests;
