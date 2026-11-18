//! Rule 7: authored sibling unit tests are wired and non-empty.
//! Integration coverage does not require a second test file per module.

use std::path::{Path, PathBuf};

use super::syntax::{is_cfg_test, path_value, test_fns};
use super::{Context, Finding};
use crate::source::{Kind, Source};

/// Authored unit-test files that are missing, unwired or empty.
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
  let stem = source.rel.file_stem()?.to_string_lossy().into_owned();
  let suffix = &ctx.rules.tests.file_suffix;
  let wiring = format!("tests/{stem}{suffix}.rs");
  let expected: PathBuf = source.rel.parent().unwrap_or(Path::new("")).join(&wiring);
  let shown = expected.to_string_lossy().replace('\\', "/");
  let declared = wired(ast, &wiring);
  let problem = match ctx.source(&expected) {
    None if !declared => return None,
    None => format!("declares tests but has no {shown} beside it"),
    Some(_) if !declared => {
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

#[cfg(test)]
#[path = "tests/colocated_test.rs"]
mod tests;
