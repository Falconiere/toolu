//! Rules 1 and 3: file length and `impl` block length, in code lines.

use syn::spanned::Spanned;
use syn::visit::Visit;

use super::syntax::line;
use super::{Context, Finding};

/// Files over `maxFileLines` and `impl` blocks over `maxImplLines`.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  let mut found = Vec::new();
  for source in &ctx.sources {
    let path = source.display();
    let lines = source.lines.code_lines();
    if lines > ctx.limits.file {
      found.push(Finding::new(
        "file-length",
        &path,
        1,
        format!(
          "{lines} code lines, limit {} — split the module",
          ctx.limits.file
        ),
      ));
    }
    let Ok(ast) = &source.ast else { continue };
    let mut impls = Impls(Vec::new());
    impls.visit_file(ast);
    for (first, last) in impls.0 {
      let lines = source.lines.code_lines_between(first, last);
      if lines > ctx.limits.impl_block {
        found.push(Finding::new(
          "impl-length",
          &path,
          first,
          format!(
            "impl block has {lines} code lines, limit {} — split it",
            ctx.limits.impl_block
          ),
        ));
      }
    }
  }
  found
}

/// First and last line of every `impl` block.
struct Impls(Vec<(usize, usize)>);

impl<'ast> Visit<'ast> for Impls {
  fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
    let span = item.span();
    self.0.push((line(span), span.end().line));
    syn::visit::visit_item_impl(self, item);
  }
}

#[cfg(test)]
#[path = "tests/size_test.rs"]
mod tests;
