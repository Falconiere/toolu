//! Heredocs (`shell-words.ts` `heredocContent`, `heredocCat`). An unquoted body
//! expands `$`, backticks and backslash escapes, so it is static only without
//! them; a quoted body is always static data.
//!
//! tree-sitter-bash nests the rest of the line after `<<TAG` inside the
//! `heredoc_redirect` node: further redirects and arguments of the same
//! command, a `|` pipeline, or an `&&`/`||` list. `Heredoc::read` separates
//! them so the walk can put each back where bash runs it.

use tree_sitter::Node;

use crate::analysis::RedirectOperator;
use crate::words::{self, text_of};

/// The parts of one `heredoc_redirect` node.
pub(crate) struct Heredoc<'t> {
  /// `<<` or `<<-`.
  pub(crate) operator: RedirectOperator,
  /// The explicit descriptor, if any.
  pub(crate) fd: Option<u32>,
  /// The delimiter was quoted or escaped: the body is data.
  pub(crate) quoted: bool,
  /// The body node.
  pub(crate) body: Option<Node<'t>>,
  /// Redirects written after the delimiter, on the same command.
  pub(crate) redirects: Vec<Node<'t>>,
  /// Arguments written after the delimiter, of the same command.
  pub(crate) arguments: Vec<Node<'t>>,
  /// End of the last part that belongs to the command line itself.
  pub(crate) line_end: usize,
  /// What follows on the line: `|` and a statement, or `&&`/`||` and a statement.
  pub(crate) continuation: Option<(&'static str, Node<'t>)>,
}

/// The connector a token stands for.
fn connector(token: &str) -> Option<&'static str> {
  match token {
    "|" | "|&" => Some("|"),
    "&&" => Some("&&"),
    "||" => Some("||"),
    _ => None,
  }
}

/// Whether tree-sitter-bash read part of the body of the heredoc `node` into
/// the line of its delimiter. The text between the delimiter and the body is
/// one line in bash; a body line that starts with `\` (`\x`, or a lone `\`
/// joining the next line) was read as words there, and the body lost it.
pub(crate) fn split_body(node: Node<'_>, source: &str) -> bool {
  let mut cursor = node.walk();
  let mut start = None;
  let mut body = node.end_byte();
  for child in node.children(&mut cursor) {
    match child.kind() {
      "heredoc_start" => start = Some(child.end_byte()),
      "heredoc_body" | "heredoc_end" => {
        body = child.start_byte();
        break;
      }
      _ => {}
    }
  }
  let header = start
    .and_then(|from| source.get(from..body))
    .unwrap_or_default();
  // tree-sitter starts a body after its first line's indentation, or after a
  // blank line: only words on a later line mean a split.
  header
    .split_once('\n')
    .is_some_and(|(_, later)| !later.trim().is_empty())
}

impl<'t> Heredoc<'t> {
  /// Read a `heredoc_redirect` node.
  pub(crate) fn read(node: Node<'t>, source: &str) -> Heredoc<'t> {
    let mut heredoc = Heredoc {
      operator: RedirectOperator::Heredoc,
      fd: None,
      quoted: false,
      body: None,
      redirects: Vec::new(),
      arguments: Vec::new(),
      line_end: node.start_byte(),
      continuation: None,
    };
    let mut pending = None;
    let mut cursor = node.walk();
    let mut more = cursor.goto_first_child();
    while more {
      heredoc.take(cursor.node(), cursor.field_name(), source, &mut pending);
      more = cursor.goto_next_sibling();
    }
    heredoc
  }

  fn take(
    &mut self,
    child: Node<'t>,
    field: Option<&str>,
    source: &str,
    pending: &mut Option<&'static str>,
  ) {
    let line = child.end_byte();
    match (child.kind(), field) {
      ("<<-", _) => self.operator = RedirectOperator::HeredocStrip,
      ("file_descriptor", _) => self.fd = text_of(child, source).parse().ok(),
      ("heredoc_start", _) => {
        self.quoted = text_of(child, source).contains(['\'', '"', '\\']);
        self.line_end = line;
      }
      ("heredoc_body", _) => self.body = Some(child),
      (_, Some("right")) => self.continuation = pending.take().map(|op| (op, child)),
      ("pipeline", _) => self.continuation = Some(("|", child)),
      (_, Some("redirect")) => {
        self.redirects.push(child);
        self.line_end = line;
      }
      (_, Some("argument")) => {
        self.arguments.push(child);
        self.line_end = line;
      }
      (kind, _) => {
        if let Some(op) = connector(kind) {
          *pending = Some(op);
        }
      }
    }
  }

  /// The body as the command reads it (`heredocContent`): tabs stripped for
  /// `<<-`; `None` when an unquoted body expands.
  pub(crate) fn content(&self, source: &str) -> Option<String> {
    // tree-sitter starts the body node after the first line's indentation; the
    // body starts on the line after the delimiter's.
    let body = self.body.map_or("", |node| {
      let line = source
        .get(self.line_end..)
        .and_then(|rest| rest.find('\n'))
        .map_or(node.start_byte(), |at| self.line_end + at + 1);
      source
        .get(line.min(node.start_byte())..node.end_byte())
        .unwrap_or_default()
    });
    if !self.quoted && body.contains(['$', '`', '\\']) {
      return None;
    }
    if self.operator != RedirectOperator::HeredocStrip {
      return Some(body.to_owned());
    }
    let stripped: Vec<&str> = body
      .split('\n')
      .map(|line| line.trim_start_matches('\t'))
      .collect();
    Some(stripped.join("\n"))
  }

  /// Nothing else follows the delimiter on the line.
  fn is_bare(&self) -> bool {
    self.redirects.is_empty() && self.arguments.is_empty() && self.continuation.is_none()
  }
}

/// The statements of a script node, skipping comments and separators.
pub(crate) fn statements<'t>(node: Node<'t>) -> Vec<Node<'t>> {
  let mut cursor = node.walk();
  let found: Vec<Node<'t>> = node
    .named_children(&mut cursor)
    .filter(|child| child.kind() != "comment")
    .collect();
  found
}

/// `cat` with nothing else: no assignment, argument or own redirect.
fn is_bare_cat(command: Node<'_>, source: &str) -> bool {
  if command.kind() != "command" || command.named_child_count() != 1 {
    return false;
  }
  let Some(name) = command.child_by_field_name("name") else {
    return false;
  };
  let mut cursor = name.walk();
  let parts: Vec<Node<'_>> = name.named_children(&mut cursor).collect();
  words::resolve(&parts, source).value.as_deref() == Some("cat")
}

/// `"$(cat <<TAG … TAG)"`: the body minus trailing newlines, as command
/// substitution yields it (`heredocCat`). `None` for anything else.
pub(crate) fn cat_body(node: Node<'_>, source: &str) -> Option<String> {
  if node.kind() != "command_substitution" || node.has_error() {
    return None;
  }
  let mut cursor = node.walk();
  if node.children(&mut cursor).any(|child| child.kind() == "&") {
    return None;
  }
  let [statement] = statements(node).try_into().ok()?;
  if statement.kind() != "redirected_statement" {
    return None;
  }
  let command = statement.child_by_field_name("body")?;
  let mut cursor = statement.walk();
  let redirects: Vec<Node<'_>> = statement
    .children_by_field_name("redirect", &mut cursor)
    .collect();
  let [redirect] = redirects.try_into().ok()?;
  if !is_bare_cat(command, source) || redirect.kind() != "heredoc_redirect" {
    return None;
  }
  let heredoc = Heredoc::read(redirect, source);
  if !heredoc.is_bare() {
    return None;
  }
  let content = heredoc.content(source)?;
  Some(content.trim_end_matches('\n').to_owned())
}

#[cfg(test)]
#[path = "tests/heredoc_test.rs"]
mod tests;
