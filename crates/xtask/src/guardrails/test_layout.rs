//! Rule 6: no inline test module in `src`; `tests/` directories stay flat;
//! unit-test files are named `<module>_test.rs`.

use std::path::Path;

use syn::visit::Visit;

use super::syntax::{is_cfg_test, line};
use super::{Context, Finding};
use crate::source::{Kind, Source};
use crate::workspace::parts;

/// Inline test bodies, nested test directories and misnamed unit-test files.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  let mut found = Vec::new();
  for source in &ctx.sources {
    if source.kind == Kind::Src {
      found.extend(inline_test_modules(source));
    }
    if source.is_test() {
      found.extend(layout(ctx, source));
    }
  }
  found
}

fn inline_test_modules(source: &Source<'_>) -> Vec<Finding> {
  struct Inline(Vec<usize>);
  impl<'ast> Visit<'ast> for Inline {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
      if item.content.is_some() && item.attrs.iter().any(is_cfg_test) {
        self.0.push(line(item.ident.span()));
      }
      syn::visit::visit_item_mod(self, item);
    }
  }
  let Ok(ast) = &source.ast else {
    return Vec::new();
  };
  let mut inline = Inline(Vec::new());
  inline.visit_file(ast);
  inline
    .0
    .into_iter()
    .map(|at| {
      Finding::new(
        "test-layout",
        &source.display(),
        at,
        "inline #[cfg(test)] module body in src — move the tests to tests/<module>_test.rs \
         beside this file, wired by a bodyless #[cfg(test)] #[path] mod declaration"
          .to_owned(),
      )
    })
    .collect()
}

/// The part of a test file's path after its last `tests` directory.
fn after_tests(rel: &Path) -> Vec<String> {
  let parts = parts(rel);
  let start = parts
    .iter()
    .rposition(|part| part == "tests")
    .map_or(0, |at| at + 1);
  parts.into_iter().skip(start).collect()
}

fn layout(ctx: &Context<'_>, source: &Source<'_>) -> Vec<Finding> {
  let path = source.display();
  let tail = after_tests(&source.rel);
  let subdirs = &ctx.rules.tests.subdirs;
  if let [first, _, ..] = tail.as_slice()
    && !subdirs.contains(first)
  {
    return vec![Finding::new(
      "test-layout",
      &path,
      1,
      format!(
        "tests/ is flat: only {} may be subdirectories",
        subdirs.join(", ")
      ),
    )];
  }
  let suffix = &ctx.rules.tests.file_suffix;
  let named = source
    .rel
    .file_stem()
    .is_some_and(|stem| stem.to_string_lossy().ends_with(suffix.as_str()));
  if source.kind == Kind::UnitTest && tail.len() == 1 && !named {
    return vec![Finding::new(
      "test-layout",
      &path,
      1,
      format!("a unit-test file under src is named <module>{suffix}.rs"),
    )];
  }
  Vec::new()
}

#[cfg(test)]
#[path = "tests/test_layout_test.rs"]
mod tests;
