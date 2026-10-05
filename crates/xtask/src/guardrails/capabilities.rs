//! Rule 14: environment reads, process spawning and the standard streams only
//! in the modules that own them (`rules.json` `capabilities`).

use proc_macro2::TokenTree;
use syn::UseTree;
use syn::spanned::Spanned;
use syn::visit::Visit;

use super::syntax::line;
use super::{Context, Finding};
use crate::data::Capability;
use crate::source::{Kind, Source};
use crate::workspace::parts;

/// Capability uses in `src` outside their owners.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  let mut found = Vec::new();
  for source in ctx.sources.iter().filter(|source| source.kind == Kind::Src) {
    let Ok(ast) = &source.ast else { continue };
    let mut paths = Paths(Vec::new());
    paths.visit_file(ast);
    for capability in &ctx.rules.capabilities {
      if !owns(source, capability) {
        found.extend(hits(source, &paths, capability));
      }
    }
  }
  found
}

/// Uses of `capability` among the paths of `source`.
fn hits(source: &Source<'_>, paths: &Paths, capability: &Capability) -> Vec<Finding> {
  paths
    .0
    .iter()
    .filter_map(|(at, segments)| {
      let window = uses(segments, capability)?;
      Some(Finding::new(
        "capabilities",
        &source.display(),
        *at,
        format!(
          "`{window}` is the `{}` capability; only {} may use it",
          capability.id,
          owners(capability)
        ),
      ))
    })
    .collect()
}

fn owners(capability: &Capability) -> String {
  capability
    .owners
    .iter()
    .map(|owner| match &owner.module {
      Some(module) => format!("{}::{module}", owner.krate),
      None => owner.krate.clone(),
    })
    .collect::<Vec<_>>()
    .join(", ")
}

/// Whether the crate (and top-level module) of `source` owns `capability`.
fn owns(source: &Source<'_>, capability: &Capability) -> bool {
  let Some(member) = source.member else {
    return false;
  };
  let module = source.in_src().and_then(|inside| {
    parts(&inside)
      .first()
      .map(|name| name.trim_end_matches(".rs").to_owned())
  });
  capability.owners.iter().any(|owner| {
    owner.krate == member.name
      && owner
        .module
        .as_ref()
        .is_none_or(|wanted| module.as_ref() == Some(wanted))
  })
}

/// A `use` of a whole module, by glob or under another name, ends in this marker.
const WHOLE_MODULE: &str = "*";

/// The `a::b` tail of a capability path that `segments` contains, or the
/// `module::*` a whole-module import of its full parent path makes, if any.
fn uses(segments: &[String], capability: &Capability) -> Option<String> {
  if let Some((last, module)) = segments.split_last()
    && last == WHOLE_MODULE
  {
    return capability.paths.iter().find_map(|path| {
      let (parent, _) = path.rsplit_once("::")?;
      let name = parent.rsplit("::").next()?;
      (module.join("::") == parent).then(|| format!("{name}::*"))
    });
  }
  capability.paths.iter().find_map(|path| {
    let tail: Vec<&str> = path
      .rsplit("::")
      .take(2)
      .collect::<Vec<_>>()
      .into_iter()
      .rev()
      .collect();
    segments
      .windows(tail.len())
      .any(|window| window.iter().zip(&tail).all(|(have, want)| have == want))
      .then(|| tail.join("::"))
  })
}

/// Every path in a file with its line: expression and type paths, macro paths
/// and each import a `use` tree spells out.
struct Paths(Vec<(usize, Vec<String>)>);

impl Paths {
  fn tree(&mut self, prefix: &[String], tree: &UseTree) {
    let mut joined = prefix.to_vec();
    match tree {
      UseTree::Path(path) => {
        joined.push(path.ident.to_string());
        self.tree(&joined, &path.tree);
      }
      UseTree::Name(name) => {
        joined.push(name.ident.to_string());
        self.0.push((line(name.ident.span()), joined));
      }
      UseTree::Rename(rename) => {
        // `{self as x}` renames the module itself.
        if rename.ident != "self" {
          joined.push(rename.ident.to_string());
        }
        let at = line(rename.ident.span());
        let mut whole = joined.clone();
        whole.push(WHOLE_MODULE.to_owned());
        self.0.push((at, joined));
        self.0.push((at, whole));
      }
      UseTree::Glob(glob) => {
        joined.push(WHOLE_MODULE.to_owned());
        self.0.push((line(glob.span()), joined));
      }
      UseTree::Group(group) => {
        for item in &group.items {
          self.tree(prefix, item);
        }
      }
    }
  }
}

impl Paths {
  /// Paths spelled inside macro arguments, which the syntax tree keeps as tokens:
  /// every run of identifiers joined by `::`.
  fn tokens(&mut self, tokens: proc_macro2::TokenStream) {
    let mut run = Run::default();
    for tree in tokens {
      match tree {
        TokenTree::Ident(ident) => run.ident(&ident, self),
        TokenTree::Punct(punct) if punct.as_char() == ':' => run.colons += 1,
        TokenTree::Group(group) => {
          run.end(self);
          self.tokens(group.stream());
        }
        TokenTree::Punct(_) | TokenTree::Literal(_) => run.end(self),
      }
    }
    run.end(self);
  }
}

/// The identifiers of one `a::b::c` run in a token stream.
#[derive(Default)]
struct Run {
  segments: Vec<String>,
  at: usize,
  colons: usize,
}

impl Run {
  fn ident(&mut self, ident: &proc_macro2::Ident, paths: &mut Paths) {
    if self.colons != 2 {
      self.end(paths);
      self.at = line(ident.span());
    }
    self.segments.push(ident.to_string());
    self.colons = 0;
  }

  fn end(&mut self, paths: &mut Paths) {
    if self.segments.len() > 1 {
      paths.0.push((self.at, std::mem::take(&mut self.segments)));
    }
    self.segments.clear();
    self.colons = 0;
  }
}

impl<'ast> Visit<'ast> for Paths {
  fn visit_macro(&mut self, mac: &'ast syn::Macro) {
    self.tokens(mac.tokens.clone());
    syn::visit::visit_macro(self, mac);
  }

  fn visit_path(&mut self, path: &'ast syn::Path) {
    let segments = path
      .segments
      .iter()
      .map(|segment| segment.ident.to_string())
      .collect();
    self.0.push((line(path.span()), segments));
    syn::visit::visit_path(self, path);
  }

  fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
    self.tree(&[], &item.tree);
  }
}

#[cfg(test)]
#[path = "tests/capabilities_test.rs"]
mod tests;
