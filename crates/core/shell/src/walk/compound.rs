//! Compound commands (`walkNode` in `shell-walk.ts`). Subshells and groups keep
//! the caller's `proves`; conditions, loop bodies and case arms never prove the
//! line's status; a function body runs only when called. Words that are not
//! commands (`for` words, `case` patterns, C-style `for` clauses) are scanned
//! for nested scripts.

use tree_sitter::Node;

use crate::analysis::CommandOrigin;
use crate::walk::{Ctx, Walker};

/// Scan the children of `node` under any field in `fields` for nested scripts.
fn scan_fields(walker: &mut Walker, node: Node<'_>, fields: &[&str], ctx: Ctx<'_>) {
  let mut cursor = node.walk();
  let mut more = cursor.goto_first_child();
  let mut found = Vec::new();
  while more {
    if cursor
      .field_name()
      .is_some_and(|name| fields.contains(&name))
    {
      found.push(cursor.node());
    }
    more = cursor.goto_next_sibling();
  }
  for child in found {
    walker.visit_nested(child, ctx);
  }
}

/// The first child token is `token` (`((`, `{`).
fn opens_with(node: Node<'_>, token: &str) -> bool {
  node.child(0).is_some_and(|first| first.kind() == token)
}

/// A function definition: its redirects, then its body as `origin = function`.
fn function(walker: &mut Walker, node: Node<'_>, ctx: Ctx<'_>) {
  let mut cursor = node.walk();
  let redirects: Vec<Node<'_>> = node
    .children_by_field_name("redirect", &mut cursor)
    .collect();
  walker.compound_redirects(&redirects, ctx);
  if let Some(body) = node.child_by_field_name("body") {
    let inside = Ctx {
      origin: CommandOrigin::Function,
      ..ctx.off()
    };
    walker.walk_statement(body, inside);
  }
}

/// Walk a compound command.
pub(crate) fn walk(walker: &mut Walker, node: Node<'_>, ctx: Ctx<'_>) {
  let off = ctx.off();
  match node.kind() {
    "compound_statement" if opens_with(node, "((") => walker.visit_nested(node, ctx),
    "subshell" | "compound_statement" => walker.walk_body(node, &[], ctx),
    "do_group" | "else_clause" | "elif_clause" | "if_statement" | "while_statement" => {
      walker.walk_body(node, &[], off);
    }
    "for_statement" => {
      scan_fields(walker, node, &["value"], ctx);
      walker.walk_body(node, &["variable", "value"], off);
    }
    "c_style_for_statement" => {
      let clauses = ["initializer", "condition", "update"];
      scan_fields(walker, node, &clauses, ctx);
      walker.walk_body(node, &clauses, off);
    }
    "case_statement" | "case_item" => {
      scan_fields(walker, node, &["value"], ctx);
      walker.walk_body(node, &["value"], off);
    }
    "function_definition" => function(walker, node, ctx),
    _ => walker.visit_nested(node, ctx),
  }
}

#[cfg(test)]
#[path = "tests/compound_test.rs"]
mod tests;
