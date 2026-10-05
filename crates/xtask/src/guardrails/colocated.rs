//! Rule 7: a module file that defines a function has `tests/<module>_test.rs`
//! beside it, wired by a bodyless `#[cfg(test)]` declaration, with a test in it.

use std::path::{Path, PathBuf};

use syn::visit::Visit;

use super::syntax::{is_cfg_test, path_value, test_fns};
use super::{Context, Finding};
use crate::source::{Kind, Source};

/// Module files with functions and no wired, non-empty unit-test file.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  ctx
    .sources
    .iter()
    .filter(|source| source.kind == Kind::Src)
    .filter_map(|source| missing(ctx, source))
    .collect()
}

fn missing(ctx: &Context<'_>, source: &Source<'_>) -> Option<Finding> {
  let ast = source.ast.as_ref().ok()?;
  if !defines_fn(ast) {
    return None;
  }
  let stem = source.rel.file_stem()?.to_string_lossy().into_owned();
  let suffix = &ctx.rules.tests.file_suffix;
  let wiring = format!("tests/{stem}{suffix}.rs");
  let expected: PathBuf = source.rel.parent().unwrap_or(Path::new("")).join(&wiring);
  let shown = expected.to_string_lossy().replace('\\', "/");
  let problem = match ctx.source(&expected) {
    None => format!("defines functions but has no {shown} beside it"),
    Some(_) if !wired(ast, &wiring) => {
      format!("{shown} is not wired: declare #[cfg(test)] #[path = \"{wiring}\"] mod tests;")
    }
    Some(test) if !has_test(test) => format!("{shown} has no #[test] function"),
    Some(_) => return None,
  };
  Some(Finding::new(
    "colocated-tests",
    &source.display(),
    1,
    problem,
  ))
}

fn has_test(test: &Source<'_>) -> bool {
  test.ast.as_ref().is_ok_and(|ast| !test_fns(ast).is_empty())
}

/// Whether `ast` declares `#[cfg(test)] #[path = "<wiring>"] mod …;`.
fn wired(ast: &syn::File, wiring: &str) -> bool {
  ast.items.iter().any(|item| {
    let syn::Item::Mod(module) = item else {
      return false;
    };
    module.content.is_none()
      && module.attrs.iter().any(is_cfg_test)
      && module
        .attrs
        .iter()
        .any(|attr| path_value(attr).as_deref() == Some(wiring))
  })
}

/// Whether `ast` has a function with a body outside test modules.
fn defines_fn(ast: &syn::File) -> bool {
  struct Fns(bool);
  impl<'ast> Visit<'ast> for Fns {
    fn visit_item_fn(&mut self, _: &'ast syn::ItemFn) {
      self.0 = true;
    }
    fn visit_impl_item_fn(&mut self, _: &'ast syn::ImplItemFn) {
      self.0 = true;
    }
    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
      self.0 |= item.default.is_some();
    }
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
      if !item.attrs.iter().any(is_cfg_test) {
        syn::visit::visit_item_mod(self, item);
      }
    }
  }
  let mut fns = Fns(false);
  fns.visit_file(ast);
  fns.0
}

#[cfg(test)]
#[path = "tests/colocated_test.rs"]
mod tests;
