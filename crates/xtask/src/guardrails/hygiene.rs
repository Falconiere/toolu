//! Rule 12: every crate inherits the workspace lints and version, and its
//! crate root opens with a `//!` doc comment.

use syn::{AttrStyle, Attribute};

use super::{Context, Finding};

/// Manifests that do not inherit, and crate roots without a crate doc.
pub(super) fn check(ctx: &Context<'_>) -> Result<Vec<Finding>, String> {
  let mut found = Vec::new();
  for member in &ctx.workspace.members {
    let manifest = member.dir.join("Cargo.toml");
    let shown = manifest.to_string_lossy().replace('\\', "/");
    let text = ctx.workspace.read(&manifest)?;
    let table: toml::Table = text.parse().map_err(|err| format!("{shown}: {err}"))?;
    if !inherits(&table, &["lints", "workspace"]) {
      found.push(Finding::new(
        "crate-hygiene",
        &shown,
        1,
        "missing `[lints] workspace = true`".to_owned(),
      ));
    }
    if !inherits(&table, &["package", "version", "workspace"]) {
      found.push(Finding::new(
        "crate-hygiene",
        &shown,
        1,
        "missing `version.workspace = true`".to_owned(),
      ));
    }
    for root in ["src/lib.rs", "src/main.rs"] {
      let rel = member.dir.join(root);
      let Some(source) = ctx.source(&rel) else {
        continue;
      };
      let documented = source
        .ast
        .as_ref()
        .is_ok_and(|ast| ast.attrs.iter().any(is_inner_doc));
      if !documented {
        found.push(Finding::new(
          "crate-hygiene",
          &source.display(),
          1,
          "no crate-level `//!` doc comment".to_owned(),
        ));
      }
    }
  }
  Ok(found)
}

/// Whether `table` has `true` at `keys`.
fn inherits(table: &toml::Table, keys: &[&str]) -> bool {
  let Some((last, parents)) = keys.split_last() else {
    return false;
  };
  let mut current = table;
  for key in parents {
    match current.get(*key) {
      Some(toml::Value::Table(inner)) => current = inner,
      _ => return false,
    }
  }
  current.get(*last).and_then(toml::Value::as_bool) == Some(true)
}

fn is_inner_doc(attr: &Attribute) -> bool {
  matches!(attr.style, AttrStyle::Inner(_)) && attr.path().is_ident("doc")
}

#[cfg(test)]
#[path = "tests/hygiene_test.rs"]
mod tests;
