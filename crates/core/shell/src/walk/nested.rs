//! Scripts nested in a command (#416): substitutions found inside words,
//! `bash -c`/`eval` strings and decoded backtick bodies parsed again, and the
//! redirects of compound commands, whose targets can run substitutions too.

use tree_sitter::Node;

use crate::analysis::{CommandOrigin, PipelinePosition, ShellError};
use crate::heredoc;
use crate::parse::{ParseFailure, preorder};
use crate::walk::{Ctx, Walker};
use crate::words::{self, quote};

/// The body of a backtick substitution that holds a backslash: bash decodes
/// it before parsing, so it is parsed again decoded, as unbash does.
fn escaped_backticks<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
  if node.kind() != "command_substitution" {
    return None;
  }
  let text = words::text_of(node, source);
  let body = text.strip_prefix('`')?;
  let body = body.strip_suffix('`').unwrap_or(body);
  body.contains('\\').then_some(body)
}

/// Whether the backtick substitution `node` holds an unescaped backtick:
/// tree-sitter-bash reads `` `a` `b` `` as one substitution, `a` `b`.
fn merged_backticks(node: Node<'_>, source: &str) -> bool {
  let text = words::text_of(node, source);
  let Some(body) = text.strip_prefix('`') else {
    return false;
  };
  let mut chars = body.strip_suffix('`').unwrap_or(body).chars();
  while let Some(c) = chars.next() {
    match c {
      '\\' => {
        chars.next();
      }
      '`' => return true,
      _ => {}
    }
  }
  false
}

impl Walker {
  /// Parse `script` and walk it as `origin` (`bash -c`, `eval`).
  pub(crate) fn run_string(&mut self, script: &str, origin: CommandOrigin, ctx: Ctx<'_>) {
    let depth = ctx.depth + 1;
    self.parse_and_walk(
      script,
      Ctx {
        origin,
        depth,
        ..ctx
      },
    );
  }

  /// Parse `script` and walk it in `ctx` (whose `source` is replaced).
  fn parse_and_walk(&mut self, script: &str, ctx: Ctx<'_>) {
    match self.syntax.script(script) {
      Ok((tree, text)) => self.walk_script(
        &tree,
        Ctx {
          source: &text,
          ..ctx
        },
      ),
      Err(failure) => self.fail(&failure, ctx.origin),
    }
  }

  /// Record a script that could not be parsed.
  pub(crate) fn fail(&mut self, failure: &ParseFailure, origin: CommandOrigin) {
    self.errors.push(ShellError {
      message: failure.message(),
      pos: 0,
      origin,
    });
  }

  /// Record an error for each heredoc whose body tree-sitter split (`split_body`).
  pub(crate) fn split_bodies(&mut self, redirects: &[Node<'_>], ctx: Ctx<'_>) {
    for node in redirects {
      if node.kind() == "heredoc_redirect" && heredoc::split_body(*node, ctx.source) {
        self.errors.push(ShellError {
          message: "heredoc: the body's first line was read as a word".to_owned(),
          pos: node.start_byte(),
          origin: ctx.origin,
        });
      }
    }
  }

  /// Redirects on a compound command, and the scripts their targets run.
  pub(crate) fn compound_redirects(&mut self, redirects: &[Node<'_>], ctx: Ctx<'_>) {
    self.split_bodies(redirects, ctx);
    for node in redirects {
      let Some(redirect) = crate::redirect::read(*node, ctx.source) else {
        continue;
      };
      for nested in &redirect.nested {
        self.visit_nested(*nested, ctx);
      }
      self.compound_redirects.push(redirect.record);
    }
  }

  /// Walk every script nested in `node`: `$(…)`, backticks, `<(…)`, `>(…)`.
  /// They run, but their status proves nothing.
  pub(crate) fn visit_nested(&mut self, node: Node<'_>, ctx: Ctx<'_>) {
    let nested = Ctx {
      origin: CommandOrigin::Substitution,
      proves: false,
      pipeline: PipelinePosition::ALONE,
      ..ctx
    };
    preorder(node, |next| {
      let substitution = next.kind() == "command_substitution";
      if substitution && merged_backticks(next, ctx.source) {
        self.errors.push(ShellError {
          message: "backticks: two substitutions read as one".to_owned(),
          pos: next.start_byte(),
          origin: ctx.origin,
        });
        return false;
      }
      if let Some(body) = escaped_backticks(next, ctx.source) {
        self.parse_and_walk(&quote::backticks(body), nested);
        return false;
      }
      if matches!(next.kind(), "command_substitution" | "process_substitution") {
        self.walk_body(next, &[], nested);
        return false;
      }
      true
    });
  }
}

#[cfg(test)]
#[path = "tests/nested_test.rs"]
mod tests;
