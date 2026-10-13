//! Install order from the catalog, not the CLI: every plugin follows the plugins
//! it depends on, so adding one to the manifest needs no code change.

use std::collections::BTreeSet;

use crate::catalog::Catalog;
use crate::fail::{Fail, Res};

struct Walk<'a> {
  catalog: &'a Catalog,
  ordered: Vec<String>,
  seen: BTreeSet<String>,
  active: BTreeSet<String>,
}

impl Walk<'_> {
  fn visit(&mut self, name: &str) -> Res<()> {
    if self.seen.contains(name) {
      return Ok(());
    }
    if self.active.contains(name) {
      return Err(Fail::usage(format!(
        "circular plugin dependency involving {name}"
      )));
    }
    let Some(entry) = self.catalog.entry(name) else {
      return Err(Fail::usage(format!(
        "unknown plugin: {name}. Run `toolu plugins list` for the catalog."
      )));
    };
    self.active.insert(name.to_owned());
    for dependency in &entry.dependencies {
      if self.catalog.entry(dependency).is_some() {
        self.visit(dependency)?;
      }
    }
    self.active.remove(name);
    self.seen.insert(name.to_owned());
    self.ordered.push(name.to_owned());
    Ok(())
  }
}

/// `requested` with every dependency first; an empty request is the whole
/// catalog. An unknown name or a cycle is a usage error.
pub(crate) fn install_order(catalog: &Catalog, requested: &[String]) -> Res<Vec<String>> {
  let mut walk = Walk {
    catalog,
    ordered: Vec::new(),
    seen: BTreeSet::new(),
    active: BTreeSet::new(),
  };
  let names = if requested.is_empty() {
    catalog.names()
  } else {
    requested.to_vec()
  };
  for name in &names {
    walk.visit(name)?;
  }
  Ok(walk.ordered)
}

/// The plugins that declare `name` as a dependency, in manifest order.
pub(crate) fn dependents_of(catalog: &Catalog, name: &str) -> Vec<String> {
  catalog
    .plugins
    .iter()
    .filter(|entry| {
      entry
        .dependencies
        .iter()
        .any(|dependency| dependency == name)
    })
    .map(|entry| entry.name.clone())
    .collect()
}

#[cfg(test)]
#[path = "tests/order_test.rs"]
mod tests;
