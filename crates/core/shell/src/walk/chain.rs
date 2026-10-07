//! One statement as bash runs it: an and-or list of pipelines (#416).
//!
//! tree-sitter-bash nests `a && b && c` left-recursively, nests whatever follows
//! `<<TAG` inside the heredoc redirect (`cat <<EOF | bash`), and can put a whole
//! list under one redirect (`a && b 2>&1 | c`, which bash reads as
//! `a && (b 2>&1 | c)`). All three are flattened here without recursion into
//! the shape unbash produced: pipelines and the operators between them, with a
//! list's redirects on its last command.

use tree_sitter::Node;

use crate::heredoc::Heredoc;
use crate::walk::redirects_of;

/// One command of a pipeline, and redirects an enclosing statement adds to it.
pub(crate) struct Element<'t> {
  /// The command or compound command.
  pub(crate) node: Node<'t>,
  /// Redirects tree-sitter put on a list that ends with this command.
  pub(crate) redirects: Vec<Node<'t>>,
}

/// One pipeline of an and-or list.
pub(crate) struct Pipeline<'t> {
  /// Written with a leading `!`.
  pub(crate) negated: bool,
  /// The commands joined by `|`.
  pub(crate) elements: Vec<Element<'t>>,
}

/// An and-or list: pipelines and the `&&`/`||` between them.
pub(crate) struct Chain<'t> {
  /// The pipelines, in order.
  pub(crate) pipelines: Vec<Pipeline<'t>>,
  /// `operators[i]` joins `pipelines[i]` and `pipelines[i + 1]`.
  pub(crate) operators: Vec<&'static str>,
}

/// A step of the flattening.
enum Work<'t> {
  /// A node still to flatten.
  Expand(Node<'t>),
  /// A command of a pipeline.
  Element(Node<'t>),
  /// Redirects for the last command flattened so far.
  Attach(Vec<Node<'t>>),
  /// `|`, `&&` or `||`.
  Link(&'static str),
  /// A leading `!`.
  Not,
}

/// The connector a token stands for.
fn link(token: &str) -> Option<&'static str> {
  match token {
    "|" | "|&" => Some("|"),
    "&&" => Some("&&"),
    "||" => Some("||"),
    _ => None,
  }
}

/// What follows the heredoc delimiter of a `redirected_statement`, if anything.
fn continuation<'t>(redirects: &[Node<'t>], source: &str) -> Option<(&'static str, Node<'t>)> {
  let (op, next) = redirects
    .iter()
    .filter(|redirect| redirect.kind() == "heredoc_redirect")
    .find_map(|redirect| Heredoc::read(*redirect, source).continuation)?;
  if op != "|" {
    return Some((op, next));
  }
  let mut cursor = next.walk();
  let statement = next.named_children(&mut cursor).next()?;
  Some((op, statement))
}

/// A `redirected_statement`: the command with its redirects, or a list whose
/// last command takes them; then whatever a heredoc nested after its delimiter.
fn redirected<'t>(node: Node<'t>, source: &str) -> Vec<Work<'t>> {
  let redirects = redirects_of(node);
  let follow = continuation(&redirects, source);
  let body = node.child_by_field_name("body");
  let mut steps = match body {
    Some(body) if matches!(body.kind(), "list" | "pipeline" | "negated_command") => {
      vec![Work::Expand(body), Work::Attach(redirects)]
    }
    _ => vec![Work::Element(node)],
  };
  if let Some((op, next)) = follow {
    steps.extend([Work::Link(op), Work::Expand(next)]);
  }
  steps
}

/// The steps one node expands to, in order.
fn expand<'t>(node: Node<'t>, source: &str) -> Vec<Work<'t>> {
  match node.kind() {
    "list" | "pipeline" => {
      let mut cursor = node.walk();
      let parts: Vec<Work<'t>> = node
        .children(&mut cursor)
        .filter_map(|child| match (child.is_named(), link(child.kind())) {
          (true, _) if child.kind() != "comment" => Some(Work::Expand(child)),
          (false, Some(op)) => Some(Work::Link(op)),
          _ => None,
        })
        .collect();
      parts
    }
    "negated_command" => {
      let mut cursor = node.walk();
      let inner = node.named_children(&mut cursor).next();
      std::iter::once(Work::Not)
        .chain(inner.map(Work::Expand))
        .collect()
    }
    "redirected_statement" => redirected(node, source),
    _ => vec![Work::Element(node)],
  }
}

/// Group the flat steps into pipelines and operators.
fn group(flat: Vec<Work<'_>>) -> Chain<'_> {
  let empty = || Pipeline {
    negated: false,
    elements: Vec::new(),
  };
  let mut chain = Chain {
    pipelines: Vec::new(),
    operators: Vec::new(),
  };
  let mut current = empty();
  for work in flat {
    match work {
      Work::Element(node) | Work::Expand(node) if node.byte_range().is_empty() => {}
      Work::Element(node) | Work::Expand(node) => {
        current.elements.push(Element {
          node,
          redirects: Vec::new(),
        });
      }
      Work::Attach(redirects) => {
        let previous = chain.pipelines.last_mut();
        let last = current
          .elements
          .last_mut()
          .or_else(|| previous.and_then(|done| done.elements.last_mut()));
        if let Some(last) = last {
          last.redirects.extend(redirects);
        }
      }
      Work::Not => current.negated = true,
      Work::Link("|") => {}
      Work::Link(op) => {
        chain
          .pipelines
          .push(std::mem::replace(&mut current, empty()));
        chain.operators.push(op);
      }
    }
  }
  chain.pipelines.push(current);
  chain
}

/// Flatten `statement` into an and-or list of pipelines.
pub(crate) fn chain<'t>(statement: Node<'t>, source: &str) -> Chain<'t> {
  let mut stack = vec![Work::Expand(statement)];
  let mut flat = Vec::new();
  while let Some(work) = stack.pop() {
    match work {
      Work::Expand(node) => stack.extend(expand(node, source).into_iter().rev()),
      done @ (Work::Element(_) | Work::Attach(_) | Work::Link(_) | Work::Not) => flat.push(done),
    }
  }
  group(flat)
}

#[cfg(test)]
#[path = "tests/chain_test.rs"]
mod tests;
