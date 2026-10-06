//! The walk (`shell-walk.ts`): every simple command of a parsed script, with
//! where it runs and whether a zero exit status of the line proves it passed
//! (only the last statement, `&&` on both sides, the last element of a
//! non-negated pipeline). Only entering a nested script or compound body
//! recurses, counted against `MAX_NESTING`; everything else is iterative.

pub(crate) mod chain;
mod compound;
mod nested;

use tree_sitter::{Node, Tree};

use crate::analysis::{CommandOrigin, PipelinePosition, ShellCommand, ShellError, ShellRedirect};
use crate::command;
use crate::parse::{Syntax, syntax_errors};

/// How deep nested scripts and compound bodies are followed before the line is unknown.
pub const MAX_NESTING: usize = 64;

/// How deep `bash -c` / `eval` strings are followed before the rest is unknown.
pub const MAX_RUN_DEPTH: usize = 4;

/// Where the walk is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ctx<'s> {
  /// The text node positions index: the line or an inner `-c` string.
  pub(crate) source: &'s str,
  pub(crate) origin: CommandOrigin,
  /// `bash -c` / `eval` depth.
  pub(crate) depth: usize,
  pub(crate) proves: bool,
  pub(crate) pipeline: PipelinePosition,
  /// Nested scripts and compound bodies entered so far.
  pub(crate) nesting: usize,
}

impl<'s> Ctx<'s> {
  /// The context of a whole line.
  pub(crate) fn line(source: &'s str) -> Ctx<'s> {
    Ctx {
      source,
      origin: CommandOrigin::Line,
      depth: 0,
      proves: true,
      pipeline: PipelinePosition::ALONE,
      nesting: 0,
    }
  }

  /// The same place, but nothing here proves the line's status.
  pub(crate) fn off(self) -> Ctx<'s> {
    Ctx {
      proves: false,
      ..self
    }
  }
}

/// What the walk has found.
pub(crate) struct Walker {
  pub(crate) syntax: Syntax,
  pub(crate) commands: Vec<ShellCommand>,
  pub(crate) compound_redirects: Vec<ShellRedirect>,
  pub(crate) errors: Vec<ShellError>,
  /// A nesting level past `MAX_NESTING` was cut off.
  pub(crate) overflow: bool,
  /// A parse ran out of the budget.
  pub(crate) cancelled: bool,
}

/// The tokens that end a `case` arm; anywhere else they are a syntax error.
const CASE_ENDS: [&str; 3] = [";;", ";&", ";;&"];

/// The children of `node` with their field names.
fn children_with_fields(node: Node<'_>) -> Vec<(Node<'_>, Option<&'static str>)> {
  let mut found = Vec::new();
  let mut cursor = node.walk();
  let mut more = cursor.goto_first_child();
  while more {
    found.push((cursor.node(), cursor.field_name()));
    more = cursor.goto_next_sibling();
  }
  found
}

/// Whether `node` is a statement rather than a comment or a separator.
fn is_statement(node: Node<'_>) -> bool {
  node.is_named() && node.kind() != "comment"
}

impl Walker {
  /// A walker that parses with `syntax`.
  pub(crate) fn new(syntax: Syntax) -> Walker {
    Walker {
      syntax,
      commands: Vec::new(),
      compound_redirects: Vec::new(),
      errors: Vec::new(),
      overflow: false,
      cancelled: false,
    }
  }

  /// One level deeper, or `None` (recorded) past the limit.
  fn enter<'s>(&mut self, ctx: Ctx<'s>) -> Option<Ctx<'s>> {
    if ctx.nesting >= MAX_NESTING {
      if !self.overflow {
        let message = format!("nesting: deeper than {MAX_NESTING} levels");
        let pos = 0;
        self.errors.push(ShellError {
          message,
          pos,
          origin: ctx.origin,
        });
      }
      self.overflow = true;
      return None;
    }
    Some(Ctx {
      nesting: ctx.nesting + 1,
      ..ctx
    })
  }

  /// Walk a parsed script, collecting its errors.
  pub(crate) fn walk_script(&mut self, tree: &Tree, ctx: Ctx<'_>) {
    let root = tree.root_node();
    self
      .errors
      .extend(syntax_errors(tree, ctx.source, ctx.origin));
    self.walk_body(root, &[], ctx);
  }

  /// Walk the statements of `parent` as one list (`walkList`), entering a level.
  /// Children under a field in `skip` are not statements (`for` words, `case` patterns).
  pub(crate) fn walk_body(&mut self, parent: Node<'_>, skip: &[&str], ctx: Ctx<'_>) {
    let Some(ctx) = self.enter(ctx) else {
      return;
    };
    let statements = self.statements(parent, skip, ctx);
    let last = statements.len().saturating_sub(1);
    for (index, (statement, background)) in statements.into_iter().enumerate() {
      let proves = ctx.proves && index == last && !background;
      self.walk_statement(statement, Ctx { proves, ..ctx });
    }
  }

  /// The statements of `parent`, each with whether `&` runs it in the background.
  fn statements<'t>(
    &mut self,
    parent: Node<'t>,
    skip: &[&str],
    ctx: Ctx<'_>,
  ) -> Vec<(Node<'t>, bool)> {
    let mut found: Vec<(Node<'t>, bool)> = Vec::new();
    for (node, field) in children_with_fields(parent) {
      if node.kind() == "&" {
        found.last_mut().into_iter().for_each(|last| last.1 = true);
      } else if CASE_ENDS.contains(&node.kind()) && parent.kind() != "case_item" {
        let message = format!("unexpected token '{}'", node.kind());
        let pos = node.start_byte();
        self.errors.push(ShellError {
          message,
          pos,
          origin: ctx.origin,
        });
      } else if is_statement(node) && !field.is_some_and(|name| skip.contains(&name)) {
        found.push((node, false));
      }
    }
    found
  }

  /// Walk one statement: an and-or list of pipelines (`walkAndOr`, `walkPipeline`).
  pub(crate) fn walk_statement(&mut self, statement: Node<'_>, ctx: Ctx<'_>) {
    let chain = chain::chain(statement, ctx.source);
    let mut and_after = vec![true; chain.pipelines.len()];
    for index in (0..chain.operators.len()).rev() {
      let rest = and_after.get(index + 1).copied().unwrap_or(true);
      let joined = chain.operators.get(index).is_some_and(|op| *op == "&&");
      if let Some(slot) = and_after.get_mut(index) {
        *slot = joined && rest;
      }
    }
    for (index, pipeline) in chain.pipelines.iter().enumerate() {
      let before = index
        .checked_sub(1)
        .and_then(|at| chain.operators.get(at))
        .copied();
      let after = and_after.get(index).copied().unwrap_or(true);
      let proves = ctx.proves && before.is_none_or(|op| op == "&&") && after;
      self.walk_pipeline(pipeline, Ctx { proves, ..ctx });
    }
  }

  /// The commands of one pipeline: only the last of a non-negated one proves.
  fn walk_pipeline(&mut self, pipeline: &chain::Pipeline<'_>, ctx: Ctx<'_>) {
    let size = pipeline.elements.len();
    for (index, element) in pipeline.elements.iter().enumerate() {
      let proves = ctx.proves && !pipeline.negated && index + 1 == size;
      let position = if size > 1 {
        PipelinePosition { index, size }
      } else {
        ctx.pipeline
      };
      let at = Ctx {
        proves,
        pipeline: position,
        ..ctx
      };
      self.walk_element(element.node, &element.redirects, at);
    }
  }

  /// One element of a pipeline, with redirects an enclosing list put on it.
  fn walk_element<'t>(&mut self, node: Node<'t>, extra: &[Node<'t>], ctx: Ctx<'_>) {
    match node.kind() {
      _ if command::is_simple(node) => command::emit(self, node, extra, ctx),
      "redirected_statement" => self.redirected(node, extra, ctx),
      kind if kind.ends_with("_redirect") => {
        let all: Vec<Node<'t>> = std::iter::once(node).chain(extra.iter().copied()).collect();
        command::emit(self, node, &all, ctx);
      }
      _ => {
        self.compound_redirects(extra, ctx);
        match node.kind() {
          "list" | "pipeline" | "negated_command" => self.walk_statement(node, ctx),
          "ERROR" => self.walk_body(node, &[], ctx.off()),
          _ => compound::walk(self, node, ctx),
        }
      }
    }
  }

  /// A statement with redirects: on a simple command they are the command's;
  /// on anything else they are compound redirects.
  fn redirected<'t>(&mut self, node: Node<'t>, extra: &[Node<'t>], ctx: Ctx<'_>) {
    let mut cursor = node.walk();
    let mut redirects: Vec<Node<'t>> = node
      .children_by_field_name("redirect", &mut cursor)
      .collect();
    redirects.extend_from_slice(extra);
    match node.child_by_field_name("body") {
      None => command::emit(self, node, &redirects, ctx),
      Some(body) => self.walk_element(body, &redirects, ctx),
    }
  }
}

#[cfg(test)]
#[path = "tests/walk_test.rs"]
mod tests;
