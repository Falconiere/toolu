//! Rule 15, the part no dependency tool sees: no type-erased error
//! (`dyn … Error`) in a library's public function signatures.

use syn::spanned::Spanned;
use syn::visit::Visit;

use super::syntax::line;
use super::{Context, Finding};
use crate::source::Kind;

/// `pub fn` signatures of library crates that mention `dyn … Error`.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  let mut found = Vec::new();
  for source in &ctx.sources {
    let is_lib = source.member.is_some_and(|member| member.is_lib);
    if source.kind != Kind::Src || !is_lib {
      continue;
    }
    let Ok(ast) = &source.ast else { continue };
    let mut public = Public(Vec::new());
    public.visit_file(ast);
    for (at, name) in public.0 {
      found.push(Finding::new(
        "public-api",
        &source.display(),
        at,
        format!("pub fn {name} returns or takes a type-erased error; use a typed error"),
      ));
    }
  }
  found
}

/// Public functions whose signature names a `dyn` trait ending in `Error`.
struct Public(Vec<(usize, String)>);

impl Public {
  fn sig(&mut self, vis: &syn::Visibility, sig: &syn::Signature) {
    if !matches!(vis, syn::Visibility::Public(_)) {
      return;
    }
    let mut erased = Erased(false);
    erased.visit_signature(sig);
    if erased.0 {
      self.0.push((line(sig.span()), sig.ident.to_string()));
    }
  }
}

impl<'ast> Visit<'ast> for Public {
  fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
    self.sig(&item.vis, &item.sig);
  }

  fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
    self.sig(&item.vis, &item.sig);
  }
}

struct Erased(bool);

impl<'ast> Visit<'ast> for Erased {
  fn visit_type_trait_object(&mut self, object: &'ast syn::TypeTraitObject) {
    self.0 |= object.bounds.iter().any(|bound| {
      let syn::TypeParamBound::Trait(bound) = bound else {
        return false;
      };
      bound
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Error")
    });
    syn::visit::visit_type_trait_object(self, object);
  }
}

#[cfg(test)]
#[path = "tests/public_api_test.rs"]
mod tests;
