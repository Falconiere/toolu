//! `cargo xtask check-unused-pub`: every `pub` item of a library crate is used
//! by another crate or by a test (rule 21). Binary crates are held by `unreachable_pub`.

use std::collections::BTreeSet;

use syn::visit::Visit;

use crate::options::Options;
use crate::source::{Kind, Source};
use crate::workspace::Workspace;
use crate::{Verdict, output};

/// Public items nobody else names.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let workspace = Workspace::load(&options.root)?;
  let sources = workspace
    .files_in("crates", "rs")
    .map(|rel| Source::load(&workspace, rel))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(output::findings("check-unused-pub", &unused(&sources)))
}

/// One line per unused public item.
pub(crate) fn unused(sources: &[Source<'_>]) -> Vec<String> {
  let mut found = Vec::new();
  for source in sources {
    let Some(member) = source.member else {
      continue;
    };
    if source.kind != Kind::Src || !member.is_lib {
      continue;
    }
    let Ok(ast) = &source.ast else { continue };
    let mut items = PubItems(Vec::new());
    items.visit_file(ast);
    for (line, name) in items.0 {
      let used = sources.iter().any(|other| {
        let elsewhere = other.member.is_none_or(|owner| owner.name != member.name);
        let own_test =
          other.member.is_some_and(|owner| owner.name == member.name) && other.is_test();
        (elsewhere || own_test) && idents(other).contains(name.as_str())
      });
      if !used {
        found.push(format!(
          "unused-pub {}:{line}: {name} is used by no other crate and no test",
          source.display()
        ));
      }
    }
  }
  found
}

/// Every identifier token of `source`.
fn idents(source: &Source<'_>) -> BTreeSet<String> {
  struct Idents(BTreeSet<String>);
  impl<'ast> Visit<'ast> for Idents {
    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
      self.0.insert(ident.to_string());
    }
  }
  let mut idents = Idents(BTreeSet::new());
  if let Ok(ast) = &source.ast {
    idents.visit_file(ast);
  }
  idents.0
}

/// Items declared exactly `pub`, with their line and name.
struct PubItems(Vec<(usize, String)>);

impl PubItems {
  fn item(&mut self, vis: &syn::Visibility, ident: &syn::Ident) {
    if matches!(vis, syn::Visibility::Public(_)) {
      self.0.push((ident.span().start().line, ident.to_string()));
    }
  }
}

/// The visibility and name of an item that can be `pub` and named elsewhere.
fn named(item: &syn::Item) -> Option<(&syn::Visibility, &syn::Ident)> {
  use syn::Item;
  if let Item::Fn(item) = item {
    return Some((&item.vis, &item.sig.ident));
  }
  if let Item::Struct(item) = item {
    return Some((&item.vis, &item.ident));
  }
  if let Item::Enum(item) = item {
    return Some((&item.vis, &item.ident));
  }
  if let Item::Trait(item) = item {
    return Some((&item.vis, &item.ident));
  }
  if let Item::Const(item) = item {
    return Some((&item.vis, &item.ident));
  }
  if let Item::Static(item) = item {
    return Some((&item.vis, &item.ident));
  }
  if let Item::Type(item) = item {
    return Some((&item.vis, &item.ident));
  }
  let Item::Union(item) = item else {
    return None;
  };
  Some((&item.vis, &item.ident))
}

impl<'ast> Visit<'ast> for PubItems {
  fn visit_item(&mut self, item: &'ast syn::Item) {
    if let Some((vis, ident)) = named(item) {
      self.item(vis, ident);
    }
    syn::visit::visit_item(self, item);
  }
}

#[cfg(test)]
#[path = "tests/unused_pub_test.rs"]
mod tests;
