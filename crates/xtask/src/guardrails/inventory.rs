//! Rule 8: every discovered item (today: each `cargo xtask` task) names a
//! passing and a failing scenario, each resolved to a real, non-ignored test.

use std::collections::BTreeSet;
use std::path::Path;

use syn::spanned::Spanned;

use super::syntax::{line, string_literal, test_fns};
use super::{Context, Finding};
use crate::data::{DATA_DIR, InventoryKind};

const INVENTORY: &str = "inventory.json";

/// Missing entries, stale entries and scenarios that resolve to no test.
pub(super) fn check(ctx: &Context<'_>) -> Result<Vec<Finding>, String> {
  let file = format!("{DATA_DIR}/{INVENTORY}");
  let mut found = Vec::new();
  let mut known = BTreeSet::new();
  for kind in &ctx.rules.inventory.kinds {
    let Some((at, items)) = discover(ctx, kind)? else {
      continue;
    };
    for item in &items {
      let listed = ctx
        .inventory
        .entries
        .iter()
        .any(|entry| entry.kind == kind.kind && &entry.id == item);
      if !listed {
        found.push(Finding::new(
          "inventory",
          &kind.file,
          at,
          format!(
            "{} `{item}` has no entry in {file}: add a passing and a failing scenario",
            kind.kind
          ),
        ));
      }
      known.insert((kind.kind.clone(), item.clone()));
    }
  }
  for entry in &ctx.inventory.entries {
    if !known.contains(&(entry.kind.clone(), entry.id.clone())) {
      found.push(Finding::new(
        "inventory",
        &file,
        1,
        format!("{} `{}` is not a discovered item", entry.kind, entry.id),
      ));
    }
    for scenario in [&entry.pass, &entry.fail] {
      if let Some(problem) = unresolved(ctx, scenario) {
        found.push(Finding::new(
          "inventory",
          &file,
          1,
          format!("{} `{}`: {problem}", entry.kind, entry.id),
        ));
      }
    }
  }
  Ok(found)
}

/// The items of `kind` and the line of their table, or `None` when its file is absent.
fn discover(
  ctx: &Context<'_>,
  kind: &InventoryKind,
) -> Result<Option<(usize, Vec<String>)>, String> {
  let Some(source) = ctx.source(Path::new(&kind.file)) else {
    return Ok(None);
  };
  let ast = source
    .ast
    .as_ref()
    .map_err(|(_, err)| format!("{}: cannot discover {}: {err}", kind.file, kind.kind))?;
  let table = ast.items.iter().find_map(|item| {
    let syn::Item::Const(constant) = item else {
      return None;
    };
    (constant.ident == kind.constant).then_some(constant)
  });
  let table = table.ok_or_else(|| {
    format!(
      "{}: no const {} to discover {} items from",
      kind.file, kind.constant, kind.kind
    )
  })?;
  Ok(Some((line(table.span()), names(&table.expr))))
}

/// String literals that are array elements, or the first field of a tuple element.
fn names(expr: &syn::Expr) -> Vec<String> {
  if let syn::Expr::Reference(reference) = expr {
    return names(&reference.expr);
  }
  let syn::Expr::Array(array) = expr else {
    return Vec::new();
  };
  array.elems.iter().filter_map(first_str).collect()
}

fn first_str(expr: &syn::Expr) -> Option<String> {
  if let syn::Expr::Tuple(tuple) = expr {
    return tuple.elems.first().and_then(first_str);
  }
  string_literal(expr)
}

/// Why `scenario` (`<path>::<test fn>`) does not resolve, or `None`.
fn unresolved(ctx: &Context<'_>, scenario: &str) -> Option<String> {
  let Some((path, name)) = scenario.rsplit_once("::") else {
    return Some(format!("`{scenario}` is not <path>::<test fn>"));
  };
  let Some(source) = ctx.source(Path::new(path)) else {
    return Some(format!("{path} is not a Rust file of the workspace"));
  };
  let tests = source.ast.as_ref().map(test_fns).unwrap_or_default();
  match tests.iter().find(|test| test.name == name) {
    None => Some(format!("{path} has no #[test] fn {name}")),
    Some(test) if test.ignored => Some(format!("{path}::{name} is #[ignore]d")),
    Some(_) => None,
  }
}

#[cfg(test)]
#[path = "tests/inventory_test.rs"]
mod tests;
